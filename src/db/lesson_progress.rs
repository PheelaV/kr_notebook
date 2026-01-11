//! Pack lesson progress tracking and management.
//!
//! Similar to tier management but for vocabulary pack lessons.
//! Each pack can have lesson-based progression with unlock thresholds.

use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

/// Validate that a pack_id is safe for SQL string interpolation.
/// Pack IDs should only contain alphanumeric characters, hyphens, and underscores.
/// This is a defense-in-depth measure - pack_ids should already be validated at input.
fn is_safe_pack_id(pack_id: &str) -> bool {
    !pack_id.is_empty()
        && pack_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

// ==================== Study Filter Types ====================

/// Filter mode for card selection during study sessions.
/// Determines which cards are included in the study pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum StudyFilterMode {
    /// All enabled content (Hangul tiers + enabled packs with unlocked lessons)
    #[default]
    All,
    /// Only Hangul/baseline content (cards without pack_id or with lesson=NULL)
    HangulOnly,
    /// Only cards from a specific pack (all unlocked lessons)
    PackOnly(String),
    /// Only cards from a specific pack and lesson
    PackLesson(String, u8),
}


impl StudyFilterMode {
    /// Parse from settings string format
    pub fn from_settings(mode: &str, pack: &str, lessons: &str) -> Self {
        match mode {
            "hangul" => StudyFilterMode::HangulOnly,
            "pack" if !pack.is_empty() => {
                if let Some(lesson) = lessons.split(',').next().and_then(|s| s.trim().parse().ok()) {
                    StudyFilterMode::PackLesson(pack.to_string(), lesson)
                } else {
                    StudyFilterMode::PackOnly(pack.to_string())
                }
            }
            _ => StudyFilterMode::All,
        }
    }

    /// Convert to settings string format (mode, pack, lessons)
    pub fn to_settings(&self) -> (&'static str, String, String) {
        match self {
            StudyFilterMode::All => ("all", String::new(), String::new()),
            StudyFilterMode::HangulOnly => ("hangul", String::new(), String::new()),
            StudyFilterMode::PackOnly(pack) => ("pack", pack.clone(), String::new()),
            StudyFilterMode::PackLesson(pack, lesson) => ("pack", pack.clone(), lesson.to_string()),
        }
    }
}

/// Get the current study filter mode from settings
pub fn get_study_filter_mode(conn: &Connection) -> Result<StudyFilterMode> {
    let mode = crate::db::tiers::get_setting(conn, "study_filter_mode")?
        .unwrap_or_else(|| "all".to_string());
    let pack = crate::db::tiers::get_setting(conn, "study_filter_pack")?
        .unwrap_or_default();
    let lessons = crate::db::tiers::get_setting(conn, "study_filter_lessons")?
        .unwrap_or_default();

    Ok(StudyFilterMode::from_settings(&mode, &pack, &lessons))
}

/// Set the study filter mode in settings
pub fn set_study_filter_mode(conn: &Connection, mode: &StudyFilterMode) -> Result<()> {
    let (mode_str, pack, lessons) = mode.to_settings();
    crate::db::tiers::set_setting(conn, "study_filter_mode", mode_str)?;
    crate::db::tiers::set_setting(conn, "study_filter_pack", &pack)?;
    crate::db::tiers::set_setting(conn, "study_filter_lessons", &lessons)?;
    Ok(())
}

/// Build SQL WHERE clause fragment for study filter
/// Returns (clause, params, skip_tier_filter) where:
/// - clause: SQL WHERE fragment
/// - params: bind values (currently unused but kept for future)
/// - skip_tier_filter: if true, don't apply the default Hangul tier filter (1-4)
///
/// The app_conn and user_id parameters are used to verify pack access permissions.
/// Only packs that the user has permission to access will be included.
pub fn build_filter_where_clause(
    conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
    mode: &StudyFilterMode,
) -> Result<(String, Vec<String>, bool)> {
    match mode {
        StudyFilterMode::All => {
            // Include Hangul cards (tier-filtered) + cards from accessible packs (lesson-filtered)
            // For global packs: permission = access (no need to "enable")
            let accessible_packs = crate::auth::db::list_accessible_pack_ids(app_conn, user_id)
                .unwrap_or_default();

            // Get effective Hangul tiers
            let effective_tiers = crate::db::tiers::get_effective_tiers(conn)?;
            let tier_list = if effective_tiers.is_empty() {
                "0".to_string() // No tiers = no Hangul cards
            } else {
                effective_tiers
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            };

            if accessible_packs.is_empty() {
                // No packs accessible, only Hangul cards with tier filter
                return Ok((
                    format!("AND cd.pack_id IS NULL AND cd.tier IN ({})", tier_list),
                    vec![],
                    true, // Skip the caller's tier filter since we handle it here
                ));
            }

            // Build pack conditions for each accessible pack
            // Filter out any pack_ids that don't pass safety validation (defense-in-depth)
            let mut pack_conditions = Vec::new();
            for pack_id in &accessible_packs {
                if !is_safe_pack_id(pack_id) {
                    tracing::warn!("Skipping unsafe pack_id in study filter: {:?}", pack_id);
                    continue;
                }
                let is_accel = is_pack_accelerated(conn, pack_id)?;
                if is_accel {
                    pack_conditions.push(format!("cd.pack_id = '{}'", pack_id));
                } else {
                    let max_lesson = get_max_unlocked_lesson(conn, pack_id)?;
                    pack_conditions.push(format!(
                        "(cd.pack_id = '{}' AND (cd.lesson IS NULL OR cd.lesson <= {}))",
                        pack_id, max_lesson
                    ));
                }
            }

            let pack_filter = pack_conditions.join(" OR ");
            // Include Hangul (with tier filter) OR any accessible pack (with lesson filter)
            Ok((
                format!(
                    "AND ((cd.pack_id IS NULL AND cd.tier IN ({})) OR ({}))",
                    tier_list, pack_filter
                ),
                vec![],
                true, // Skip caller's tier filter since we handle everything here
            ))
        }
        StudyFilterMode::HangulOnly => {
            // Only baseline content (no pack_id or lesson is NULL)
            // Use tier filter (tiers 1-4 only)
            Ok(("AND (cd.pack_id IS NULL OR cd.lesson IS NULL)".to_string(), vec![], false))
        }
        StudyFilterMode::PackOnly(pack_id) => {
            // Defense-in-depth: validate pack_id format before SQL interpolation
            if !is_safe_pack_id(pack_id) {
                tracing::warn!("Unsafe pack_id in PackOnly filter: {:?}", pack_id);
                return build_filter_where_clause(conn, app_conn, user_id, &StudyFilterMode::All);
            }

            // Check if user has permission to access this pack
            if !crate::auth::db::can_user_access_pack(app_conn, user_id, pack_id).unwrap_or(false) {
                // No access, fall back to All mode
                return build_filter_where_clause(conn, app_conn, user_id, &StudyFilterMode::All);
            }

            // Get all unlocked lessons for this pack
            // Skip tier filter - pack filter is sufficient
            let is_accel = is_pack_accelerated(conn, pack_id)?;
            if is_accel {
                // All lessons available
                Ok((format!("AND cd.pack_id = '{}'", pack_id), vec![], true))
            } else {
                let max_lesson = get_max_unlocked_lesson(conn, pack_id)?;
                Ok((
                    format!(
                        "AND cd.pack_id = '{}' AND cd.lesson <= {}",
                        pack_id,
                        max_lesson
                    ),
                    vec![],
                    true,
                ))
            }
        }
        StudyFilterMode::PackLesson(pack_id, lesson) => {
            // Defense-in-depth: validate pack_id format before SQL interpolation
            if !is_safe_pack_id(pack_id) {
                tracing::warn!("Unsafe pack_id in PackLesson filter: {:?}", pack_id);
                return build_filter_where_clause(conn, app_conn, user_id, &StudyFilterMode::All);
            }

            // Check if user has permission to access this pack
            if !crate::auth::db::can_user_access_pack(app_conn, user_id, pack_id).unwrap_or(false) {
                // No access, fall back to All mode
                return build_filter_where_clause(conn, app_conn, user_id, &StudyFilterMode::All);
            }

            // Check if this lesson is unlocked
            // Skip tier filter - pack filter is sufficient
            if !is_lesson_unlocked(conn, pack_id, *lesson)? {
                // Lesson not unlocked, fall back to pack-only mode
                build_filter_where_clause(conn, app_conn, user_id, &StudyFilterMode::PackOnly(pack_id.clone()))
            } else {
                Ok((
                    format!(
                        "AND cd.pack_id = '{}' AND cd.lesson = {}",
                        pack_id,
                        lesson
                    ),
                    vec![],
                    true,
                ))
            }
        }
    }
}

/// Get list of pack IDs that should be included in "All" mode
/// This includes enabled packs that the user has access to
pub fn get_enabled_pack_ids_for_study(user_conn: &Connection) -> Vec<String> {
    crate::content::cards::list_enabled_packs(user_conn)
}

/// Get enabled packs that the user actually has permission to access.
/// Filters the user's enabled_packs through the app.db permission system.
pub fn list_enabled_packs_with_access(
    user_conn: &Connection,
    app_conn: &Connection,
    user_id: i64,
) -> Vec<String> {
    let enabled = crate::content::cards::list_enabled_packs(user_conn);
    enabled
        .into_iter()
        .filter(|pack_id| {
            crate::auth::db::can_user_access_pack(app_conn, user_id, pack_id).unwrap_or(false)
        })
        .collect()
}

/// Progress information for a single lesson within a pack
#[derive(Debug, Clone, Serialize)]
pub struct LessonProgress {
    pub lesson: u8,
    pub label: Option<String>,
    pub total: i64,
    pub new_cards: i64,
    pub learning: i64,
    pub learned: i64,
    pub is_unlocked: bool,
    pub percentage: i64,
}

impl LessonProgress {
    pub fn calculate_percentage(learned: i64, total: i64) -> i64 {
        if total > 0 {
            (learned * 100) / total
        } else {
            0
        }
    }
}

/// Pack progress with all lessons
#[derive(Debug, Clone, Serialize)]
pub struct PackProgress {
    pub pack_id: String,
    pub display_name: String,
    pub unit_name: String,
    pub section_prefix: String,
    pub lessons: Vec<LessonProgress>,
    pub total_cards: i64,
    pub total_learned: i64,
    pub is_accelerated: bool,
    pub can_unlock_next: bool,
}

impl PackProgress {
    pub fn overall_percentage(&self) -> i64 {
        if self.total_cards > 0 {
            (self.total_learned * 100) / self.total_cards
        } else {
            0
        }
    }
}

/// UI metadata loaded from pack manifest
#[derive(Debug, Clone, Deserialize)]
pub struct PackUiMetadata {
    pub pack_id: String,
    pub display_name: String,
    pub unit_name: String,
    pub section_prefix: String,
    pub lesson_labels: Option<std::collections::HashMap<String, String>>,
    pub unlock_threshold: u8,
    pub total_lessons: Option<u8>,
    pub progress_section_title: Option<String>,
    pub study_filter_label: Option<String>,
}

// ==================== Settings Access ====================

/// Check if a pack is in accelerated mode (all lessons unlocked)
pub fn is_pack_accelerated(conn: &Connection, pack_id: &str) -> Result<bool> {
    let value = crate::db::tiers::get_setting(conn, "accelerated_packs")?;
    Ok(value
        .map(|v| v.split(',').any(|p| p.trim() == pack_id))
        .unwrap_or(false))
}

/// Set accelerated mode for a pack
pub fn set_pack_accelerated(conn: &Connection, pack_id: &str, accelerated: bool) -> Result<()> {
    let current = crate::db::tiers::get_setting(conn, "accelerated_packs")?
        .unwrap_or_default();

    let mut packs: Vec<&str> = current.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != pack_id)
        .collect();

    if accelerated {
        packs.push(pack_id);
    }

    crate::db::tiers::set_setting(conn, "accelerated_packs", &packs.join(","))
}

// ==================== Lesson Unlock Management ====================

/// Get the maximum unlocked lesson for a pack
pub fn get_max_unlocked_lesson(conn: &Connection, pack_id: &str) -> Result<u8> {
    let max: Option<i64> = conn
        .query_row(
            "SELECT MAX(lesson) FROM pack_lesson_progress WHERE pack_id = ?1 AND unlocked = 1",
            params![pack_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    // Default to lesson 1 being unlocked
    Ok(max.map(|m| m as u8).unwrap_or(1))
}

/// Check if a specific lesson is unlocked
pub fn is_lesson_unlocked(conn: &Connection, pack_id: &str, lesson: u8) -> Result<bool> {
    // Lesson 1 is always unlocked
    if lesson == 1 {
        return Ok(true);
    }

    // Check if accelerated mode
    if is_pack_accelerated(conn, pack_id)? {
        return Ok(true);
    }

    let unlocked: i64 = conn
        .query_row(
            "SELECT COALESCE(unlocked, 0) FROM pack_lesson_progress WHERE pack_id = ?1 AND lesson = ?2",
            params![pack_id, lesson],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(unlocked == 1)
}

/// Unlock a specific lesson
pub fn unlock_lesson(conn: &Connection, pack_id: &str, lesson: u8) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        r#"INSERT INTO pack_lesson_progress (pack_id, lesson, unlocked, unlocked_at)
           VALUES (?1, ?2, 1, ?3)
           ON CONFLICT(pack_id, lesson) DO UPDATE SET unlocked = 1, unlocked_at = ?3"#,
        params![pack_id, lesson, now],
    )?;
    Ok(())
}

/// Try to auto-unlock the next lesson if current lesson is >= threshold
/// Returns the newly unlocked lesson number, or None if no unlock occurred
pub fn try_auto_unlock_lesson(
    conn: &Connection,
    app_conn: &Connection,
    pack_id: &str,
    threshold: u8,
    total_lessons: u8,
) -> Result<Option<u8>> {
    // Don't auto-unlock if accelerated
    if is_pack_accelerated(conn, pack_id)? {
        return Ok(None);
    }

    let current_max = get_max_unlocked_lesson(conn, pack_id)?;
    if current_max >= total_lessons {
        return Ok(None);
    }

    // Check if current max lesson has >= threshold% learned
    let progress = get_lesson_progress(conn, app_conn, pack_id, current_max)?;
    if progress.percentage >= threshold as i64 {
        let next_lesson = current_max + 1;
        unlock_lesson(conn, pack_id, next_lesson)?;
        tracing::info!(
            "Auto-unlocked {} lesson {} (previous at {}%)",
            pack_id, next_lesson, progress.percentage
        );
        return Ok(Some(next_lesson));
    }

    Ok(None)
}

// ==================== Progress Queries ====================

/// SQL fragment for cross-DB join (learning.db with attached app.db)
const LESSON_FROM: &str = r#"
FROM app.card_definitions cd
LEFT JOIN card_progress cp ON cp.card_id = cd.id
"#;

/// Get progress for a single lesson
pub fn get_lesson_progress(
    conn: &Connection,
    _app_conn: &Connection,  // App DB should be attached to conn as 'app'
    pack_id: &str,
    lesson: u8,
) -> Result<LessonProgress> {
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) {} WHERE cd.pack_id = ?1 AND cd.lesson = ?2", LESSON_FROM),
        params![pack_id, lesson],
        |row| row.get(0),
    )?;

    let new_cards: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) {} WHERE cd.pack_id = ?1 AND cd.lesson = ?2 AND COALESCE(cp.total_reviews, 0) = 0",
            LESSON_FROM
        ),
        params![pack_id, lesson],
        |row| row.get(0),
    )?;

    let learning: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) {} WHERE cd.pack_id = ?1 AND cd.lesson = ?2 AND COALESCE(cp.total_reviews, 0) > 0 AND COALESCE(cp.repetitions, 0) < 2",
            LESSON_FROM
        ),
        params![pack_id, lesson],
        |row| row.get(0),
    )?;

    let learned: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) {} WHERE cd.pack_id = ?1 AND cd.lesson = ?2 AND COALESCE(cp.repetitions, 0) >= 2",
            LESSON_FROM
        ),
        params![pack_id, lesson],
        |row| row.get(0),
    )?;

    let is_unlocked = is_lesson_unlocked(conn, pack_id, lesson)?;
    let percentage = LessonProgress::calculate_percentage(learned, total);

    Ok(LessonProgress {
        lesson,
        label: None,  // Caller should fill in from UI metadata
        total,
        new_cards,
        learning,
        learned,
        is_unlocked,
        percentage,
    })
}

/// Get progress for all lessons in a pack
pub fn get_pack_progress(
    conn: &Connection,
    app_conn: &Connection,
    pack_id: &str,
    ui_metadata: &PackUiMetadata,
) -> Result<PackProgress> {
    let total_lessons = ui_metadata.total_lessons.unwrap_or(1);
    let is_accelerated = is_pack_accelerated(conn, pack_id)?;

    let mut lessons = Vec::new();
    let mut total_cards = 0i64;
    let mut total_learned = 0i64;

    for lesson_num in 1..=total_lessons {
        let mut progress = get_lesson_progress(conn, app_conn, pack_id, lesson_num)?;

        // Fill in label from UI metadata
        if let Some(ref labels) = ui_metadata.lesson_labels {
            progress.label = labels.get(&lesson_num.to_string()).cloned();
        }

        // If accelerated, all lessons are unlocked
        if is_accelerated {
            progress.is_unlocked = true;
        }

        total_cards += progress.total;
        total_learned += progress.learned;
        lessons.push(progress);
    }

    // Check if can unlock next
    let max_unlocked = get_max_unlocked_lesson(conn, pack_id)?;
    let can_unlock_next = if max_unlocked < total_lessons && !is_accelerated {
        if let Some(current) = lessons.iter().find(|l| l.lesson == max_unlocked) {
            current.percentage >= ui_metadata.unlock_threshold as i64
        } else {
            false
        }
    } else {
        false
    };

    Ok(PackProgress {
        pack_id: pack_id.to_string(),
        display_name: ui_metadata.display_name.clone(),
        unit_name: ui_metadata.unit_name.clone(),
        section_prefix: ui_metadata.section_prefix.clone(),
        lessons,
        total_cards,
        total_learned,
        is_accelerated,
        can_unlock_next,
    })
}

// ==================== UI Metadata Operations ====================

/// Store UI metadata for a pack (called when pack is enabled)
pub fn store_pack_ui_metadata(
    app_conn: &Connection,
    pack_id: &str,
    ui: &crate::content::packs::PackUiConfig,
    total_lessons: Option<u8>,
) -> Result<()> {
    let lesson_labels_json = serde_json::to_string(&ui.lesson_labels)
        .unwrap_or_else(|_| "{}".to_string());

    app_conn.execute(
        r#"INSERT OR REPLACE INTO pack_ui_metadata
           (pack_id, display_name, unit_name, section_prefix, lesson_labels,
            unlock_threshold, total_lessons, progress_section_title, study_filter_label)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
        params![
            pack_id,
            ui.display_name,
            ui.unit_name,
            ui.section_prefix,
            lesson_labels_json,
            ui.unlock_threshold,
            total_lessons,
            ui.progress_section_title,
            ui.study_filter_label,
        ],
    )?;
    Ok(())
}

/// Remove UI metadata for a pack (called when pack is disabled)
pub fn remove_pack_ui_metadata(app_conn: &Connection, pack_id: &str) -> Result<()> {
    app_conn.execute(
        "DELETE FROM pack_ui_metadata WHERE pack_id = ?1",
        params![pack_id],
    )?;
    Ok(())
}

/// Get UI metadata for a pack
pub fn get_pack_ui_metadata(app_conn: &Connection, pack_id: &str) -> Result<Option<PackUiMetadata>> {
    let result = app_conn.query_row(
        r#"SELECT pack_id, display_name, unit_name, section_prefix, lesson_labels,
                  unlock_threshold, total_lessons, progress_section_title, study_filter_label
           FROM pack_ui_metadata WHERE pack_id = ?1"#,
        params![pack_id],
        |row| {
            let labels_json: Option<String> = row.get(4)?;
            let lesson_labels = labels_json
                .and_then(|j| serde_json::from_str(&j).ok());

            Ok(PackUiMetadata {
                pack_id: row.get(0)?,
                display_name: row.get(1)?,
                unit_name: row.get(2)?,
                section_prefix: row.get(3)?,
                lesson_labels,
                unlock_threshold: row.get::<_, i64>(5)? as u8,
                total_lessons: row.get::<_, Option<i64>>(6)?.map(|n| n as u8),
                progress_section_title: row.get(7)?,
                study_filter_label: row.get(8)?,
            })
        },
    );

    match result {
        Ok(metadata) => Ok(Some(metadata)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Get all packs with UI metadata (enabled packs with lesson-based progression)
pub fn get_all_packs_with_lessons(app_conn: &Connection) -> Result<Vec<PackUiMetadata>> {
    let mut stmt = app_conn.prepare(
        r#"SELECT pack_id, display_name, unit_name, section_prefix, lesson_labels,
                  unlock_threshold, total_lessons, progress_section_title, study_filter_label
           FROM pack_ui_metadata
           WHERE total_lessons IS NOT NULL AND total_lessons > 0
           ORDER BY display_name"#,
    )?;

    let packs = stmt.query_map([], |row| {
        let labels_json: Option<String> = row.get(4)?;
        let lesson_labels = labels_json
            .and_then(|j| serde_json::from_str(&j).ok());

        Ok(PackUiMetadata {
            pack_id: row.get(0)?,
            display_name: row.get(1)?,
            unit_name: row.get(2)?,
            section_prefix: row.get(3)?,
            lesson_labels,
            unlock_threshold: row.get::<_, i64>(5)? as u8,
            total_lessons: row.get::<_, Option<i64>>(6)?.map(|n| n as u8),
            progress_section_title: row.get(7)?,
            study_filter_label: row.get(8)?,
        })
    })?
    .filter_map(|r| r.ok())
    .collect();

    Ok(packs)
}

/// Try to auto-unlock lessons for all enabled packs that have lesson progression
/// Returns list of (pack_id, unlocked_lesson) for any newly unlocked lessons
pub fn try_auto_unlock_all_pack_lessons(
    conn: &Connection,
    app_conn: &Connection,
) -> Result<Vec<(String, u8)>> {
    let mut unlocked = Vec::new();

    // Get all packs with lesson progression
    let packs = get_all_packs_with_lessons(app_conn)?;

    for pack in packs {
        if let Some(total) = pack.total_lessons
            && let Ok(Some(lesson)) = try_auto_unlock_lesson(
                conn,
                app_conn,
                &pack.pack_id,
                pack.unlock_threshold,
                total,
            ) {
                unlocked.push((pack.pack_id, lesson));
            }
    }

    Ok(unlocked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_dbs() -> (TempDir, Connection, Connection) {
        let temp = TempDir::new().unwrap();

        // Create app.db
        let app_db_path = temp.path().join("app.db");
        let app_conn = Connection::open(&app_db_path).unwrap();
        app_conn.execute_batch(r#"
            CREATE TABLE card_definitions (
                id INTEGER PRIMARY KEY,
                pack_id TEXT,
                lesson INTEGER,
                front TEXT,
                tier INTEGER
            );
            CREATE TABLE pack_ui_metadata (
                pack_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                unit_name TEXT DEFAULT 'Lessons',
                section_prefix TEXT DEFAULT 'Lesson',
                lesson_labels TEXT,
                unlock_threshold INTEGER DEFAULT 80,
                total_lessons INTEGER,
                progress_section_title TEXT,
                study_filter_label TEXT
            );
        "#).unwrap();

        // Create learning.db with app.db attached
        let user_db_path = temp.path().join("learning.db");
        let user_conn = Connection::open(&user_db_path).unwrap();
        user_conn.execute_batch(r#"
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT);
            CREATE TABLE card_progress (
                card_id INTEGER PRIMARY KEY,
                total_reviews INTEGER DEFAULT 0,
                repetitions INTEGER DEFAULT 0
            );
            CREATE TABLE pack_lesson_progress (
                pack_id TEXT NOT NULL,
                lesson INTEGER NOT NULL,
                unlocked INTEGER NOT NULL DEFAULT 0,
                unlocked_at TEXT,
                PRIMARY KEY (pack_id, lesson)
            );
        "#).unwrap();

        // Attach app.db
        user_conn.execute(
            &format!("ATTACH DATABASE '{}' AS app", app_db_path.display()),
            [],
        ).unwrap();

        (temp, app_conn, user_conn)
    }

    #[test]
    fn test_lesson_unlock() {
        let (_temp, _app_conn, user_conn) = create_test_dbs();

        // Lesson 1 should always be unlocked
        assert!(is_lesson_unlocked(&user_conn, "test-pack", 1).unwrap());

        // Lesson 2 should not be unlocked initially
        assert!(!is_lesson_unlocked(&user_conn, "test-pack", 2).unwrap());

        // Unlock lesson 2
        unlock_lesson(&user_conn, "test-pack", 2).unwrap();
        assert!(is_lesson_unlocked(&user_conn, "test-pack", 2).unwrap());

        // Max unlocked should be 2
        assert_eq!(get_max_unlocked_lesson(&user_conn, "test-pack").unwrap(), 2);
    }

    #[test]
    fn test_accelerated_mode() {
        let (_temp, _app_conn, user_conn) = create_test_dbs();

        // Initially not accelerated
        assert!(!is_pack_accelerated(&user_conn, "test-pack").unwrap());

        // Enable accelerated
        set_pack_accelerated(&user_conn, "test-pack", true).unwrap();
        assert!(is_pack_accelerated(&user_conn, "test-pack").unwrap());

        // All lessons should be unlocked
        assert!(is_lesson_unlocked(&user_conn, "test-pack", 5).unwrap());

        // Disable accelerated
        set_pack_accelerated(&user_conn, "test-pack", false).unwrap();
        assert!(!is_pack_accelerated(&user_conn, "test-pack").unwrap());
    }

    #[test]
    fn test_pack_ui_metadata() {
        let (_temp, app_conn, _user_conn) = create_test_dbs();

        let ui = crate::content::packs::PackUiConfig {
            display_name: "Test Pack".to_string(),
            unit_name: "Units".to_string(),
            section_prefix: "Unit".to_string(),
            lesson_labels: [("1".to_string(), "Intro".to_string())].into_iter().collect(),
            unlock_threshold: 75,
            progress_section_title: Some("Test Progress".to_string()),
            study_filter_label: Some("Test".to_string()),
        };

        store_pack_ui_metadata(&app_conn, "test-pack", &ui, Some(5)).unwrap();

        let loaded = get_pack_ui_metadata(&app_conn, "test-pack").unwrap().unwrap();
        assert_eq!(loaded.display_name, "Test Pack");
        assert_eq!(loaded.unit_name, "Units");
        assert_eq!(loaded.unlock_threshold, 75);
        assert_eq!(loaded.total_lessons, Some(5));
        assert_eq!(loaded.lesson_labels.unwrap().get("1"), Some(&"Intro".to_string()));
    }
}
