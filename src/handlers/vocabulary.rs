//! Vocabulary library handler for passive vocabulary browsing.
//!
//! Displays vocabulary content organized by lesson with collapsible entries
//! showing rich metadata (common usages, notes, examples).

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect, Response};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::Path;

use crate::auth::AuthContext;
use crate::filters;
use crate::handlers::NavContext;
use crate::services::pack_manager::{self, PackFilter};
use crate::state::AppState;

/// SRS learning status for a vocabulary entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SrsStatus {
    /// Never reviewed
    New,
    /// Actively being drilled (total_reviews > 0, learning_step < 4)
    Learning,
    /// Graduated from learning steps (learning_step >= 4)
    Graduated,
}

impl SrsStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SrsStatus::New => "new",
            SrsStatus::Learning => "learning",
            SrsStatus::Graduated => "graduated",
        }
    }
}

/// Vocabulary entry with full metadata from vocabulary.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VocabularyEntry {
    pub term: String,
    pub romanization: String,
    pub translation: String,
    pub word_type: String,
    #[serde(default)]
    pub lesson: u8,
    #[serde(default)]
    pub page: u8,
    #[serde(default)]
    pub common_usages: Vec<Usage>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub examples: Vec<Example>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Usage {
    pub korean: String,
    pub english: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Example {
    pub korean: String,
    pub english: String,
}

/// Vocabulary entries grouped by lesson
pub struct LessonGroup {
    pub lesson: u8,
    pub entries: Vec<VocabularyEntry>,
}

/// Flattened, searchable entry for Fuse.js (includes all text fields)
#[derive(Debug, Clone, Serialize)]
pub struct SearchableEntry {
    pub id: String,
    pub term: String,
    pub romanization: String,
    pub translation: String,
    pub notes: String,
    pub word_type: String,
    pub lesson: u8,
    pub pack_id: String,
    pub usages_text: String,
    pub examples_text: String,
}

/// A pack with its vocabulary content grouped by lesson
pub struct PackGroup {
    pub pack_id: String,
    pub pack_name: String,
    pub pack_description: Option<String>,
    pub lessons: Vec<LessonGroup>,
    pub word_count: usize,
}

/// TOC item for pack navigation
pub struct PackTocItem {
    pub id: String,
    pub name: String,
    pub word_count: usize,
    pub lessons: Vec<LessonTocItem>,
}

/// TOC item for lesson navigation within a pack
pub struct LessonTocItem {
    pub id: String,
    pub short_label: String,
    pub full_label: String,
    pub count: usize,
}

#[derive(Template)]
#[template(path = "library/vocabulary.html")]
pub struct VocabularyTemplate {
    pub pack_enabled: bool,
    pub packs: Vec<PackGroup>,
    pub toc_items: Vec<PackTocItem>,
    pub total_count: usize,
    pub nav: NavContext,
    /// JSON array of searchable entries for client-side Fuse.js search
    pub vocabulary_json: String,
    /// Use local Fuse.js instead of CDN (set via USE_LOCAL_FUSE env var for tests)
    pub use_local_fuse: bool,
}

/// Load vocabulary from a pack's vocabulary source.
///
/// Supports two formats:
/// 1. Single `vocabulary.json` file at pack root (legacy format)
/// 2. `vocabulary/` directory with `lesson_*.json` files (directory format)
fn load_vocabulary_from_path(pack_path: &Path) -> Option<Vec<VocabularyEntry>> {
    // Try single vocabulary.json first (backward compatible)
    let vocab_file = pack_path.join("vocabulary.json");
    if vocab_file.exists() {
        let content = fs::read_to_string(&vocab_file).ok()?;
        return serde_json::from_str(&content).ok();
    }

    // Try vocabulary directory with lesson_*.json files
    let vocab_dir = pack_path.join("vocabulary");
    if vocab_dir.is_dir() {
        let mut all_entries = Vec::new();

        if let Ok(dir_entries) = fs::read_dir(&vocab_dir) {
            for entry in dir_entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                        if name.starts_with("lesson_") {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(entries) =
                                    serde_json::from_str::<Vec<VocabularyEntry>>(&content)
                                {
                                    all_entries.extend(entries);
                                }
                            }
                        }
                    }
                }
            }
        }

        if !all_entries.is_empty() {
            return Some(all_entries);
        }
    }

    None
}

/// Build searchable entries from pack groups for Fuse.js client-side search
fn build_searchable_entries(packs: &[PackGroup]) -> Vec<SearchableEntry> {
    let mut entries = Vec::new();

    for pack in packs {
        for lesson_group in &pack.lessons {
            for (entry_idx, entry) in lesson_group.entries.iter().enumerate() {
                // Flatten common_usages to searchable text
                let usages_text: String = entry
                    .common_usages
                    .iter()
                    .map(|u| format!("{} {}", u.korean, u.english))
                    .collect::<Vec<_>>()
                    .join(" ");

                // Flatten examples to searchable text
                let examples_text: String = entry
                    .examples
                    .iter()
                    .map(|e| format!("{} {}", e.korean, e.english))
                    .collect::<Vec<_>>()
                    .join(" ");

                // ID format matches template: pack_id-lesson-entry_index (0-based within lesson)
                entries.push(SearchableEntry {
                    id: format!("{}-{}-{}", pack.pack_id, lesson_group.lesson, entry_idx),
                    term: entry.term.clone(),
                    romanization: entry.romanization.clone(),
                    translation: entry.translation.clone(),
                    notes: entry.notes.clone().unwrap_or_default(),
                    word_type: entry.word_type.clone(),
                    lesson: lesson_group.lesson,
                    pack_id: pack.pack_id.clone(),
                    usages_text,
                    examples_text,
                });
            }
        }
    }

    entries
}

/// Fetch SRS status for vocabulary cards in the given packs.
/// Returns a map of (pack_id, lesson, front) -> SrsStatus.
/// Uses the user's learning.db connection which has app.db attached.
fn fetch_vocab_srs_statuses(
    conn: &Connection,
    pack_ids: &[String],
) -> HashMap<(String, u8, String), SrsStatus> {
    let mut map = HashMap::new();
    if pack_ids.is_empty() {
        return map;
    }

    let placeholders: Vec<String> = (1..=pack_ids.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        r#"SELECT cd.pack_id, cd.lesson, cd.front,
                  COALESCE(cp.total_reviews, 0) as total_reviews,
                  COALESCE(cp.learning_step, 0) as learning_step
           FROM app.card_definitions cd
           LEFT JOIN card_progress cp ON cp.card_id = cd.id
           WHERE cd.pack_id IN ({})
             AND cd.is_reverse = 0"#,
        placeholders.join(",")
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Failed to prepare SRS status query: {e}");
            return map;
        }
    };

    let params: Vec<&dyn rusqlite::ToSql> =
        pack_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

    let rows = match stmt.query_map(params.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, u8>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to query SRS statuses: {e}");
            return map;
        }
    };

    for row in rows.flatten() {
        let (pack_id, lesson, front, total_reviews, learning_step) = row;
        let status = if total_reviews == 0 {
            SrsStatus::New
        } else if learning_step >= 4 {
            SrsStatus::Graduated
        } else {
            SrsStatus::Learning
        };
        map.insert((pack_id, lesson, front), status);
    }

    map
}

/// Vocabulary library page handler
pub async fn vocabulary_library(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
                .into_response()
        }
    };

    // Check if any vocabulary packs exist at all (for redirect vs "not enabled")
    if !pack_manager::any_accessible_pack_provides(&app_conn, auth.user_id, "vocabulary") {
        // Check if vocabulary packs exist but user can't access them
        let all_packs = pack_manager::discover_all_packs(&app_conn);
        let vocab_exists = all_packs
            .iter()
            .any(|p| p.manifest.provides.iter().any(|t| t == "vocabulary"));

        if !vocab_exists {
            // No vocabulary packs installed at all - redirect to library
            return Redirect::to("/library").into_response();
        }

        // Vocabulary packs exist but user can't access any
        let template = VocabularyTemplate {
            pack_enabled: false,
            packs: vec![],
            toc_items: vec![],
            total_count: 0,
            nav: NavContext::from_auth(&auth),
            vocabulary_json: "[]".to_string(),
            use_local_fuse: env::var("USE_LOCAL_FUSE").is_ok(),
        };
        return Html(template.render().unwrap_or_default()).into_response();
    }

    // Get accessible vocabulary packs using PackManager
    let accessible_packs = pack_manager::get_accessible_packs(
        &app_conn,
        auth.user_id,
        Some(PackFilter::provides("vocabulary")),
    );

    // Build pack groups with vocabulary content
    let mut pack_groups: Vec<PackGroup> = Vec::new();
    let mut toc_items: Vec<PackTocItem> = Vec::new();
    let mut total_count = 0;

    for pack in &accessible_packs {
        if let Some(vocab_entries) = load_vocabulary_from_path(&pack.path) {
            if vocab_entries.is_empty() {
                continue;
            }

            let word_count = vocab_entries.len();
            total_count += word_count;

            // Group entries by lesson within this pack
            let mut lesson_map: BTreeMap<u8, Vec<VocabularyEntry>> = BTreeMap::new();
            for entry in vocab_entries {
                lesson_map.entry(entry.lesson).or_default().push(entry);
            }

            // Convert to Vec<LessonGroup>
            let lessons: Vec<LessonGroup> = lesson_map
                .into_iter()
                .map(|(lesson, entries)| LessonGroup { lesson, entries })
                .collect();

            // Build lesson TOC items for this pack
            let lesson_toc_items: Vec<LessonTocItem> = lessons
                .iter()
                .map(|g| LessonTocItem {
                    id: format!("{}-lesson-{}", pack.manifest.id, g.lesson),
                    short_label: format!("L{}", g.lesson),
                    full_label: format!("Lesson {} ({})", g.lesson, g.entries.len()),
                    count: g.entries.len(),
                })
                .collect();

            toc_items.push(PackTocItem {
                id: pack.manifest.id.clone(),
                name: pack.manifest.name.clone(),
                word_count,
                lessons: lesson_toc_items,
            });

            pack_groups.push(PackGroup {
                pack_id: pack.manifest.id.clone(),
                pack_name: pack.manifest.name.clone(),
                pack_description: pack.manifest.description.clone(),
                lessons,
                word_count,
            });
        }
    }

    // Build searchable entries for client-side search
    let searchable_entries = build_searchable_entries(&pack_groups);
    let vocabulary_json = serde_json::to_string(&searchable_entries).unwrap_or_else(|_| "[]".to_string());

    let template = VocabularyTemplate {
        pack_enabled: !pack_groups.is_empty(),
        packs: pack_groups,
        toc_items,
        total_count,
        nav: NavContext::from_auth(&auth),
        vocabulary_json,
        use_local_fuse: env::var("USE_LOCAL_FUSE").is_ok(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}

/// API endpoint to lazily fetch SRS statuses for vocabulary cards.
/// Returns JSON map of "pack_id|lesson|term" -> status string.
/// Called on-demand by client-side JS when the "Show Learning" toggle is activated.
pub async fn vocabulary_srs_statuses(auth: AuthContext) -> Response {
    let conn = match auth.user_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            return axum::Json(HashMap::<String, String>::new()).into_response();
        }
    };

    // Get all accessible pack IDs for this user
    let pack_ids: Vec<String> = match conn.prepare(
        "SELECT DISTINCT cd.pack_id FROM app.card_definitions cd WHERE cd.pack_id IS NOT NULL",
    ) {
        Ok(mut stmt) => stmt
            .query_map([], |row| row.get(0))
            .ok()
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default(),
        Err(_) => return axum::Json(HashMap::<String, String>::new()).into_response(),
    };

    let statuses = fetch_vocab_srs_statuses(&conn, &pack_ids);

    // Convert to JSON-friendly format: "pack_id|lesson|term" -> "learning"
    let json_map: HashMap<String, String> = statuses
        .into_iter()
        .filter(|(_, status)| *status != SrsStatus::New)
        .map(|((pack_id, lesson, term), status)| {
            (format!("{pack_id}|{lesson}|{term}"), status.as_str().to_string())
        })
        .collect();

    axum::Json(json_map).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// Helper: create a TestEnv and ATTACH app.db to user_conn for cross-DB queries
    fn setup_test_env() -> crate::testing::TestEnv {
        let env = crate::testing::TestEnv::new().unwrap();

        // ATTACH app.db to user_conn (same as production middleware does)
        let app_db_path = env.temp.path().join("app.db");
        env.user_conn
            .execute(
                &format!(
                    "ATTACH DATABASE '{}' AS app",
                    app_db_path.to_str().unwrap()
                ),
                [],
            )
            .unwrap();

        env
    }

    /// Helper: insert a content pack into app.db
    fn insert_pack(conn: &Connection, pack_id: &str) {
        conn.execute(
            "INSERT INTO content_packs (id, name, pack_type, scope, source_path, installed_at)
             VALUES (?1, ?1, 'cards', 'standard', '/test', datetime('now'))",
            params![pack_id],
        )
        .unwrap();
    }

    /// Helper: insert a card definition into app.db
    fn insert_card(
        conn: &Connection,
        id: i64,
        front: &str,
        main_answer: &str,
        pack_id: &str,
        lesson: u8,
        is_reverse: bool,
    ) {
        conn.execute(
            "INSERT INTO card_definitions (id, front, main_answer, description, card_type, tier, pack_id, lesson, is_reverse)
             VALUES (?1, ?2, ?3, '', 'Vocabulary', 5, ?4, ?5, ?6)",
            params![id, front, main_answer, pack_id, lesson, is_reverse as i32],
        )
        .unwrap();
    }

    /// Helper: insert card progress into learning.db
    fn insert_progress(conn: &Connection, card_id: i64, learning_step: i64, total_reviews: i64) {
        conn.execute(
            "INSERT INTO card_progress (card_id, learning_step, total_reviews, correct_reviews, ease_factor, interval_days, repetitions, next_review)
             VALUES (?1, ?2, ?3, 0, 2.5, 0, 0, datetime('now'))",
            params![card_id, learning_step, total_reviews],
        )
        .unwrap();
    }

    #[test]
    fn test_srs_status_as_str() {
        assert_eq!(SrsStatus::New.as_str(), "new");
        assert_eq!(SrsStatus::Learning.as_str(), "learning");
        assert_eq!(SrsStatus::Graduated.as_str(), "graduated");
    }

    #[test]
    fn test_fetch_srs_statuses_empty_packs() {
        let env = setup_test_env();
        let result = fetch_vocab_srs_statuses(&env.user_conn, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fetch_srs_statuses_no_progress() {
        let env = setup_test_env();
        insert_pack(&env.app_conn, "test_pack");
        insert_card(&env.app_conn, 1, "음식", "food", "test_pack", 3, false);
        insert_card(&env.app_conn, 2, "food", "음식", "test_pack", 3, true);

        let result =
            fetch_vocab_srs_statuses(&env.user_conn, &["test_pack".to_string()]);

        // Only forward card (is_reverse=0) should appear
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[&("test_pack".to_string(), 3, "음식".to_string())],
            SrsStatus::New
        );
    }

    #[test]
    fn test_fetch_srs_statuses_learning() {
        let env = setup_test_env();
        insert_pack(&env.app_conn, "test_pack");
        insert_card(&env.app_conn, 1, "음식", "food", "test_pack", 3, false);

        // Card has reviews but hasn't graduated (learning_step < 4)
        insert_progress(&env.user_conn, 1, 2, 3);

        let result =
            fetch_vocab_srs_statuses(&env.user_conn, &["test_pack".to_string()]);

        assert_eq!(
            result[&("test_pack".to_string(), 3, "음식".to_string())],
            SrsStatus::Learning
        );
    }

    #[test]
    fn test_fetch_srs_statuses_graduated() {
        let env = setup_test_env();
        insert_pack(&env.app_conn, "test_pack");
        insert_card(&env.app_conn, 1, "음식", "food", "test_pack", 3, false);

        // Card has graduated (learning_step >= 4)
        insert_progress(&env.user_conn, 1, 4, 10);

        let result =
            fetch_vocab_srs_statuses(&env.user_conn, &["test_pack".to_string()]);

        assert_eq!(
            result[&("test_pack".to_string(), 3, "음식".to_string())],
            SrsStatus::Graduated
        );
    }

    #[test]
    fn test_fetch_srs_statuses_mixed_states() {
        let env = setup_test_env();
        insert_pack(&env.app_conn, "test_pack");
        insert_card(&env.app_conn, 1, "음식", "food", "test_pack", 3, false);
        insert_card(&env.app_conn, 2, "케이크", "cake", "test_pack", 3, false);
        insert_card(&env.app_conn, 3, "공항", "airport", "test_pack", 3, false);

        // 음식: learning (step 1, 2 reviews)
        insert_progress(&env.user_conn, 1, 1, 2);
        // 케이크: graduated (step 4, 8 reviews)
        insert_progress(&env.user_conn, 2, 4, 8);
        // 공항: new (no progress row)

        let result =
            fetch_vocab_srs_statuses(&env.user_conn, &["test_pack".to_string()]);

        assert_eq!(result.len(), 3);
        assert_eq!(
            result[&("test_pack".to_string(), 3, "음식".to_string())],
            SrsStatus::Learning
        );
        assert_eq!(
            result[&("test_pack".to_string(), 3, "케이크".to_string())],
            SrsStatus::Graduated
        );
        assert_eq!(
            result[&("test_pack".to_string(), 3, "공항".to_string())],
            SrsStatus::New
        );
    }

    #[test]
    fn test_fetch_srs_statuses_excludes_reverse_cards() {
        let env = setup_test_env();
        insert_pack(&env.app_conn, "test_pack");
        // Forward card
        insert_card(&env.app_conn, 1, "음식", "food", "test_pack", 3, false);
        // Reverse card (English -> Korean)
        insert_card(&env.app_conn, 2, "food", "음식", "test_pack", 3, true);

        insert_progress(&env.user_conn, 1, 2, 5);
        insert_progress(&env.user_conn, 2, 4, 10);

        let result =
            fetch_vocab_srs_statuses(&env.user_conn, &["test_pack".to_string()]);

        // Should only include forward card
        assert_eq!(result.len(), 1);
        assert!(result.contains_key(&("test_pack".to_string(), 3, "음식".to_string())));
        assert!(!result.contains_key(&("test_pack".to_string(), 3, "food".to_string())));
    }

    #[test]
    fn test_fetch_srs_statuses_multiple_lessons() {
        let env = setup_test_env();
        insert_pack(&env.app_conn, "test_pack");
        insert_card(&env.app_conn, 1, "음식", "food", "test_pack", 3, false);
        insert_card(&env.app_conn, 2, "한국", "Korea", "test_pack", 1, false);

        insert_progress(&env.user_conn, 1, 2, 3);
        insert_progress(&env.user_conn, 2, 4, 12);

        let result =
            fetch_vocab_srs_statuses(&env.user_conn, &["test_pack".to_string()]);

        assert_eq!(result.len(), 2);
        assert_eq!(
            result[&("test_pack".to_string(), 3, "음식".to_string())],
            SrsStatus::Learning
        );
        assert_eq!(
            result[&("test_pack".to_string(), 1, "한국".to_string())],
            SrsStatus::Graduated
        );
    }

    #[test]
    fn test_fetch_srs_statuses_multiple_packs() {
        let env = setup_test_env();
        insert_pack(&env.app_conn, "pack_a");
        insert_pack(&env.app_conn, "pack_b");
        insert_card(&env.app_conn, 1, "음식", "food", "pack_a", 3, false);
        insert_card(&env.app_conn, 2, "한국", "Korea", "pack_b", 1, false);

        insert_progress(&env.user_conn, 1, 1, 2);

        let result = fetch_vocab_srs_statuses(
            &env.user_conn,
            &["pack_a".to_string(), "pack_b".to_string()],
        );

        assert_eq!(result.len(), 2);
        assert_eq!(
            result[&("pack_a".to_string(), 3, "음식".to_string())],
            SrsStatus::Learning
        );
        assert_eq!(
            result[&("pack_b".to_string(), 1, "한국".to_string())],
            SrsStatus::New
        );
    }

    #[test]
    fn test_fetch_srs_statuses_boundary_learning_step() {
        let env = setup_test_env();
        insert_pack(&env.app_conn, "test_pack");
        insert_card(&env.app_conn, 1, "a", "x", "test_pack", 1, false);
        insert_card(&env.app_conn, 2, "b", "y", "test_pack", 1, false);
        insert_card(&env.app_conn, 3, "c", "z", "test_pack", 1, false);

        // Step 3 = still learning (boundary)
        insert_progress(&env.user_conn, 1, 3, 4);
        // Step 4 = graduated (boundary)
        insert_progress(&env.user_conn, 2, 4, 5);
        // Step 0 with reviews = learning (relearning case)
        insert_progress(&env.user_conn, 3, 0, 6);

        let result =
            fetch_vocab_srs_statuses(&env.user_conn, &["test_pack".to_string()]);

        assert_eq!(
            result[&("test_pack".to_string(), 1, "a".to_string())],
            SrsStatus::Learning,
            "step 3 should be Learning"
        );
        assert_eq!(
            result[&("test_pack".to_string(), 1, "b".to_string())],
            SrsStatus::Graduated,
            "step 4 should be Graduated"
        );
        assert_eq!(
            result[&("test_pack".to_string(), 1, "c".to_string())],
            SrsStatus::Learning,
            "step 0 with reviews should be Learning (relearning)"
        );
    }
}
