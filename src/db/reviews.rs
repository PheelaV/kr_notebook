//! Review logging and confusion tracking

use chrono::Utc;
use rusqlite::{params, Connection, Result};

use crate::domain::{ReviewDirection, ReviewLog, StudyMode};

/// Pre-review card state for backup/restore on override
#[derive(Debug, Clone, Default)]
pub struct PreReviewState {
    pub next_review: Option<String>,
    pub learning_step: Option<i64>,
    pub repetitions: Option<i64>,
    pub fsrs_stability: Option<f64>,
    pub fsrs_difficulty: Option<f64>,
    pub fsrs_state: Option<String>,
}
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

pub fn insert_review_log(conn: &Connection, log: &ReviewLog) -> Result<i64> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "insert".into(),
        table: "review_logs".into(),
    });

    conn.execute(
        "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (?1, ?2, ?3)",
        params![log.card_id, log.quality, log.reviewed_at.to_rfc3339()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert a review log with enhanced tracking fields
pub fn insert_review_log_enhanced(
    conn: &Connection,
    card_id: i64,
    quality: u8,
    is_correct: bool,
    study_mode: StudyMode,
    direction: ReviewDirection,
    response_time_ms: Option<i64>,
    hints_used: i32,
) -> Result<i64> {
    insert_review_log_with_pre_state(
        conn,
        card_id,
        quality,
        is_correct,
        study_mode,
        direction,
        response_time_ms,
        hints_used,
        None,
    )
}

/// Insert a review log with pre-review state for override restoration
pub fn insert_review_log_with_pre_state(
    conn: &Connection,
    card_id: i64,
    quality: u8,
    is_correct: bool,
    study_mode: StudyMode,
    direction: ReviewDirection,
    response_time_ms: Option<i64>,
    hints_used: i32,
    pre_state: Option<&PreReviewState>,
) -> Result<i64> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "insert_enhanced".into(),
        table: "review_logs".into(),
    });

    let now = Utc::now().to_rfc3339();

    let (pre_next_review, pre_learning_step, pre_repetitions, pre_fsrs_stability, pre_fsrs_difficulty, pre_fsrs_state) =
        match pre_state {
            Some(s) => (
                s.next_review.as_deref(),
                s.learning_step,
                s.repetitions,
                s.fsrs_stability,
                s.fsrs_difficulty,
                s.fsrs_state.as_deref(),
            ),
            None => (None, None, None, None, None, None),
        };

    conn.execute(
        r#"
    INSERT INTO review_logs (
        card_id, quality, reviewed_at, is_correct, study_mode, direction, response_time_ms, hints_used,
        pre_review_next_review, pre_review_learning_step, pre_review_repetitions,
        pre_review_fsrs_stability, pre_review_fsrs_difficulty, pre_review_fsrs_state
    )
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
    "#,
        params![
            card_id,
            quality,
            now,
            if is_correct { 1 } else { 0 },
            study_mode.as_str(),
            direction.as_str(),
            response_time_ms,
            hints_used,
            pre_next_review,
            pre_learning_step,
            pre_repetitions,
            pre_fsrs_stability,
            pre_fsrs_difficulty,
            pre_fsrs_state,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get the most recent review log for a card with pre-review state
pub fn get_latest_review_pre_state(conn: &Connection, card_id: i64) -> Result<Option<PreReviewState>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_latest".into(),
        table: "review_logs".into(),
    });

    let result = conn.query_row(
        r#"
        SELECT pre_review_next_review, pre_review_learning_step, pre_review_repetitions,
               pre_review_fsrs_stability, pre_review_fsrs_difficulty, pre_review_fsrs_state
        FROM review_logs
        WHERE card_id = ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
        params![card_id],
        |row| {
            Ok(PreReviewState {
                next_review: row.get(0)?,
                learning_step: row.get(1)?,
                repetitions: row.get(2)?,
                fsrs_stability: row.get(3)?,
                fsrs_difficulty: row.get(4)?,
                fsrs_state: row.get(5)?,
            })
        },
    );

    match result {
        Ok(state) => {
            // Only return if we have actual pre-review data
            if state.next_review.is_some() {
                Ok(Some(state))
            } else {
                Ok(None)
            }
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Get the timestamp of the most recent non-override review for a card
/// Used for override timing: SRS should be calculated from original attempt time
pub fn get_latest_review_time(conn: &Connection, card_id: i64) -> Result<Option<chrono::DateTime<Utc>>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_latest_time".into(),
        table: "review_logs".into(),
    });

    let result = conn.query_row(
        r#"
        SELECT reviewed_at
        FROM review_logs
        WHERE card_id = ?1 AND study_mode != 'override'
        ORDER BY id DESC
        LIMIT 1
        "#,
        params![card_id],
        |row| {
            let reviewed_at_str: String = row.get(0)?;
            Ok(reviewed_at_str)
        },
    );

    match result {
        Ok(ts_str) => {
            match chrono::DateTime::parse_from_rfc3339(&ts_str) {
                Ok(dt) => Ok(Some(dt.with_timezone(&Utc))),
                Err(_) => Ok(None),
            }
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Update the quality and is_correct of the most recent review log for a card
/// Used for override ruling to correct the existing log rather than adding a new one
pub fn update_latest_review_quality(
    conn: &Connection,
    card_id: i64,
    quality: u8,
    is_correct: bool,
) -> Result<usize> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "update_quality".into(),
        table: "review_logs".into(),
    });

    // Update the most recent non-override review for this card
    let updated = conn.execute(
        r#"
        UPDATE review_logs
        SET quality = ?2, is_correct = ?3
        WHERE id = (
            SELECT id FROM review_logs
            WHERE card_id = ?1 AND study_mode != 'override'
            ORDER BY id DESC
            LIMIT 1
        )
        "#,
        params![card_id, quality, if is_correct { 1 } else { 0 }],
    )?;

    Ok(updated)
}

/// Record a confusion (wrong answer) for analysis
pub fn record_confusion(conn: &Connection, card_id: i64, wrong_answer: &str) -> Result<()> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "upsert".into(),
        table: "confusions".into(),
    });

    let now = Utc::now().to_rfc3339();

    // Try to update existing confusion first
    let updated = conn.execute(
        r#"
    UPDATE confusions
    SET count = count + 1, last_confused_at = ?1
    WHERE card_id = ?2 AND wrong_answer = ?3
    "#,
        params![now, card_id, wrong_answer],
    )?;

    // If no existing row, insert new one
    if updated == 0 {
        conn.execute(
            r#"
      INSERT INTO confusions (card_id, wrong_answer, count, last_confused_at)
      VALUES (?1, ?2, 1, ?3)
      "#,
            params![card_id, wrong_answer, now],
        )?;
    }

    Ok(())
}

/// Get top confusions for a card
pub fn get_card_confusions(
    conn: &Connection,
    card_id: i64,
    limit: usize,
) -> Result<Vec<(String, i64)>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select".into(),
        table: "confusions".into(),
    });

    let mut stmt = conn.prepare(
        r#"
    SELECT wrong_answer, count
    FROM confusions
    WHERE card_id = ?1
    ORDER BY count DESC
    LIMIT ?2
    "#,
    )?;

    let confusions = stmt
        .query_map(params![card_id, limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(confusions)
}

/// Raw problem card data from database
pub struct ProblemCardRaw {
    pub id: i64,
    pub front: String,
    pub main_answer: String,
    pub is_reverse: bool,
    pub confusion_count: i64,
}

/// Get cards with most confusions (problem cards)
pub fn get_problem_cards(conn: &Connection, limit: usize) -> Result<Vec<ProblemCardRaw>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_problem".into(),
        table: "confusions".into(),
    });

    let mut stmt = conn.prepare(
        r#"
    SELECT c.card_id, cd.front, cd.main_answer, cd.is_reverse, SUM(c.count) as total_confusions
    FROM confusions c
    JOIN app.card_definitions cd ON c.card_id = cd.id
    GROUP BY c.card_id
    ORDER BY total_confusions DESC
    LIMIT ?1
    "#,
    )?;

    let problems = stmt
        .query_map(params![limit as i64], |row| {
            Ok(ProblemCardRaw {
                id: row.get(0)?,
                front: row.get(1)?,
                main_answer: row.get(2)?,
                is_reverse: row.get(3)?,
                confusion_count: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(problems)
}
