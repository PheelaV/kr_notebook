//! Card CRUD and query operations

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};

use crate::domain::{Card, CardType, FsrsState};
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

use super::tiers::{get_all_tiers_unlocked, get_effective_tiers, get_enabled_tiers, get_max_unlocked_tier};

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
    let mut stmt = conn.prepare(
        r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards WHERE id = ?1
    "#,
    )?;

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
                r#"
        SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
               interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
               fsrs_stability, fsrs_difficulty, fsrs_state
        FROM cards
        WHERE next_review <= ?1 AND tier IN ({})
          AND id != ?2
          AND main_answer != ?3
          AND front NOT LIKE '%' || ?4 || '%'
        ORDER BY tier ASC, next_review ASC
        LIMIT ?5
        "#,
                tier_list
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
        r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE next_review <= ?1 AND tier IN ({})
    ORDER BY tier ASC, next_review ASC
    LIMIT ?2
    "#,
        tier_list
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
        "SELECT COUNT(*) FROM cards WHERE next_review <= ?1 AND tier IN ({})",
        tier_list
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
        "SELECT MIN(next_review) FROM cards WHERE tier IN ({})",
        tier_list
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
        "SELECT MIN(next_review) FROM cards WHERE tier IN ({}) AND next_review > ?1",
        tier_list
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
                "AND id != {} AND main_answer != '{}' AND front NOT LIKE '%{}%'",
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
        r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE next_review <= ?1 AND tier IN ({})
    {}
    ORDER BY card_type, RANDOM()
    LIMIT ?2
    "#,
        tier_list, exclude_clause
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
                r#"
        SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
               interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
               fsrs_stability, fsrs_difficulty, fsrs_state
        FROM cards
        WHERE tier IN ({})
          AND id != ?1
          AND main_answer != ?2
          AND front NOT LIKE '%' || ?3 || '%'
        ORDER BY RANDOM()
        LIMIT ?4
        "#,
                tier_list
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
        r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE tier IN ({})
    ORDER BY RANDOM()
    LIMIT ?1
    "#,
        tier_list
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
        r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE tier IN ({})
    ORDER BY tier ASC, id ASC
    "#,
        tier_list
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
        r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE tier IN ({})
    ORDER BY tier ASC, id ASC
    "#,
        tier_list
    );
    let mut stmt = conn.prepare(&query)?;

    let cards = stmt
        .query_map([], |row| row_to_card(row))?
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
                "AND c.id != {} AND c.main_answer != '{}' AND c.front NOT LIKE '%{}%'",
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
        r#"
    SELECT c.id, c.front, c.main_answer, c.description, c.card_type, c.tier, c.audio_hint, c.is_reverse,
           c.ease_factor, c.interval_days, c.repetitions, c.next_review, c.total_reviews,
           c.correct_reviews, c.learning_step, c.fsrs_stability, c.fsrs_difficulty, c.fsrs_state
    FROM cards c
    WHERE c.tier IN ({})
      AND NOT EXISTS (
        SELECT 1 FROM review_logs r
        WHERE r.card_id = c.id AND r.reviewed_at >= ?1
      )
      {}
    ORDER BY c.tier ASC, RANDOM()
    LIMIT ?2
    "#,
        tier_list, exclude_clause
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
        r#"
    SELECT COUNT(*)
    FROM cards c
    WHERE c.tier IN ({})
      AND NOT EXISTS (
        SELECT 1 FROM review_logs r
        WHERE r.card_id = c.id AND r.reviewed_at >= ?1
      )
    "#,
        tier_list
    );
    conn.query_row(&query, params![today_start], |row| row.get(0))
}

pub fn get_cards_by_tier(conn: &Connection, tier: u8) -> Result<Vec<Card>> {
    let mut stmt = conn.prepare(
        r#"
    SELECT id, front, main_answer, description, card_type, tier, audio_hint, is_reverse, ease_factor,
           interval_days, repetitions, next_review, total_reviews, correct_reviews, learning_step,
           fsrs_stability, fsrs_difficulty, fsrs_state
    FROM cards
    WHERE tier = ?1
    ORDER BY id ASC
    "#,
    )?;

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
        table: "cards".into(),
    });

    conn.execute(
        r#"
    UPDATE cards
    SET ease_factor = ?1, interval_days = ?2, repetitions = ?3, next_review = ?4,
        learning_step = ?5, total_reviews = total_reviews + 1,
        correct_reviews = correct_reviews + ?6
    WHERE id = ?7
    "#,
        params![
            ease_factor,
            interval_days,
            repetitions,
            next_review.to_rfc3339(),
            learning_step,
            if correct { 1 } else { 0 },
            card_id,
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
        table: "cards".into(),
    });

    conn.execute(
        r#"
    UPDATE cards
    SET next_review = ?1, fsrs_stability = ?2, fsrs_difficulty = ?3, fsrs_state = ?4,
        learning_step = ?5, repetitions = ?6,
        total_reviews = total_reviews + 1, correct_reviews = correct_reviews + ?7
    WHERE id = ?8
    "#,
        params![
            next_review.to_rfc3339(),
            stability,
            difficulty,
            state.as_str(),
            learning_step,
            repetitions,
            if correct { 1 } else { 0 },
            card_id,
        ],
    )?;
    Ok(())
}

/// Convert a database row to a Card struct
pub(crate) fn row_to_card(row: &rusqlite::Row) -> Result<Card> {
    let card_type_str: String = row.get(4)?;
    let is_reverse_int: i64 = row.get(7)?;
    let next_review_str: String = row.get(11)?;
    let fsrs_state_str: Option<String> = row.get(17)?;

    Ok(Card {
        id: row.get(0)?,
        front: row.get(1)?,
        main_answer: row.get(2)?,
        description: row.get(3)?,
        card_type: CardType::from_str(&card_type_str).unwrap_or(CardType::Consonant),
        tier: row.get(5)?,
        audio_hint: row.get(6)?,
        is_reverse: is_reverse_int != 0,
        ease_factor: row.get(8)?,
        interval_days: row.get(9)?,
        repetitions: row.get(10)?,
        next_review: DateTime::parse_from_rfc3339(&next_review_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        total_reviews: row.get(12)?,
        correct_reviews: row.get(13)?,
        learning_step: row.get(14)?,
        fsrs_stability: row.get(15)?,
        fsrs_difficulty: row.get(16)?,
        fsrs_state: fsrs_state_str.map(|s| FsrsState::from_str(&s)),
    })
}
