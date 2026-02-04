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
        
        (self.strong_memories * 100 + self.medium_memories * 60 + self.weak_memories * 30)
            / graduated
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

/// Check if focus mode is enabled (simple boolean toggle)
pub fn is_focus_mode_enabled(conn: &Connection) -> Result<bool> {
    get_setting(conn, "focus_mode").map(|v| v.as_deref() == Some("true"))
}

/// Enable or disable focus mode (faster learning steps)
pub fn set_focus_mode_enabled(conn: &Connection, enabled: bool) -> Result<()> {
    set_setting(conn, "focus_mode", if enabled { "true" } else { "false" })
}

/// Check if focus mode is active (uses new simple boolean)
pub fn is_focus_mode_active(conn: &Connection) -> Result<bool> {
    is_focus_mode_enabled(conn)
}

// --- Deprecated: tier/lesson focus (kept for migration/cleanup) ---

/// Get the currently focused tier (None = no focus, study all unlocked tiers)
/// DEPRECATED: Use is_focus_mode_enabled() instead
pub fn get_focus_tier(conn: &Connection) -> Result<Option<u8>> {
    get_setting(conn, "focus_tier").map(|v| v.and_then(|s| s.parse().ok()))
}

/// Set the focus tier (None to disable focus mode)
/// DEPRECATED: Use set_focus_mode_enabled() instead
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

// --- Deprecated: Focus Lesson Mode (kept for migration/cleanup) ---

/// Get the currently focused lesson (None = no focus)
/// DEPRECATED: Use is_focus_mode_enabled() instead
/// Returns (pack_id, lesson_number) if set
pub fn get_focus_lesson(conn: &Connection) -> Result<Option<(String, u8)>> {
    let pack = get_setting(conn, "focus_pack")?;
    let lesson = get_setting(conn, "focus_lesson")?;

    match (pack, lesson) {
        (Some(p), Some(l)) if !p.is_empty() => {
            if let Ok(lesson_num) = l.parse::<u8>() {
                Ok(Some((p, lesson_num)))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

/// Set the focus lesson (None to disable focus lesson mode)
/// DEPRECATED: Use set_focus_mode_enabled() instead
pub fn set_focus_lesson(conn: &Connection, focus: Option<(&str, u8)>) -> Result<()> {
    match focus {
        Some((pack_id, lesson)) => {
            set_setting(conn, "focus_pack", pack_id)?;
            set_setting(conn, "focus_lesson", &lesson.to_string())?;
        }
        None => {
            conn.execute("DELETE FROM settings WHERE key = 'focus_pack'", [])?;
            conn.execute("DELETE FROM settings WHERE key = 'focus_lesson'", [])?;
        }
    }
    Ok(())
}

// ==================== Daily New Card Limit ====================

/// Get the daily new cards limit (0 = unlimited/off)
pub fn get_daily_new_cards_limit(conn: &Connection) -> Result<u32> {
    get_setting(conn, "daily_new_cards")
        .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(20))
}

/// Set the daily new cards limit (0 = unlimited/off)
pub fn set_daily_new_cards_limit(conn: &Connection, limit: u32) -> Result<()> {
    set_setting(conn, "daily_new_cards", &limit.to_string())
}

/// Count new cards introduced today (first review was today)
/// A "new card" is one where the first review happened today
pub fn count_new_cards_today(conn: &Connection) -> Result<u32> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Count cards whose first-ever review was today
    // We find cards where the earliest review_log entry for that card was today
    // Note: Don't filter on total_reviews since it increases with each learning step
    let count: i64 = conn.query_row(
        r#"
        SELECT COUNT(DISTINCT rl.card_id)
        FROM review_logs rl
        WHERE date(rl.reviewed_at, 'localtime') = ?1
          AND rl.id = (
            SELECT MIN(rl2.id) FROM review_logs rl2 WHERE rl2.card_id = rl.card_id
          )
        "#,
        params![today],
        |row| row.get(0),
    )?;

    Ok(count as u32)
}

/// Check if we can introduce a new card based on daily limit
/// Returns true if limit is 0 (off) or we haven't reached the limit yet
pub fn can_introduce_new_card(conn: &Connection) -> Result<bool> {
    let limit = get_daily_new_cards_limit(conn)?;
    if limit == 0 {
        return Ok(true); // Unlimited
    }
    let today_count = count_new_cards_today(conn)?;
    Ok(today_count < limit)
}

/// Get remaining new card slots for today
/// Returns u32::MAX if limit is 0 (unlimited), otherwise returns (limit - today_count)
pub fn get_remaining_new_card_slots(conn: &Connection) -> Result<u32> {
    let limit = get_daily_new_cards_limit(conn)?;
    if limit == 0 {
        return Ok(u32::MAX); // Unlimited
    }
    let today_count = count_new_cards_today(conn)?;
    Ok(limit.saturating_sub(today_count))
}

// ==================== Session Stats ====================

/// Stats about the current study session
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    /// Number of new cards introduced today
    pub new_cards_today: u32,
    /// Daily new cards limit (0 = unlimited)
    pub new_cards_limit: u32,
    /// Number of new cards available (never seen, no progress entry)
    pub new_available: u32,
    /// Number of due reviews remaining (graduated cards, learning_step >= 4)
    pub reviews_due: u32,
    /// Number of cards in learning queue (learning_step 1-3)
    pub learning_due: u32,
}

impl SessionStats {
    /// Total due cards (should match home page count)
    pub fn total_due(&self) -> u32 {
        self.new_available + self.reviews_due + self.learning_due
    }
}

/// Get session statistics for display
/// Uses same query pattern as home page for consistency
pub fn get_session_stats(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
    filter: &super::StudyFilterMode,
) -> Result<SessionStats> {
    use chrono::Utc;

    let new_cards_today = count_new_cards_today(conn)?;
    let new_cards_limit = get_daily_new_cards_limit(conn)?;

    let now = Utc::now().to_rfc3339();
    let (filter_clause, _, skip_tier_filter) = super::build_filter_where_clause(conn, app_conn, user_id, filter)?;

    let tier_clause = if skip_tier_filter {
        String::new()
    } else {
        let effective_tiers = get_effective_tiers(conn)?;
        if effective_tiers.is_empty() {
            return Ok(SessionStats {
                new_cards_today,
                new_cards_limit,
                new_available: 0,
                reviews_due: 0,
                learning_due: 0,
            });
        }
        let tier_list = effective_tiers
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("AND cd.tier IN ({})", tier_list)
    };

    // Count new cards (no progress entry yet)
    let new_query = format!(
        r#"
        SELECT COUNT(*)
        FROM app.card_definitions cd
        LEFT JOIN card_progress cp ON cp.card_id = cd.id
        WHERE cp.card_id IS NULL
        {} {}
        "#,
        tier_clause, filter_clause
    );
    let new_available: i64 = conn.query_row(&new_query, [], |row| row.get(0)).unwrap_or(0);

    // Count due reviews (graduated cards: learning_step >= 4, due now)
    let reviews_query = format!(
        r#"
        SELECT COUNT(*)
        FROM app.card_definitions cd
        LEFT JOIN card_progress cp ON cp.card_id = cd.id
        WHERE cp.learning_step >= 4
          AND cp.next_review <= ?1
        {} {}
        "#,
        tier_clause, filter_clause
    );
    let reviews_due: i64 = conn.query_row(&reviews_query, params![now], |row| row.get(0)).unwrap_or(0);

    // Count learning queue (ALL cards in learning steps 1-3, regardless of when due)
    let learning_query = format!(
        r#"
        SELECT COUNT(*)
        FROM app.card_definitions cd
        LEFT JOIN card_progress cp ON cp.card_id = cd.id
        WHERE cp.learning_step BETWEEN 1 AND 3
        {} {}
        "#,
        tier_clause, filter_clause
    );
    let learning_due: i64 = conn.query_row(&learning_query, [], |row| row.get(0)).unwrap_or(0);

    Ok(SessionStats {
        new_cards_today,
        new_cards_limit,
        new_available: new_available as u32,
        reviews_due: reviews_due as u32,
        learning_due: learning_due as u32,
    })
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

    if let Some(progress) = current_progress
        && progress.percentage() >= 80 {
            let new_tier = unlock_next_tier(conn)?;
            return Ok(Some(new_tier));
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

/// SQL constants for tier queries using cross-DB join
const TIER_FROM: &str = r#"
FROM app.card_definitions cd
LEFT JOIN card_progress cp ON cp.card_id = cd.id
"#;

pub fn get_progress_by_tier(conn: &Connection) -> Result<Vec<TierProgress>> {
    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::DbQuery {
        operation: "select_progress".into(),
        table: "card_progress".into(),
    });

    let max_tier = get_max_unlocked_tier(conn)?;
    let all_unlocked = get_all_tiers_unlocked(conn)?;
    let enabled_tiers = get_enabled_tiers(conn)?;

    let mut progress = Vec::new();
    for tier in 1..=4u8 {
        let total: i64 = conn.query_row(
            &format!("SELECT COUNT(*) {} WHERE cd.tier = ?1", TIER_FROM),
            params![tier],
            |row| row.get(0),
        )?;

        let new_cards: i64 = conn.query_row(
            &format!("SELECT COUNT(*) {} WHERE cd.tier = ?1 AND COALESCE(cp.total_reviews, 0) = 0", TIER_FROM),
            params![tier],
            |row| row.get(0),
        )?;

        let learning: i64 = conn.query_row(
            &format!("SELECT COUNT(*) {} WHERE cd.tier = ?1 AND COALESCE(cp.total_reviews, 0) > 0 AND COALESCE(cp.learning_step, 0) < 4", TIER_FROM),
            params![tier],
            |row| row.get(0),
        )?;

        let learned: i64 = conn.query_row(
            &format!("SELECT COUNT(*) {} WHERE cd.tier = ?1 AND COALESCE(cp.learning_step, 0) >= 4", TIER_FROM),
            params![tier],
            |row| row.get(0),
        )?;

        let total_reviews: i64 = conn.query_row(
            &format!("SELECT COALESCE(SUM(cp.total_reviews), 0) {} WHERE cd.tier = ?1", TIER_FROM),
            params![tier],
            |row| row.get(0),
        )?;

        // Stability metrics for graduated cards only (learning_step >= 4)
        let avg_stability_days: f64 = conn
            .query_row(
                &format!("SELECT COALESCE(AVG(cp.fsrs_stability), 0) {} WHERE cd.tier = ?1 AND COALESCE(cp.learning_step, 0) >= 4 AND cp.fsrs_stability > 0", TIER_FROM),
                params![tier],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let strong_memories: i64 = conn.query_row(
            &format!("SELECT COUNT(*) {} WHERE cd.tier = ?1 AND COALESCE(cp.learning_step, 0) >= 4 AND cp.fsrs_stability >= 14", TIER_FROM),
            params![tier],
            |row| row.get(0),
        )?;

        let medium_memories: i64 = conn.query_row(
            &format!("SELECT COUNT(*) {} WHERE cd.tier = ?1 AND COALESCE(cp.learning_step, 0) >= 4 AND cp.fsrs_stability >= 7 AND cp.fsrs_stability < 14", TIER_FROM),
            params![tier],
            |row| row.get(0),
        )?;

        let weak_memories: i64 = conn.query_row(
            &format!("SELECT COUNT(*) {} WHERE cd.tier = ?1 AND COALESCE(cp.learning_step, 0) >= 4 AND cp.fsrs_stability > 0 AND cp.fsrs_stability < 7", TIER_FROM),
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
    let count = conn.execute("UPDATE card_progress SET next_review = ?1", params![now])?;
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

    // For graduation, we need to insert/update card_progress for all cards in the tier
    // First, get all card IDs for this tier
    let card_ids: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT cd.id FROM app.card_definitions cd WHERE cd.tier = ?1"
        )?;
        stmt.query_map(params![tier], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };

    let mut count = 0;
    for card_id in card_ids {
        conn.execute(
            r#"INSERT INTO card_progress (card_id, learning_step, repetitions, fsrs_stability, fsrs_state, next_review, ease_factor, interval_days, total_reviews, correct_reviews)
               VALUES (?1, 4, 2, 3.0, 'Review', ?2, 2.5, 3, 0, 0)
               ON CONFLICT(card_id) DO UPDATE SET
                   learning_step = 4,
                   repetitions = 2,
                   fsrs_stability = 3.0,
                   fsrs_state = 'Review',
                   next_review = ?2"#,
            params![card_id, next_review],
        )?;
        count += 1;
    }

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
    // A card is not graduated if it either has no progress entry or learning_step < 4
    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(*) {} WHERE cd.tier = ?1 AND COALESCE(cp.learning_step, 0) < 4", TIER_FROM),
        params![tier],
        |row| row.get(0),
    )?;
    Ok(count == 0)
}

/// Backup current card states for a tier before graduation
pub fn backup_tier_state(conn: &Connection, tier: u8) -> Result<()> {
    use chrono::Utc;

    let mut stmt = conn.prepare(
        &format!(
            r#"SELECT cd.id, COALESCE(cp.learning_step, 0), COALESCE(cp.repetitions, 0),
                      cp.fsrs_stability, cp.fsrs_difficulty, cp.fsrs_state,
                      COALESCE(cp.next_review, datetime('now'))
               {} WHERE cd.tier = ?1"#,
            TIER_FROM
        ),
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
            r#"INSERT INTO card_progress (card_id, learning_step, repetitions, fsrs_stability, fsrs_difficulty, fsrs_state, next_review, ease_factor, interval_days, total_reviews, correct_reviews)
               VALUES (?7, ?1, ?2, ?3, ?4, ?5, ?6, 2.5, 0, 0, 0)
               ON CONFLICT(card_id) DO UPDATE SET
                   learning_step = ?1,
                   repetitions = ?2,
                   fsrs_stability = ?3,
                   fsrs_difficulty = ?4,
                   fsrs_state = ?5,
                   next_review = ?6"#,
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
        table: "card_progress".into(),
    });

    let total_cards: i64 = conn.query_row(
        &format!("SELECT COUNT(*) {}", TIER_FROM),
        [],
        |row| row.get(0),
    )?;
    let total_reviews: i64 = conn.query_row(
        &format!("SELECT COALESCE(SUM(cp.total_reviews), 0) {}", TIER_FROM),
        [],
        |row| row.get(0),
    )?;
    let cards_learned: i64 = conn.query_row(
        &format!("SELECT COUNT(*) {} WHERE COALESCE(cp.repetitions, 0) >= 2", TIER_FROM),
        [],
        |row| row.get(0),
    )?;
    Ok((total_cards, total_reviews, cards_learned))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tier_progress(
        tier: u8,
        total: i64,
        learned: i64,
        strong: i64,
        medium: i64,
        weak: i64,
    ) -> TierProgress {
        TierProgress {
            tier,
            total,
            new_cards: 0,
            learning: 0,
            learned,
            total_reviews: 0,
            is_unlocked: true,
            is_enabled: true,
            avg_stability_days: 0.0,
            strong_memories: strong,
            medium_memories: medium,
            weak_memories: weak,
        }
    }

    #[test]
    fn test_tier_progress_percentage_zero_total() {
        let progress = make_tier_progress(1, 0, 0, 0, 0, 0);
        assert_eq!(progress.percentage(), 0);
    }

    #[test]
    fn test_tier_progress_percentage_none_learned() {
        let progress = make_tier_progress(1, 30, 0, 0, 0, 0);
        assert_eq!(progress.percentage(), 0);
    }

    #[test]
    fn test_tier_progress_percentage_partial() {
        let progress = make_tier_progress(1, 30, 15, 0, 0, 0);
        assert_eq!(progress.percentage(), 50);
    }

    #[test]
    fn test_tier_progress_percentage_all_learned() {
        let progress = make_tier_progress(1, 30, 30, 0, 0, 0);
        assert_eq!(progress.percentage(), 100);
    }

    #[test]
    fn test_tier_progress_percentage_rounding() {
        // 24/30 = 80%, 25/30 = 83.33...% -> 83
        let progress1 = make_tier_progress(1, 30, 24, 0, 0, 0);
        assert_eq!(progress1.percentage(), 80);

        let progress2 = make_tier_progress(1, 30, 25, 0, 0, 0);
        assert_eq!(progress2.percentage(), 83);
    }

    #[test]
    fn test_memory_strength_no_graduated_cards() {
        let progress = make_tier_progress(1, 30, 0, 0, 0, 0);
        assert_eq!(progress.memory_strength(), 0);
    }

    #[test]
    fn test_memory_strength_all_strong() {
        let progress = make_tier_progress(1, 30, 30, 30, 0, 0);
        assert_eq!(progress.memory_strength(), 100);
    }

    #[test]
    fn test_memory_strength_all_medium() {
        let progress = make_tier_progress(1, 30, 30, 0, 30, 0);
        assert_eq!(progress.memory_strength(), 60);
    }

    #[test]
    fn test_memory_strength_all_weak() {
        let progress = make_tier_progress(1, 30, 30, 0, 0, 30);
        assert_eq!(progress.memory_strength(), 30);
    }

    #[test]
    fn test_memory_strength_mixed() {
        // 10 strong (100pts each) + 10 medium (60pts each) + 10 weak (30pts each)
        // = (1000 + 600 + 300) / 30 = 63.33... -> 63
        let progress = make_tier_progress(1, 30, 30, 10, 10, 10);
        assert_eq!(progress.memory_strength(), 63);
    }

    #[test]
    fn test_has_stability_data_no_graduated() {
        let progress = make_tier_progress(1, 30, 0, 0, 0, 0);
        assert!(!progress.has_stability_data());
    }

    #[test]
    fn test_has_stability_data_with_graduated() {
        let progress = make_tier_progress(1, 30, 10, 5, 3, 2);
        assert!(progress.has_stability_data());
    }

    #[test]
    fn test_has_stability_data_only_strong() {
        let progress = make_tier_progress(1, 30, 10, 10, 0, 0);
        assert!(progress.has_stability_data());
    }

    // Helper to create a test database with review_logs table
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE review_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id INTEGER NOT NULL,
                quality INTEGER NOT NULL,
                reviewed_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_count_new_cards_today_single_review() {
        let conn = setup_test_db();
        let today = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        // Insert one review for card 1 today
        conn.execute(
            "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (1, 4, ?1)",
            params![today],
        )
        .unwrap();

        assert_eq!(count_new_cards_today(&conn).unwrap(), 1);
    }

    #[test]
    fn test_count_new_cards_today_multiple_reviews_same_card() {
        let conn = setup_test_db();
        let today = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        // Insert 4 reviews for the same card today (simulating learning steps)
        for _ in 0..4 {
            conn.execute(
                "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (1, 4, ?1)",
                params![today],
            )
            .unwrap();
        }

        // Should count as 1 new card, not 4
        assert_eq!(count_new_cards_today(&conn).unwrap(), 1);
    }

    #[test]
    fn test_count_new_cards_today_multiple_cards() {
        let conn = setup_test_db();
        let today = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        // Card 1: 3 reviews today
        for _ in 0..3 {
            conn.execute(
                "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (1, 4, ?1)",
                params![today],
            )
            .unwrap();
        }

        // Card 2: 2 reviews today
        for _ in 0..2 {
            conn.execute(
                "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (2, 4, ?1)",
                params![today],
            )
            .unwrap();
        }

        // Should count as 2 new cards
        assert_eq!(count_new_cards_today(&conn).unwrap(), 2);
    }

    #[test]
    fn test_count_new_cards_today_excludes_yesterday() {
        let conn = setup_test_db();
        let today = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();

        // Card 1: first reviewed yesterday, reviewed again today
        conn.execute(
            "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (1, 4, ?1)",
            params![yesterday],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (1, 4, ?1)",
            params![today],
        )
        .unwrap();

        // Card 2: first reviewed today
        conn.execute(
            "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (2, 4, ?1)",
            params![today],
        )
        .unwrap();

        // Should only count card 2 (card 1's first review was yesterday)
        assert_eq!(count_new_cards_today(&conn).unwrap(), 1);
    }

    #[test]
    fn test_count_new_cards_today_empty() {
        let conn = setup_test_db();
        assert_eq!(count_new_cards_today(&conn).unwrap(), 0);
    }

    // Helper to create a test database with settings and review_logs tables
    fn setup_test_db_with_settings() -> Connection {
        let conn = setup_test_db();
        conn.execute(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_get_remaining_new_card_slots_unlimited() {
        let conn = setup_test_db_with_settings();
        // Default daily_new_cards is 20, but if explicitly set to 0, it's unlimited
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('daily_new_cards', '0')",
            [],
        )
        .unwrap();

        assert_eq!(get_remaining_new_card_slots(&conn).unwrap(), u32::MAX);
    }

    #[test]
    fn test_get_remaining_new_card_slots_default_limit() {
        let conn = setup_test_db_with_settings();
        // No setting = default limit of 20, no cards studied today
        assert_eq!(get_remaining_new_card_slots(&conn).unwrap(), 20);
    }

    #[test]
    fn test_get_remaining_new_card_slots_partial_used() {
        let conn = setup_test_db_with_settings();
        let today = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        // Set limit to 10
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('daily_new_cards', '10')",
            [],
        )
        .unwrap();

        // Study 3 new cards today
        for card_id in 1..=3 {
            conn.execute(
                "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (?1, 4, ?2)",
                params![card_id, today],
            )
            .unwrap();
        }

        // 10 - 3 = 7 remaining
        assert_eq!(get_remaining_new_card_slots(&conn).unwrap(), 7);
    }

    #[test]
    fn test_get_remaining_new_card_slots_at_limit() {
        let conn = setup_test_db_with_settings();
        let today = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        // Set limit to 3
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('daily_new_cards', '3')",
            [],
        )
        .unwrap();

        // Study 3 new cards today (exactly at limit)
        for card_id in 1..=3 {
            conn.execute(
                "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (?1, 4, ?2)",
                params![card_id, today],
            )
            .unwrap();
        }

        // 3 - 3 = 0 remaining
        assert_eq!(get_remaining_new_card_slots(&conn).unwrap(), 0);
    }

    #[test]
    fn test_get_remaining_new_card_slots_over_limit() {
        let conn = setup_test_db_with_settings();
        let today = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        // Set limit to 2
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('daily_new_cards', '2')",
            [],
        )
        .unwrap();

        // Study 5 new cards today (somehow over limit)
        for card_id in 1..=5 {
            conn.execute(
                "INSERT INTO review_logs (card_id, quality, reviewed_at) VALUES (?1, 4, ?2)",
                params![card_id, today],
            )
            .unwrap();
        }

        // Should saturate at 0, not underflow
        assert_eq!(get_remaining_new_card_slots(&conn).unwrap(), 0);
    }
}
