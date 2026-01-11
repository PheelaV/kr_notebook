//! Character statistics tracking

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, Result};

/// Character-level statistics for tracking learning progress
#[derive(Debug, Clone)]
pub struct CharacterStats {
    pub character: String,
    pub character_type: String,
    pub total_attempts: i64,
    pub total_correct: i64,
    pub attempts_7d: i64,
    pub correct_7d: i64,
    pub attempts_1d: i64,
    pub correct_1d: i64,
    pub last_attempt_at: Option<DateTime<Utc>>,
}

impl CharacterStats {
    pub fn lifetime_rate(&self) -> f64 {
        if self.total_attempts > 0 {
            self.total_correct as f64 / self.total_attempts as f64
        } else {
            0.0
        }
    }

    pub fn rate_7d(&self) -> f64 {
        if self.attempts_7d > 0 {
            self.correct_7d as f64 / self.attempts_7d as f64
        } else {
            0.0
        }
    }

    pub fn rate_1d(&self) -> f64 {
        if self.attempts_1d > 0 {
            self.correct_1d as f64 / self.attempts_1d as f64
        } else {
            0.0
        }
    }
}

/// Update character stats after a review
pub fn update_character_stats(
    conn: &Connection,
    character: &str,
    character_type: &str,
    is_correct: bool,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let correct_increment = if is_correct { 1 } else { 0 };

    // Try to update existing row first
    let updated = conn.execute(
        r#"
    UPDATE character_stats
    SET total_attempts = total_attempts + 1,
        total_correct = total_correct + ?1,
        attempts_7d = attempts_7d + 1,
        correct_7d = correct_7d + ?1,
        attempts_1d = attempts_1d + 1,
        correct_1d = correct_1d + ?1,
        last_attempt_at = ?2
    WHERE character = ?3
    "#,
        params![correct_increment, now, character],
    )?;

    // If no existing row, insert new one
    if updated == 0 {
        conn.execute(
            r#"
      INSERT INTO character_stats
        (character, character_type, total_attempts, total_correct,
         attempts_7d, correct_7d, attempts_1d, correct_1d, last_attempt_at)
      VALUES (?1, ?2, 1, ?3, 1, ?3, 1, ?3, ?4)
      "#,
            params![character, character_type, correct_increment, now],
        )?;
    }

    Ok(())
}

/// Get stats for a specific character
pub fn get_character_stats(conn: &Connection, character: &str) -> Result<Option<CharacterStats>> {
    let mut stmt = conn.prepare(
        r#"
    SELECT character, character_type, total_attempts, total_correct,
           attempts_7d, correct_7d, attempts_1d, correct_1d, last_attempt_at
    FROM character_stats
    WHERE character = ?1
    "#,
    )?;

    let mut rows = stmt.query(params![character])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_character_stats(row)?))
    } else {
        Ok(None)
    }
}

/// Get all stats for a character type
pub fn get_character_stats_by_type(
    conn: &Connection,
    character_type: &str,
) -> Result<Vec<CharacterStats>> {
    let mut stmt = conn.prepare(
        r#"
    SELECT character, character_type, total_attempts, total_correct,
           attempts_7d, correct_7d, attempts_1d, correct_1d, last_attempt_at
    FROM character_stats
    WHERE character_type = ?1
    ORDER BY character
    "#,
    )?;

    let stats = stmt
        .query_map(params![character_type], row_to_character_stats)?
        .collect::<Result<Vec<_>>>()?;

    Ok(stats)
}

/// Get all character stats
pub fn get_all_character_stats(conn: &Connection) -> Result<Vec<CharacterStats>> {
    let mut stmt = conn.prepare(
        r#"
    SELECT character, character_type, total_attempts, total_correct,
           attempts_7d, correct_7d, attempts_1d, correct_1d, last_attempt_at
    FROM character_stats
    ORDER BY character_type, character
    "#,
    )?;

    let stats = stmt
        .query_map([], row_to_character_stats)?
        .collect::<Result<Vec<_>>>()?;

    Ok(stats)
}

/// Refresh decay windows (recalculate 7d and 1d stats from review_logs)
/// Also recalculates all-time stats to ensure consistency
pub fn refresh_character_stats_decay(conn: &Connection) -> Result<()> {
    let seven_days_ago = (Utc::now() - Duration::days(7)).to_rfc3339();
    let one_day_ago = (Utc::now() - Duration::days(1)).to_rfc3339();

    // Recalculate ALL stats from review_logs to ensure consistency
    conn.execute(
        r#"
    UPDATE character_stats
    SET total_attempts = (
          SELECT COUNT(*) FROM review_logs rl
          JOIN cards c ON rl.card_id = c.id
          WHERE c.front = character_stats.character OR c.main_answer = character_stats.character
        ),
        total_correct = (
          SELECT COUNT(*) FROM review_logs rl
          JOIN cards c ON rl.card_id = c.id
          WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
            AND rl.is_correct = 1
        ),
        attempts_7d = (
          SELECT COUNT(*) FROM review_logs rl
          JOIN cards c ON rl.card_id = c.id
          WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
            AND rl.reviewed_at >= ?1
        ),
        correct_7d = (
          SELECT COUNT(*) FROM review_logs rl
          JOIN cards c ON rl.card_id = c.id
          WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
            AND rl.reviewed_at >= ?1 AND rl.is_correct = 1
        ),
        attempts_1d = (
          SELECT COUNT(*) FROM review_logs rl
          JOIN cards c ON rl.card_id = c.id
          WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
            AND rl.reviewed_at >= ?2
        ),
        correct_1d = (
          SELECT COUNT(*) FROM review_logs rl
          JOIN cards c ON rl.card_id = c.id
          WHERE (c.front = character_stats.character OR c.main_answer = character_stats.character)
            AND rl.reviewed_at >= ?2 AND rl.is_correct = 1
        )
    "#,
        params![seven_days_ago, one_day_ago],
    )?;

    Ok(())
}

/// Convert a database row to CharacterStats
fn row_to_character_stats(row: &rusqlite::Row) -> Result<CharacterStats> {
    let last_attempt_str: Option<String> = row.get(8)?;

    Ok(CharacterStats {
        character: row.get(0)?,
        character_type: row.get(1)?,
        total_attempts: row.get(2)?,
        total_correct: row.get(3)?,
        attempts_7d: row.get(4)?,
        correct_7d: row.get(5)?,
        attempts_1d: row.get(6)?,
        correct_1d: row.get(7)?,
        last_attempt_at: last_attempt_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        }),
    })
}
