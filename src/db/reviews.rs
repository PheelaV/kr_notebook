//! Review logging and confusion tracking

use chrono::Utc;
use rusqlite::{params, Connection, Result};

use crate::domain::{ReviewDirection, ReviewLog, StudyMode};
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
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "insert_enhanced".into(),
        table: "review_logs".into(),
    });

    let now = Utc::now().to_rfc3339();
    conn.execute(
        r#"
    INSERT INTO review_logs (card_id, quality, reviewed_at, is_correct, study_mode, direction, response_time_ms, hints_used)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
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
        ],
    )?;
    Ok(conn.last_insert_rowid())
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
