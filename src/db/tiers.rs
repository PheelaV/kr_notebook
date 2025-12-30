//! Tier management and settings

use rusqlite::{params, Connection, Result};

#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// Progress information for a tier
#[derive(Debug, Clone)]
pub struct TierProgress {
    pub tier: u8,
    pub total: i64,
    pub new_cards: i64,
    pub learning: i64,
    pub learned: i64,
    pub total_reviews: i64,
    pub is_unlocked: bool,
    pub is_enabled: bool,
    /// Average stability in days for graduated cards (fsrs_stability > 0)
    pub avg_stability_days: f64,
    /// Count of cards with strong memories (stability >= 14 days)
    pub strong_memories: i64,
    /// Count of cards with medium memories (stability 7-14 days)
    pub medium_memories: i64,
    /// Count of cards with weak memories (stability < 7 days, but > 0)
    pub weak_memories: i64,
}

impl TierProgress {
    pub fn percentage(&self) -> i64 {
        if self.total > 0 {
            (self.learned * 100) / self.total
        } else {
            0
        }
    }

    /// Memory strength as a 0-100 score based on stability distribution
    /// Strong = 100 points, Medium = 60 points, Weak = 30 points, New/Learning = 0
    pub fn memory_strength(&self) -> i64 {
        let graduated = self.strong_memories + self.medium_memories + self.weak_memories;
        if graduated == 0 {
            return 0;
        }
        let score = (self.strong_memories * 100 + self.medium_memories * 60 + self.weak_memories * 30)
            / graduated;
        score
    }

    /// Returns true if there are any graduated cards with stability data
    pub fn has_stability_data(&self) -> bool {
        self.strong_memories + self.medium_memories + self.weak_memories > 0
    }
}

// ==================== Settings ====================

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select".into(),
        table: "settings".into(),
    });

    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}


// ==================== Tier Management ====================

pub fn get_all_tiers_unlocked(conn: &Connection) -> Result<bool> {
    get_setting(conn, "all_tiers_unlocked").map(|v| v.as_deref() == Some("true"))
}

pub fn set_all_tiers_unlocked(conn: &Connection, enabled: bool) -> Result<()> {
    set_setting(
        conn,
        "all_tiers_unlocked",
        if enabled { "true" } else { "false" },
    )
}

pub fn get_enabled_tiers(conn: &Connection) -> Result<Vec<u8>> {
    let value = get_setting(conn, "enabled_tiers")?.unwrap_or_else(|| "1,2,3,4".to_string());
    Ok(value
        .split(',')
        .filter_map(|s| s.trim().parse::<u8>().ok())
        .collect())
}

pub fn set_enabled_tiers(conn: &Connection, tiers: &[u8]) -> Result<()> {
    let value = tiers
        .iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(",");
    set_setting(conn, "enabled_tiers", &value)
}

pub fn get_max_unlocked_tier(conn: &Connection) -> Result<u8> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select".into(),
        table: "settings".into(),
    });

    get_setting(conn, "max_unlocked_tier")
        .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(1))
}

// ==================== Focus Mode ====================

/// Get the currently focused tier (None = no focus, study all unlocked tiers)
pub fn get_focus_tier(conn: &Connection) -> Result<Option<u8>> {
    get_setting(conn, "focus_tier").map(|v| v.and_then(|s| s.parse().ok()))
}

/// Set the focus tier (None to disable focus mode)
pub fn set_focus_tier(conn: &Connection, tier: Option<u8>) -> Result<()> {
    match tier {
        Some(t) => set_setting(conn, "focus_tier", &t.to_string()),
        None => {
            // Remove the setting to indicate no focus
            conn.execute("DELETE FROM settings WHERE key = 'focus_tier'", [])?;
            Ok(())
        }
    }
}

/// Check if focus mode is active
pub fn is_focus_mode_active(conn: &Connection) -> Result<bool> {
    get_focus_tier(conn).map(|t| t.is_some())
}

/// Get the effective tiers to use for card selection
/// Respects focus mode: if a focus tier is set, only that tier is returned
pub fn get_effective_tiers(conn: &Connection) -> Result<Vec<u8>> {
    // Check for focus mode first
    if let Some(focus_tier) = get_focus_tier(conn)? {
        return Ok(vec![focus_tier]);
    }

    let all_unlocked = get_all_tiers_unlocked(conn)?;
    if all_unlocked {
        // When all tiers unlocked, use enabled_tiers setting
        get_enabled_tiers(conn)
    } else {
        // When progressive unlocking, use 1..=max_unlocked
        let max_tier = get_max_unlocked_tier(conn)?;
        Ok((1..=max_tier).collect())
    }
}

pub fn set_max_unlocked_tier(conn: &Connection, tier: u8) -> Result<()> {
    // TierUnlock profiling moved to handler level (requires username)
    set_setting(conn, "max_unlocked_tier", &tier.to_string())
}

pub fn unlock_next_tier(conn: &Connection) -> Result<u8> {
    let current = get_max_unlocked_tier(conn)?;
    let next = (current + 1).min(4);
    set_max_unlocked_tier(conn, next)?;

    // Auto-enable focus mode on the newly unlocked tier
    set_focus_tier(conn, Some(next))?;

    // TierUnlock profiling moved to handler level (requires username)
    Ok(next)
}

/// Try to auto-unlock the next tier based on progress
pub fn try_auto_unlock_tier(conn: &Connection) -> Result<Option<u8>> {
    // Don't auto-unlock if all tiers are already unlocked via setting
    if get_all_tiers_unlocked(conn)? {
        return Ok(None);
    }

    let current_tier = get_max_unlocked_tier(conn)?;
    if current_tier >= 4 {
        return Ok(None);
    }

    // Check if current tier has >= 80% learned
    let tier_stats = get_progress_by_tier(conn)?;
    let current_progress = tier_stats.iter().find(|t| t.tier == current_tier);

    if let Some(progress) = current_progress {
        if progress.percentage() >= 80 {
            let new_tier = unlock_next_tier(conn)?;
            return Ok(Some(new_tier));
        }
    }

    Ok(None)
}

// ==================== SRS Settings ====================

/// Check if FSRS is enabled in settings
pub fn get_use_fsrs(conn: &Connection) -> Result<bool> {
    get_setting(conn, "use_fsrs").map(|v| v.as_deref() != Some("false"))
}

/// Get desired retention target (default 0.9 = 90%)
pub fn get_desired_retention(conn: &Connection) -> Result<f64> {
    get_setting(conn, "desired_retention").map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(0.9))
}

/// Check if interleaving is enabled (mixing card types)
pub fn get_use_interleaving(conn: &Connection) -> Result<bool> {
    get_setting(conn, "use_interleaving").map(|v| v.as_deref() != Some("false"))
}

// ==================== Progress & Stats ====================

pub fn get_progress_by_tier(conn: &Connection) -> Result<Vec<TierProgress>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_progress".into(),
        table: "cards".into(),
    });

    let max_tier = get_max_unlocked_tier(conn)?;
    let all_unlocked = get_all_tiers_unlocked(conn)?;
    let enabled_tiers = get_enabled_tiers(conn)?;

    let mut progress = Vec::new();
    for tier in 1..=4u8 {
        let total: i64 =
            conn.query_row("SELECT COUNT(*) FROM cards WHERE tier = ?1", params![tier], |row| {
                row.get(0)
            })?;

        let new_cards: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE tier = ?1 AND total_reviews = 0",
            params![tier],
            |row| row.get(0),
        )?;

        let learning: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE tier = ?1 AND total_reviews > 0 AND repetitions < 2",
            params![tier],
            |row| row.get(0),
        )?;

        let learned: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE tier = ?1 AND repetitions >= 2",
            params![tier],
            |row| row.get(0),
        )?;

        let total_reviews: i64 = conn.query_row(
            "SELECT COALESCE(SUM(total_reviews), 0) FROM cards WHERE tier = ?1",
            params![tier],
            |row| row.get(0),
        )?;

        // Stability metrics for graduated cards only (learning_step >= 4)
        let avg_stability_days: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(fsrs_stability), 0) FROM cards WHERE tier = ?1 AND learning_step >= 4 AND fsrs_stability > 0",
                params![tier],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let strong_memories: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE tier = ?1 AND learning_step >= 4 AND fsrs_stability >= 14",
            params![tier],
            |row| row.get(0),
        )?;

        let medium_memories: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE tier = ?1 AND learning_step >= 4 AND fsrs_stability >= 7 AND fsrs_stability < 14",
            params![tier],
            |row| row.get(0),
        )?;

        let weak_memories: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE tier = ?1 AND learning_step >= 4 AND fsrs_stability > 0 AND fsrs_stability < 7",
            params![tier],
            |row| row.get(0),
        )?;

        let is_unlocked = if all_unlocked {
            enabled_tiers.contains(&tier)
        } else {
            tier <= max_tier
        };

        progress.push(TierProgress {
            tier,
            total,
            new_cards,
            learning,
            learned,
            total_reviews,
            is_unlocked,
            is_enabled: enabled_tiers.contains(&tier),
            avg_stability_days,
            strong_memories,
            medium_memories,
            weak_memories,
        });
    }

    Ok(progress)
}

/// Make all cards due now (for testing/accelerated learning)
pub fn make_all_cards_due(conn: &Connection) -> Result<usize> {
    let now = chrono::Utc::now().to_rfc3339();
    let count = conn.execute("UPDATE cards SET next_review = ?1", params![now])?;
    Ok(count)
}

/// Graduate all cards in a tier (escape hatch for users who know the material)
/// Sets cards to graduated state with 3-day review interval
/// Automatically backs up card state before graduation
pub fn graduate_tier(conn: &Connection, tier: u8) -> Result<usize> {
    use chrono::{Duration, Utc};

    // Skip if already fully graduated
    if is_tier_fully_graduated(conn, tier)? {
        return Ok(0);
    }

    // Backup current state before graduating
    backup_tier_state(conn, tier)?;

    let next_review = (Utc::now() + Duration::days(3)).to_rfc3339();

    let count = conn.execute(
        "UPDATE cards SET
            learning_step = 4,
            repetitions = 2,
            fsrs_stability = 3.0,
            fsrs_state = 'Review',
            next_review = ?1
         WHERE tier = ?2",
        params![next_review, tier],
    )?;

    Ok(count)
}

// ==================== Tier Graduation Backup ====================

use serde::{Deserialize, Serialize};

/// Card state backup for undo graduation
#[derive(Debug, Serialize, Deserialize)]
struct CardStateBackup {
    id: i64,
    learning_step: i64,
    repetitions: i64,
    fsrs_stability: Option<f64>,
    fsrs_difficulty: Option<f64>,
    fsrs_state: Option<String>,
    next_review: String,
}

/// Check if a tier is fully graduated (all cards have learning_step >= 4)
pub fn is_tier_fully_graduated(conn: &Connection, tier: u8) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cards WHERE tier = ?1 AND learning_step < 4",
        params![tier],
        |row| row.get(0),
    )?;
    Ok(count == 0)
}

/// Backup current card states for a tier before graduation
pub fn backup_tier_state(conn: &Connection, tier: u8) -> Result<()> {
    use chrono::Utc;

    let mut stmt = conn.prepare(
        "SELECT id, learning_step, repetitions, fsrs_stability, fsrs_difficulty, fsrs_state, next_review
         FROM cards WHERE tier = ?1",
    )?;

    let backups: Vec<CardStateBackup> = stmt
        .query_map(params![tier], |row| {
            Ok(CardStateBackup {
                id: row.get(0)?,
                learning_step: row.get(1)?,
                repetitions: row.get(2)?,
                fsrs_stability: row.get(3)?,
                fsrs_difficulty: row.get(4)?,
                fsrs_state: row.get(5)?,
                next_review: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let backup_json = serde_json::to_string(&backups).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(e))
    })?;

    let created_at = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO tier_graduation_backups (tier, backup_data, created_at)
         VALUES (?1, ?2, ?3)",
        params![tier, backup_json, created_at],
    )?;

    Ok(())
}

/// Restore card states from backup (undo graduation)
pub fn restore_tier_state(conn: &Connection, tier: u8) -> Result<usize> {
    let backup_json: String = conn.query_row(
        "SELECT backup_data FROM tier_graduation_backups WHERE tier = ?1",
        params![tier],
        |row| row.get(0),
    )?;

    let backups: Vec<CardStateBackup> = serde_json::from_str(&backup_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;

    let mut restored = 0;
    for backup in &backups {
        conn.execute(
            "UPDATE cards SET
                learning_step = ?1,
                repetitions = ?2,
                fsrs_stability = ?3,
                fsrs_difficulty = ?4,
                fsrs_state = ?5,
                next_review = ?6
             WHERE id = ?7",
            params![
                backup.learning_step,
                backup.repetitions,
                backup.fsrs_stability,
                backup.fsrs_difficulty,
                backup.fsrs_state,
                backup.next_review,
                backup.id
            ],
        )?;
        restored += 1;
    }

    // Delete the backup after successful restore
    delete_tier_backup(conn, tier)?;

    Ok(restored)
}

/// Check if a backup exists for a tier
pub fn has_tier_backup(conn: &Connection, tier: u8) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tier_graduation_backups WHERE tier = ?1",
        params![tier],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Delete a tier backup
pub fn delete_tier_backup(conn: &Connection, tier: u8) -> Result<()> {
    conn.execute(
        "DELETE FROM tier_graduation_backups WHERE tier = ?1",
        params![tier],
    )?;
    Ok(())
}

/// Get total stats across ALL cards (global totals, not filtered by mode)
pub fn get_total_stats(conn: &Connection) -> Result<(i64, i64, i64)> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_total".into(),
        table: "cards".into(),
    });

    let total_cards: i64 =
        conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    let total_reviews: i64 = conn.query_row(
        "SELECT COALESCE(SUM(total_reviews), 0) FROM cards",
        [],
        |row| row.get(0),
    )?;
    let cards_learned: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cards WHERE repetitions >= 2",
        [],
        |row| row.get(0),
    )?;
    Ok((total_cards, total_reviews, cards_learned))
}
