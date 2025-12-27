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
}

impl TierProgress {
    pub fn percentage(&self) -> i64 {
        if self.total > 0 {
            (self.learned * 100) / self.total
        } else {
            0
        }
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

// TODO: Planned feature - Dark mode UI toggle
// Settings stored in DB but UI toggle not yet implemented

#[allow(dead_code)]
pub fn get_dark_mode(conn: &Connection) -> Result<bool> {
    get_setting(conn, "dark_mode").map(|v| v.as_deref() == Some("true"))
}

#[allow(dead_code)]
pub fn set_dark_mode(conn: &Connection, enabled: bool) -> Result<()> {
    set_setting(conn, "dark_mode", if enabled { "true" } else { "false" })
}

// TODO: Planned feature - Text-to-speech integration
// Settings stored in DB, awaiting TTS provider integration (MMS model)

#[allow(dead_code)]
pub fn get_tts_enabled(conn: &Connection) -> Result<bool> {
    get_setting(conn, "tts_enabled").map(|v| v.as_deref() != Some("false"))
}

#[allow(dead_code)]
pub fn set_tts_enabled(conn: &Connection, enabled: bool) -> Result<()> {
    set_setting(conn, "tts_enabled", if enabled { "true" } else { "false" })
}

#[allow(dead_code)]
pub fn get_tts_model(conn: &Connection) -> Result<String> {
    get_setting(conn, "tts_model").map(|v| v.unwrap_or_else(|| "mms".to_string()))
}

#[allow(dead_code)]
pub fn set_tts_model(conn: &Connection, model: &str) -> Result<()> {
    set_setting(conn, "tts_model", model)
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

// TODO: Planned feature - Per-tier enable/disable in UI
// Used for fine-grained study control when all_tiers_unlocked is true
#[allow(dead_code)]
pub fn is_tier_enabled(conn: &Connection, tier: u8) -> Result<bool> {
    let enabled = get_enabled_tiers(conn)?;
    Ok(enabled.contains(&tier))
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

/// Get the effective tiers to use for card selection
pub fn get_effective_tiers(conn: &Connection) -> Result<Vec<u8>> {
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
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::TierUnlock { tier });

    set_setting(conn, "max_unlocked_tier", &tier.to_string())
}

pub fn unlock_next_tier(conn: &Connection) -> Result<u8> {
    let current = get_max_unlocked_tier(conn)?;
    let next = (current + 1).min(4);
    set_max_unlocked_tier(conn, next)?;

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::TierUnlock { tier: next });

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
