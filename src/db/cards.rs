//! Card CRUD and query operations
//!
//! Cards use a two-database model:
//! - Card definitions (content) are in app.db/card_definitions
//! - Card progress (SRS state) is in learning.db/card_progress
//!
//! Queries JOIN these tables via ATTACH DATABASE (app.db attached as "app").

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};

use crate::domain::{Card, CardType, FsrsState};
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

use super::lesson_progress::{build_filter_where_clause, StudyFilterMode};
use super::tiers::{get_all_tiers_unlocked, get_effective_tiers, get_enabled_tiers, get_max_unlocked_tier};

/// SQL fragment for selecting card data from joined tables
/// Uses app.card_definitions for content and card_progress for SRS state
const CARD_SELECT: &str = r#"
  cd.id, cd.front, cd.main_answer, cd.description, cd.card_type, cd.tier,
  cd.audio_hint, cd.is_reverse, cd.pack_id, cd.lesson,
  COALESCE(cp.ease_factor, 2.5) as ease_factor,
  COALESCE(cp.interval_days, 0) as interval_days,
  COALESCE(cp.repetitions, 0) as repetitions,
  COALESCE(cp.next_review, datetime('now')) as next_review,
  COALESCE(cp.total_reviews, 0) as total_reviews,
  COALESCE(cp.correct_reviews, 0) as correct_reviews,
  COALESCE(cp.learning_step, 0) as learning_step,
  cp.fsrs_stability, cp.fsrs_difficulty, cp.fsrs_state
"#;

/// SQL fragment for FROM clause with JOIN
const CARD_FROM: &str = r#"
FROM app.card_definitions cd
LEFT JOIN card_progress cp ON cp.card_id = cd.id
"#;

pub fn insert_card(conn: &Connection, card: &Card) -> Result<i64> {
    conn.execute(
        r#"
    INSERT INTO cards (front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
                       interval_days, repetitions, next_review, total_reviews, correct_reviews)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
    "#,
        params![
            card.front,
            card.main_answer,
            card.description,
            card.card_type.as_str(),
            card.tier,
            card.audio_hint,
            card.is_reverse,
            card.ease_factor,
            card.interval_days,
            card.repetitions,
            card.next_review.to_rfc3339(),
            card.total_reviews,
            card.correct_reviews,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_card_by_id(conn: &Connection, id: i64) -> Result<Option<Card>> {
    let query = format!(
        "SELECT {} {} WHERE cd.id = ?1",
        CARD_SELECT, CARD_FROM
    );
    let mut stmt = conn.prepare(&query)?;

    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_card(row)?))
    } else {
        Ok(None)
    }
}

pub fn get_due_cards(
    conn: &Connection,
    limit: usize,
    exclude_sibling_of: Option<i64>,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select".into(),
        table: "cards".into(),
    });

    let now = Utc::now().to_rfc3339();
    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(vec![]);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    if let Some(last_id) = exclude_sibling_of {
        if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
            let query = format!(
                r#"SELECT {} {}
                WHERE COALESCE(cp.next_review, datetime('now')) <= ?1 AND cd.tier IN ({})
                  AND cd.id != ?2
                  AND cd.main_answer != ?3
                  AND cd.front NOT LIKE '%' || ?4 || '%'
                ORDER BY cd.tier ASC, COALESCE(cp.next_review, datetime('now')) ASC
                LIMIT ?5"#,
                CARD_SELECT, CARD_FROM, tier_list
            );
            let mut stmt = conn.prepare(&query)?;

            let cards = stmt
                .query_map(
                    params![now, last_id, last_card.front, last_card.main_answer, limit as i64],
                    |row| row_to_card(row),
                )?
                .collect::<Result<Vec<_>>>()?;
            return Ok(cards);
        }
    }

    let query = format!(
        r#"SELECT {} {}
        WHERE COALESCE(cp.next_review, datetime('now')) <= ?1 AND cd.tier IN ({})
        ORDER BY cd.tier ASC, COALESCE(cp.next_review, datetime('now')) ASC
        LIMIT ?2"#,
        CARD_SELECT, CARD_FROM, tier_list
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![now, limit as i64], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

pub fn get_due_count(conn: &Connection) -> Result<i64> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "count".into(),
        table: "cards".into(),
    });

    let now = Utc::now().to_rfc3339();
    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(0);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        r#"SELECT COUNT(*) {}
        WHERE COALESCE(cp.next_review, datetime('now')) <= ?1 AND cd.tier IN ({})"#,
        CARD_FROM, tier_list
    );
    conn.query_row(&query, params![now], |row| row.get(0))
}

pub fn get_next_review_time(conn: &Connection) -> Result<Option<DateTime<Utc>>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_min".into(),
        table: "cards".into(),
    });

    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(None);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        r#"SELECT MIN(COALESCE(cp.next_review, datetime('now'))) {}
        WHERE cd.tier IN ({})"#,
        CARD_FROM, tier_list
    );
    let result: Option<String> = conn.query_row(&query, [], |row| row.get(0))?;

    Ok(result.and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }))
}

/// Get the next upcoming review time (only cards not yet due)
pub fn get_next_upcoming_review_time(conn: &Connection) -> Result<Option<DateTime<Utc>>> {
    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(None);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let now = Utc::now().to_rfc3339();
    let query = format!(
        r#"SELECT MIN(COALESCE(cp.next_review, datetime('now'))) {}
        WHERE cd.tier IN ({}) AND COALESCE(cp.next_review, datetime('now')) > ?1"#,
        CARD_FROM, tier_list
    );
    let result: Option<String> = conn.query_row(&query, params![now], |row| row.get(0))?;

    Ok(result.and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }))
}

pub fn get_due_cards_interleaved(
    conn: &Connection,
    limit: usize,
    exclude_sibling_of: Option<i64>,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_interleaved".into(),
        table: "cards".into(),
    });

    let now = Utc::now().to_rfc3339();
    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(vec![]);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let exclude_clause = if let Some(last_id) = exclude_sibling_of {
        if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
            format!(
                "AND cd.id != {} AND cd.main_answer != '{}' AND cd.front NOT LIKE '%{}%'",
                last_id,
                last_card.front.replace('\'', "''"),
                last_card.main_answer.replace('\'', "''")
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let query = format!(
        r#"SELECT {} {}
        WHERE COALESCE(cp.next_review, datetime('now')) <= ?1 AND cd.tier IN ({})
        {}
        ORDER BY cd.card_type, RANDOM()
        LIMIT ?2"#,
        CARD_SELECT, CARD_FROM, tier_list, exclude_clause
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![now, limit as i64], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

pub fn get_practice_cards(
    conn: &Connection,
    limit: usize,
    exclude_id: Option<i64>,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_practice".into(),
        table: "cards".into(),
    });

    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(vec![]);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    if let Some(last_id) = exclude_id {
        if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
            let query = format!(
                r#"SELECT {} {}
                WHERE cd.tier IN ({})
                  AND cd.id != ?1
                  AND cd.main_answer != ?2
                  AND cd.front NOT LIKE '%' || ?3 || '%'
                ORDER BY RANDOM()
                LIMIT ?4"#,
                CARD_SELECT, CARD_FROM, tier_list
            );
            let mut stmt = conn.prepare(&query)?;

            let cards = stmt
                .query_map(
                    params![last_id, last_card.front, last_card.main_answer, limit as i64],
                    |row| row_to_card(row),
                )?
                .collect::<Result<Vec<_>>>()?;
            return Ok(cards);
        }
    }

    let query = format!(
        r#"SELECT {} {}
        WHERE cd.tier IN ({})
        ORDER BY RANDOM()
        LIMIT ?1"#,
        CARD_SELECT, CARD_FROM, tier_list
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![limit as i64], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

pub fn get_unlocked_cards(conn: &Connection) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_unlocked".into(),
        table: "cards".into(),
    });

    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(vec![]);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        r#"SELECT {} {}
        WHERE cd.tier IN ({})
        ORDER BY cd.tier ASC, cd.id ASC"#,
        CARD_SELECT, CARD_FROM, tier_list
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map([], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

/// Get all unlocked cards ignoring focus mode (for library/reference pages)
pub fn get_all_unlocked_cards(conn: &Connection) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_all_unlocked".into(),
        table: "cards".into(),
    });

    let all_unlocked = get_all_tiers_unlocked(conn)?;
    let tiers: Vec<u8> = if all_unlocked {
        get_enabled_tiers(conn)?
    } else {
        let max_tier = get_max_unlocked_tier(conn)?;
        (1..=max_tier).collect()
    };

    if tiers.is_empty() {
        return Ok(vec![]);
    }

    let tier_list = tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        r#"SELECT {} {}
        WHERE cd.tier IN ({})
        ORDER BY cd.tier ASC, cd.id ASC"#,
        CARD_SELECT, CARD_FROM, tier_list
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map([], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

/// Get cards from the same pack and lesson (for generating vocabulary choices)
pub fn get_cards_from_same_lesson(
    conn: &Connection,
    pack_id: &str,
    lesson: u8,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_same_lesson".into(),
        table: "cards".into(),
    });

    let query = format!(
        r#"SELECT {} {}
        WHERE cd.pack_id = ?1 AND cd.lesson = ?2
        ORDER BY cd.id ASC"#,
        CARD_SELECT, CARD_FROM
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![pack_id, lesson], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

pub fn get_unreviewed_today(
    conn: &Connection,
    limit: usize,
    exclude_sibling_of: Option<i64>,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_unreviewed".into(),
        table: "cards".into(),
    });

    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(vec![]);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();

    let exclude_clause = if let Some(last_id) = exclude_sibling_of {
        if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
            format!(
                "AND cd.id != {} AND cd.main_answer != '{}' AND cd.front NOT LIKE '%{}%'",
                last_id,
                last_card.front.replace('\'', "''"),
                last_card.main_answer.replace('\'', "''")
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let query = format!(
        r#"SELECT {} {}
        WHERE cd.tier IN ({})
          AND NOT EXISTS (
            SELECT 1 FROM review_logs r
            WHERE r.card_id = cd.id AND r.reviewed_at >= ?1
          )
          {}
        ORDER BY cd.tier ASC, RANDOM()
        LIMIT ?2"#,
        CARD_SELECT, CARD_FROM, tier_list, exclude_clause
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![today_start, limit as i64], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

pub fn get_unreviewed_today_count(conn: &Connection) -> Result<i64> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "count_unreviewed".into(),
        table: "cards".into(),
    });

    let effective_tiers = get_effective_tiers(conn)?;

    if effective_tiers.is_empty() {
        return Ok(0);
    }

    let tier_list = effective_tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();

    let query = format!(
        r#"SELECT COUNT(*) {}
        WHERE cd.tier IN ({})
          AND NOT EXISTS (
            SELECT 1 FROM review_logs r
            WHERE r.card_id = cd.id AND r.reviewed_at >= ?1
          )"#,
        CARD_FROM, tier_list
    );
    conn.query_row(&query, params![today_start], |row| row.get(0))
}

pub fn get_cards_by_tier(conn: &Connection, tier: u8) -> Result<Vec<Card>> {
    let query = format!(
        r#"SELECT {} {}
        WHERE cd.tier = ?1
        ORDER BY cd.id ASC"#,
        CARD_SELECT, CARD_FROM
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![tier], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

pub fn update_card_after_review(
    conn: &Connection,
    card_id: i64,
    ease_factor: f64,
    interval_days: i64,
    repetitions: i64,
    next_review: DateTime<Utc>,
    learning_step: i64,
    correct: bool,
) -> Result<()> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "update".into(),
        table: "card_progress".into(),
    });

    // Use INSERT OR REPLACE to create progress if it doesn't exist
    conn.execute(
        r#"
        INSERT INTO card_progress (card_id, ease_factor, interval_days, repetitions, next_review,
                                   learning_step, total_reviews, correct_reviews)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)
        ON CONFLICT(card_id) DO UPDATE SET
            ease_factor = ?2,
            interval_days = ?3,
            repetitions = ?4,
            next_review = ?5,
            learning_step = ?6,
            total_reviews = total_reviews + 1,
            correct_reviews = correct_reviews + ?7
        "#,
        params![
            card_id,
            ease_factor,
            interval_days,
            repetitions,
            next_review.to_rfc3339(),
            learning_step,
            if correct { 1 } else { 0 },
        ],
    )?;
    Ok(())
}

pub fn update_card_after_fsrs_review(
    conn: &Connection,
    card_id: i64,
    next_review: DateTime<Utc>,
    stability: f64,
    difficulty: f64,
    state: FsrsState,
    learning_step: i64,
    repetitions: i64,
    correct: bool,
) -> Result<()> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "update_fsrs".into(),
        table: "card_progress".into(),
    });

    // Use INSERT OR REPLACE to create progress if it doesn't exist
    conn.execute(
        r#"
        INSERT INTO card_progress (card_id, next_review, fsrs_stability, fsrs_difficulty, fsrs_state,
                                   learning_step, repetitions, total_reviews, correct_reviews,
                                   ease_factor, interval_days)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, 2.5, 0)
        ON CONFLICT(card_id) DO UPDATE SET
            next_review = ?2,
            fsrs_stability = ?3,
            fsrs_difficulty = ?4,
            fsrs_state = ?5,
            learning_step = ?6,
            repetitions = ?7,
            total_reviews = total_reviews + 1,
            correct_reviews = correct_reviews + ?8
        "#,
        params![
            card_id,
            next_review.to_rfc3339(),
            stability,
            difficulty,
            state.as_str(),
            learning_step,
            repetitions,
            if correct { 1 } else { 0 },
        ],
    )?;
    Ok(())
}

/// Convert a database row to a Card struct
/// Column order: id(0), front(1), main_answer(2), description(3), card_type(4), tier(5),
///               audio_hint(6), is_reverse(7), pack_id(8), lesson(9),
///               ease_factor(10), interval_days(11), repetitions(12), next_review(13),
///               total_reviews(14), correct_reviews(15), learning_step(16),
///               fsrs_stability(17), fsrs_difficulty(18), fsrs_state(19)
pub(crate) fn row_to_card(row: &rusqlite::Row) -> Result<Card> {
    let card_type_str: String = row.get(4)?;
    let is_reverse_int: i64 = row.get(7)?;
    let lesson_int: Option<i64> = row.get(9)?;
    let next_review_str: String = row.get(13)?;
    let fsrs_state_str: Option<String> = row.get(19)?;

    Ok(Card {
        id: row.get(0)?,
        front: row.get(1)?,
        main_answer: row.get(2)?,
        description: row.get(3)?,
        card_type: CardType::from_str(&card_type_str).unwrap_or(CardType::Consonant),
        tier: row.get(5)?,
        audio_hint: row.get(6)?,
        is_reverse: is_reverse_int != 0,
        pack_id: row.get(8)?,
        lesson: lesson_int.map(|l| l as u8),
        ease_factor: row.get(10)?,
        interval_days: row.get(11)?,
        repetitions: row.get(12)?,
        next_review: DateTime::parse_from_rfc3339(&next_review_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        total_reviews: row.get(14)?,
        correct_reviews: row.get(15)?,
        learning_step: row.get(16)?,
        fsrs_stability: row.get(17)?,
        fsrs_difficulty: row.get(18)?,
        fsrs_state: fsrs_state_str.map(|s| FsrsState::from_str(&s)),
    })
}

// ==================== Filtered Card Selection ====================
// These functions accept a StudyFilterMode to filter cards by pack/lesson

/// Get due cards with study filter applied
pub fn get_due_cards_filtered(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
    limit: usize,
    exclude_sibling_of: Option<i64>,
    filter: &StudyFilterMode,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_filtered".into(),
        table: "cards".into(),
    });

    let now = Utc::now().to_rfc3339();

    // Build filter clause - returns (clause, params, skip_tier_filter)
    let (filter_clause, _, skip_tier_filter) = build_filter_where_clause(conn, app_conn, user_id, filter)?;

    // Build tier clause - only apply if not skipping
    let tier_clause = if skip_tier_filter {
        String::new()
    } else {
        let effective_tiers = get_effective_tiers(conn)?;
        if effective_tiers.is_empty() {
            return Ok(vec![]);
        }
        let tier_list = effective_tiers
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("AND cd.tier IN ({})", tier_list)
    };

    if let Some(last_id) = exclude_sibling_of {
        if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
            let query = format!(
                r#"SELECT {} {}
                WHERE COALESCE(cp.next_review, datetime('now')) <= ?1
                  {}
                  AND cd.id != ?2
                  AND cd.main_answer != ?3
                  AND cd.front NOT LIKE '%' || ?4 || '%'
                  {}
                ORDER BY cd.tier ASC, COALESCE(cp.next_review, datetime('now')) ASC
                LIMIT ?5"#,
                CARD_SELECT, CARD_FROM, tier_clause, filter_clause
            );
            let mut stmt = conn.prepare(&query)?;

            let cards = stmt
                .query_map(
                    params![now, last_id, last_card.front, last_card.main_answer, limit as i64],
                    |row| row_to_card(row),
                )?
                .collect::<Result<Vec<_>>>()?;
            return Ok(cards);
        }
    }

    let query = format!(
        r#"SELECT {} {}
        WHERE COALESCE(cp.next_review, datetime('now')) <= ?1
        {} {}
        ORDER BY cd.tier ASC, COALESCE(cp.next_review, datetime('now')) ASC
        LIMIT ?2"#,
        CARD_SELECT, CARD_FROM, tier_clause, filter_clause
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![now, limit as i64], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

/// Get due card count with study filter applied
pub fn get_due_count_filtered(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
    filter: &StudyFilterMode,
) -> Result<i64> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "count_filtered".into(),
        table: "cards".into(),
    });

    let now = Utc::now().to_rfc3339();
    let (filter_clause, _, skip_tier_filter) = build_filter_where_clause(conn, app_conn, user_id, filter)?;

    let tier_clause = if skip_tier_filter {
        String::new()
    } else {
        let effective_tiers = get_effective_tiers(conn)?;
        if effective_tiers.is_empty() {
            return Ok(0);
        }
        let tier_list = effective_tiers
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("AND cd.tier IN ({})", tier_list)
    };

    let query = format!(
        r#"SELECT COUNT(*) {}
        WHERE COALESCE(cp.next_review, datetime('now')) <= ?1
        {} {}"#,
        CARD_FROM, tier_clause, filter_clause
    );
    conn.query_row(&query, params![now], |row| row.get(0))
}

/// Get due cards interleaved with study filter applied
pub fn get_due_cards_interleaved_filtered(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
    limit: usize,
    exclude_sibling_of: Option<i64>,
    filter: &StudyFilterMode,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_interleaved_filtered".into(),
        table: "cards".into(),
    });

    let now = Utc::now().to_rfc3339();
    let (filter_clause, _, skip_tier_filter) = build_filter_where_clause(conn, app_conn, user_id, filter)?;

    let tier_clause = if skip_tier_filter {
        String::new()
    } else {
        let effective_tiers = get_effective_tiers(conn)?;
        if effective_tiers.is_empty() {
            return Ok(vec![]);
        }
        let tier_list = effective_tiers
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("AND cd.tier IN ({})", tier_list)
    };

    let exclude_clause = if let Some(last_id) = exclude_sibling_of {
        if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
            format!(
                "AND cd.id != {} AND cd.main_answer != '{}' AND cd.front NOT LIKE '%{}%'",
                last_id,
                last_card.front.replace('\'', "''"),
                last_card.main_answer.replace('\'', "''")
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let query = format!(
        r#"SELECT {} {}
        WHERE COALESCE(cp.next_review, datetime('now')) <= ?1
        {} {} {}
        ORDER BY cd.card_type, RANDOM()
        LIMIT ?2"#,
        CARD_SELECT, CARD_FROM, tier_clause, filter_clause, exclude_clause
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![now, limit as i64], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

/// Get unreviewed today cards with study filter applied
pub fn get_unreviewed_today_filtered(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
    limit: usize,
    exclude_sibling_of: Option<i64>,
    filter: &StudyFilterMode,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_unreviewed_filtered".into(),
        table: "cards".into(),
    });

    let (filter_clause, _, skip_tier_filter) = build_filter_where_clause(conn, app_conn, user_id, filter)?;

    let tier_clause = if skip_tier_filter {
        String::new()
    } else {
        let effective_tiers = get_effective_tiers(conn)?;
        if effective_tiers.is_empty() {
            return Ok(vec![]);
        }
        let tier_list = effective_tiers
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("AND cd.tier IN ({})", tier_list)
    };

    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();

    let exclude_clause = if let Some(last_id) = exclude_sibling_of {
        if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
            format!(
                "AND cd.id != {} AND cd.main_answer != '{}' AND cd.front NOT LIKE '%{}%'",
                last_id,
                last_card.front.replace('\'', "''"),
                last_card.main_answer.replace('\'', "''")
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let query = format!(
        r#"SELECT {} {}
        WHERE 1=1
          {}
          AND NOT EXISTS (
            SELECT 1 FROM review_logs r
            WHERE r.card_id = cd.id AND r.reviewed_at >= ?1
          )
          {} {}
        ORDER BY cd.tier ASC, RANDOM()
        LIMIT ?2"#,
        CARD_SELECT, CARD_FROM, tier_clause, filter_clause, exclude_clause
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![today_start, limit as i64], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

/// Get unreviewed today count with study filter applied
pub fn get_unreviewed_today_count_filtered(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
    filter: &StudyFilterMode,
) -> Result<i64> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "count_unreviewed_filtered".into(),
        table: "cards".into(),
    });

    let (filter_clause, _, skip_tier_filter) = build_filter_where_clause(conn, app_conn, user_id, filter)?;

    let tier_clause = if skip_tier_filter {
        String::new()
    } else {
        let effective_tiers = get_effective_tiers(conn)?;
        if effective_tiers.is_empty() {
            return Ok(0);
        }
        let tier_list = effective_tiers
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("AND cd.tier IN ({})", tier_list)
    };

    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();

    let query = format!(
        r#"SELECT COUNT(*) {}
        WHERE 1=1
          {}
          AND NOT EXISTS (
            SELECT 1 FROM review_logs r
            WHERE r.card_id = cd.id AND r.reviewed_at >= ?1
          )
          {}"#,
        CARD_FROM, tier_clause, filter_clause
    );
    conn.query_row(&query, params![today_start], |row| row.get(0))
}

/// Get practice cards with study filter applied
pub fn get_practice_cards_filtered(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
    limit: usize,
    exclude_id: Option<i64>,
    filter: &StudyFilterMode,
) -> Result<Vec<Card>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_practice_filtered".into(),
        table: "cards".into(),
    });

    let (filter_clause, _, skip_tier_filter) = build_filter_where_clause(conn, app_conn, user_id, filter)?;

    // Build tier clause conditionally
    let tier_clause = if skip_tier_filter {
        String::new()
    } else {
        let effective_tiers = get_effective_tiers(conn)?;
        if effective_tiers.is_empty() {
            return Ok(vec![]);
        }
        let tier_list = effective_tiers
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("AND cd.tier IN ({})", tier_list)
    };

    if let Some(last_id) = exclude_id {
        if let Ok(Some(last_card)) = get_card_by_id(conn, last_id) {
            let query = format!(
                r#"SELECT {} {}
                WHERE cd.id != ?1
                  AND cd.main_answer != ?2
                  AND cd.front NOT LIKE '%' || ?3 || '%'
                  {} {}
                ORDER BY RANDOM()
                LIMIT ?4"#,
                CARD_SELECT, CARD_FROM, tier_clause, filter_clause
            );
            let mut stmt = conn.prepare(&query)?;

            let cards = stmt
                .query_map(
                    params![last_id, last_card.front, last_card.main_answer, limit as i64],
                    |row| row_to_card(row),
                )?
                .collect::<Result<Vec<_>>>()?;
            return Ok(cards);
        }
    }

    let query = format!(
        r#"SELECT {} {}
        WHERE 1=1 {} {}
        ORDER BY RANDOM()
        LIMIT ?1"#,
        CARD_SELECT, CARD_FROM, tier_clause, filter_clause
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map(params![limit as i64], |row| row_to_card(row))?
        .collect::<Result<Vec<_>>>()?;
    Ok(cards)
}

/// Get count of cards accessible to the user (Hangul + enabled packs with permission)
/// This respects lesson unlock status - only counts cards user can currently study
pub fn get_accessible_card_count(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
) -> Result<(i64, i64)> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "count_accessible".into(),
        table: "cards".into(),
    });

    // Use All mode to get accessible cards count
    let filter = StudyFilterMode::All;
    let (filter_clause, _, _) = build_filter_where_clause(conn, app_conn, user_id, &filter)?;

    // Count total accessible cards
    let query = format!(
        r#"SELECT COUNT(*) {}
        WHERE 1=1 {}"#,
        CARD_FROM, filter_clause
    );
    let total_cards: i64 = conn.query_row(&query, [], |row| row.get(0))?;

    // Count learned cards (repetitions >= 2)
    let query_learned = format!(
        r#"SELECT COUNT(*) {}
        WHERE COALESCE(cp.repetitions, 0) >= 2 {}"#,
        CARD_FROM, filter_clause
    );
    let cards_learned: i64 = conn.query_row(&query_learned, [], |row| row.get(0))?;

    Ok((total_cards, cards_learned))
}

/// Get count of ALL cards available to the user (ignores lesson unlock status)
/// This counts all cards from packs user has permission for, regardless of lesson progress
pub fn get_available_card_count(app_conn: &Connection, user_id: i64) -> Result<i64> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "count_available".into(),
        table: "card_definitions".into(),
    });

    // Get all packs user has permission to access
    let accessible_packs = crate::auth::db::list_accessible_pack_ids(app_conn, user_id)
        .unwrap_or_default();

    if accessible_packs.is_empty() {
        // Only Hangul baseline cards (pack_id IS NULL)
        let count: i64 = app_conn.query_row(
            "SELECT COUNT(*) FROM card_definitions WHERE pack_id IS NULL",
            [],
            |row| row.get(0),
        )?;
        return Ok(count);
    }

    // Build pack list for IN clause
    let pack_placeholders: Vec<String> = accessible_packs
        .iter()
        .map(|p| format!("'{}'", p.replace('\'', "''")))
        .collect();
    let pack_list = pack_placeholders.join(",");

    // Count Hangul cards (pack_id IS NULL) + all cards from accessible packs
    let query = format!(
        "SELECT COUNT(*) FROM card_definitions WHERE pack_id IS NULL OR pack_id IN ({})",
        pack_list
    );
    let count: i64 = app_conn.query_row(&query, [], |row| row.get(0))?;

    Ok(count)
}
