//! Card pack loading and enabling - reads card definitions from pack JSON files.
//!
//! Card packs contain card definitions that can be enabled by users. When enabled:
//! - Cards are inserted into shared `card_definitions` table (app.db)
//! - User's `enabled_packs` table (learning.db) tracks pack enablement
//! - All users can see cards once any user has enabled the pack

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::CardType;

/// Card definition from a pack's cards.json file.
#[derive(Debug, Clone, Deserialize)]
pub struct CardDefinition {
    pub front: String,
    pub main_answer: String,
    #[serde(default)]
    pub description: Option<String>,
    pub card_type: CardType,
    pub tier: u8,
    #[serde(default)]
    pub is_reverse: bool,
    #[serde(default)]
    pub audio_hint: Option<String>,
    /// Lesson number for lesson-based progression (None for baseline/non-lesson content)
    #[serde(default)]
    pub lesson: Option<u8>,
}

/// Container for cards in a pack's cards.json file.
#[derive(Debug, Deserialize)]
pub struct CardPackData {
    pub cards: Vec<CardDefinition>,
}

/// Load cards from a pack's cards.json file.
pub fn load_cards_from_pack(pack_dir: &Path, cards_file: &str) -> Result<Vec<CardDefinition>, CardLoadError> {
    let cards_path = pack_dir.join(cards_file);

    if !cards_path.exists() {
        return Err(CardLoadError::FileNotFound(cards_path.display().to_string()));
    }

    let content = fs::read_to_string(&cards_path)
        .map_err(|e| CardLoadError::IoError(cards_path.display().to_string(), e.to_string()))?;

    let data: CardPackData = serde_json::from_str(&content)
        .map_err(|e| CardLoadError::ParseError(cards_path.display().to_string(), e.to_string()))?;

    Ok(data.cards)
}

/// Load cards from the baseline pack.
///
/// Looks for the baseline pack at `data/content/packs/baseline/cards.json`.
/// Returns None if the pack doesn't exist (fallback to hardcoded data).
pub fn load_baseline_cards() -> Option<Vec<CardDefinition>> {
    let baseline_dir = PathBuf::from(crate::paths::shared_packs_dir()).join("baseline");

    match load_cards_from_pack(&baseline_dir, "cards.json") {
        Ok(cards) => {
            tracing::debug!("Loaded {} cards from baseline pack", cards.len());
            Some(cards)
        }
        Err(e) => {
            tracing::debug!("Baseline pack not available, using hardcoded data: {}", e);
            None
        }
    }
}

/// Card loading errors.
#[derive(Debug)]
pub enum CardLoadError {
    FileNotFound(String),
    IoError(String, String),
    ParseError(String, String),
}

impl std::fmt::Display for CardLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardLoadError::FileNotFound(path) => write!(f, "Card file not found: {}", path),
            CardLoadError::IoError(path, err) => write!(f, "IO error reading {}: {}", path, err),
            CardLoadError::ParseError(path, err) => write!(f, "Parse error in {}: {}", path, err),
        }
    }
}

impl CardLoadError {
    /// Returns a user-facing error message without exposing filesystem paths.
    pub fn user_message(&self) -> &'static str {
        match self {
            CardLoadError::FileNotFound(_) => "Card file not found",
            CardLoadError::IoError(_, _) => "Failed to read card file",
            CardLoadError::ParseError(_, _) => "Failed to parse card file",
        }
    }
}

impl std::error::Error for CardLoadError {}

// ==================== Pack Enable/Disable Operations ====================

/// Result of enabling a card pack
#[derive(Debug)]
pub struct EnablePackResult {
    /// Number of new cards inserted
    pub cards_inserted: usize,
    /// Number of cards skipped (already existed)
    pub cards_skipped: usize,
}

/// Enable a card pack for a user.
///
/// This function:
/// 1. Registers the pack in content_packs table (required for FK constraint)
/// 2. Loads cards from the pack's cards.json
/// 3. Inserts new card_definitions into app.db (skipping duplicates)
/// 4. Records pack as enabled in user's learning.db
///
/// # Arguments
/// * `app_conn` - Connection to app.db (for card_definitions and content_packs)
/// * `user_conn` - Connection to user's learning.db (for enabled_packs)
/// * `pack_id` - The pack identifier
/// * `pack_name` - Human-readable pack name
/// * `pack_version` - Pack version string
/// * `pack_description` - Optional pack description
/// * `pack_scope` - Pack scope (global or user)
/// * `pack_dir` - Path to the pack directory
/// * `cards_file` - Name of the cards JSON file (from pack manifest)
///
/// # Returns
/// EnablePackResult with counts of inserted/skipped cards
pub fn enable_card_pack(
    app_conn: &Connection,
    user_conn: &Connection,
    pack_id: &str,
    pack_name: &str,
    pack_version: &str,
    pack_description: Option<&str>,
    pack_scope: &super::packs::PackScope,
    pack_dir: &Path,
    cards_file: &str,
) -> Result<EnablePackResult, CardLoadError> {
    // First, register the pack in content_packs (required for FK constraint)
    let now = Utc::now().to_rfc3339();
    let source_path = pack_dir.to_string_lossy();
    app_conn
        .execute(
            r#"INSERT OR IGNORE INTO content_packs
               (id, name, version, description, pack_type, scope, source_path, installed_at)
               VALUES (?1, ?2, ?3, ?4, 'cards', ?5, ?6, ?7)"#,
            params![pack_id, pack_name, pack_version, pack_description, pack_scope, source_path, now],
        )
        .map_err(|e| CardLoadError::IoError("content_packs".to_string(), e.to_string()))?;

    // Global packs are admin-only by default (no auto-public permission)
    // Admins can explicitly make packs public via the settings UI

    // Load cards from pack
    let cards = load_cards_from_pack(pack_dir, cards_file)?;

    let mut inserted = 0;
    let mut skipped = 0;

    // Insert cards into shared card_definitions (skip if already exists)
    for card in &cards {
        let exists: bool = app_conn
            .query_row(
                r#"SELECT EXISTS(
                    SELECT 1 FROM card_definitions
                    WHERE front = ?1 AND main_answer = ?2 AND card_type = ?3
                      AND tier = ?4 AND is_reverse = ?5
                )"#,
                params![
                    card.front,
                    card.main_answer,
                    card.card_type.as_str(),
                    card.tier,
                    card.is_reverse,
                ],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if exists {
            #[cfg(feature = "profiling")]
            crate::profile_log!(crate::profiling::EventType::PackCardSkipped {
                pack_id: pack_id.to_string(),
                front: card.front.clone(),
                main_answer: card.main_answer.clone(),
                card_type: card.card_type.as_str().to_string(),
                reason: "duplicate".to_string(),
            });
            skipped += 1;
            continue;
        }

        match app_conn.execute(
            r#"INSERT INTO card_definitions
               (front, main_answer, description, card_type, tier, audio_hint, is_reverse, pack_id, lesson)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                card.front,
                card.main_answer,
                card.description,
                card.card_type.as_str(),
                card.tier,
                card.audio_hint,
                card.is_reverse,
                pack_id,
                card.lesson,
            ],
        ) {
            Ok(_) => inserted += 1,
            Err(e) => {
                tracing::warn!("Failed to insert card '{}': {}", card.front, e);
                skipped += 1;
            }
        }
    }

    // Record in user's enabled_packs
    let now = Utc::now().to_rfc3339();
    user_conn
        .execute(
            r#"INSERT OR REPLACE INTO enabled_packs (pack_id, enabled_at, cards_created, config)
               VALUES (?1, ?2, 1, NULL)"#,
            params![pack_id, now],
        )
        .map_err(|e| CardLoadError::IoError("enabled_packs".to_string(), e.to_string()))?;

    tracing::info!(
        "Enabled card pack '{}': {} cards inserted, {} skipped",
        pack_id,
        inserted,
        skipped
    );

    Ok(EnablePackResult {
        cards_inserted: inserted,
        cards_skipped: skipped,
    })
}

/// Disable a pack for a user.
///
/// This removes the pack from the user's enabled_packs but does NOT delete
/// card_definitions (other users may have them enabled).
pub fn disable_pack(user_conn: &Connection, pack_id: &str) -> Result<bool, CardLoadError> {
    let deleted = user_conn
        .execute("DELETE FROM enabled_packs WHERE pack_id = ?1", params![pack_id])
        .map_err(|e| CardLoadError::IoError("enabled_packs".to_string(), e.to_string()))?;

    Ok(deleted > 0)
}

/// Check if a pack is enabled for a user.
pub fn is_pack_enabled(user_conn: &Connection, pack_id: &str) -> bool {
    user_conn
        .query_row(
            "SELECT 1 FROM enabled_packs WHERE pack_id = ?1",
            params![pack_id],
            |_| Ok(()),
        )
        .is_ok()
}

/// Get list of enabled pack IDs for a user.
pub fn list_enabled_packs(user_conn: &Connection) -> Vec<String> {
    let mut stmt = match user_conn.prepare("SELECT pack_id FROM enabled_packs") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map([], |row| row.get(0))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Enabled pack info (for UI display)
#[derive(Debug, Clone)]
pub struct EnabledPackInfo {
    pub pack_id: String,
    pub enabled_at: String,
    pub cards_created: bool,
}

/// Get detailed info about enabled packs for a user.
pub fn get_enabled_packs_info(user_conn: &Connection) -> Vec<EnabledPackInfo> {
    let mut stmt = match user_conn
        .prepare("SELECT pack_id, enabled_at, cards_created FROM enabled_packs")
    {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map([], |row| {
        Ok(EnabledPackInfo {
            pack_id: row.get(0)?,
            enabled_at: row.get(1)?,
            cards_created: row.get::<_, i64>(2)? != 0,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::super::packs::PackScope;
    use super::*;
    use crate::testing::TestEnv;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_cards_from_pack() {
        let temp = TempDir::new().unwrap();

        let cards_json = r#"{
            "cards": [
                {
                    "front": "ㄱ",
                    "main_answer": "g / k",
                    "description": "Test description",
                    "card_type": "Consonant",
                    "tier": 1,
                    "is_reverse": false
                },
                {
                    "front": "g / k",
                    "main_answer": "ㄱ",
                    "card_type": "Consonant",
                    "tier": 1,
                    "is_reverse": true
                }
            ]
        }"#;

        fs::write(temp.path().join("cards.json"), cards_json).unwrap();

        let cards = load_cards_from_pack(temp.path(), "cards.json").unwrap();
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].front, "ㄱ");
        assert_eq!(cards[0].card_type, CardType::Consonant);
        assert!(!cards[0].is_reverse);
        assert!(cards[1].is_reverse);
    }

    #[test]
    fn test_missing_file() {
        let temp = TempDir::new().unwrap();
        let result = load_cards_from_pack(temp.path(), "cards.json");
        assert!(matches!(result, Err(CardLoadError::FileNotFound(_))));
    }

    fn create_test_pack(dir: &Path, cards_json: &str) {
        fs::write(dir.join("cards.json"), cards_json).unwrap();
    }

    #[test]
    fn test_enable_card_pack() {
        let env = TestEnv::new().unwrap();
        let pack_dir = env.path().join("test-pack");
        fs::create_dir(&pack_dir).unwrap();

        let cards_json = r#"{
            "cards": [
                {"front": "A", "main_answer": "a", "card_type": "Vowel", "tier": 1, "is_reverse": false},
                {"front": "a", "main_answer": "A", "card_type": "Vowel", "tier": 1, "is_reverse": true}
            ]
        }"#;
        create_test_pack(&pack_dir, cards_json);

        let result = enable_card_pack(
            &env.app_conn,
            &env.user_conn,
            "test-pack",
            "Test Pack",
            "1.0.0",
            None,
            &PackScope::Global,
            &pack_dir,
            "cards.json",
        )
        .unwrap();

        assert_eq!(result.cards_inserted, 2);
        assert_eq!(result.cards_skipped, 0);

        // Check cards were inserted
        let count: i64 = env
            .app_conn
            .query_row("SELECT COUNT(*) FROM card_definitions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // Check pack is recorded as enabled
        assert!(is_pack_enabled(&env.user_conn, "test-pack"));
    }

    #[test]
    fn test_enable_card_pack_skips_duplicates() {
        let env = TestEnv::new().unwrap();
        let pack_dir = env.path().join("test-pack");
        fs::create_dir(&pack_dir).unwrap();

        let cards_json = r#"{
            "cards": [
                {"front": "A", "main_answer": "a", "card_type": "Vowel", "tier": 1, "is_reverse": false}
            ]
        }"#;
        create_test_pack(&pack_dir, cards_json);

        // First enable
        let result1 = enable_card_pack(
            &env.app_conn,
            &env.user_conn,
            "test-pack",
            "Test Pack",
            "1.0.0",
            None,
            &PackScope::Global,
            &pack_dir,
            "cards.json",
        )
        .unwrap();
        assert_eq!(result1.cards_inserted, 1);

        // Second enable should skip
        let result2 = enable_card_pack(
            &env.app_conn,
            &env.user_conn,
            "test-pack",
            "Test Pack",
            "1.0.0",
            None,
            &PackScope::Global,
            &pack_dir,
            "cards.json",
        )
        .unwrap();
        assert_eq!(result2.cards_inserted, 0);
        assert_eq!(result2.cards_skipped, 1);

        // Should still only have 1 card
        let count: i64 = env
            .app_conn
            .query_row("SELECT COUNT(*) FROM card_definitions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_disable_pack() {
        let env = TestEnv::new().unwrap();
        let pack_dir = env.path().join("test-pack");
        fs::create_dir(&pack_dir).unwrap();

        let cards_json = r#"{"cards": [{"front": "A", "main_answer": "a", "card_type": "Vowel", "tier": 1, "is_reverse": false}]}"#;
        create_test_pack(&pack_dir, cards_json);

        // Enable then disable
        enable_card_pack(
            &env.app_conn,
            &env.user_conn,
            "test-pack",
            "Test Pack",
            "1.0.0",
            None,
            &PackScope::Global,
            &pack_dir,
            "cards.json",
        )
        .unwrap();
        assert!(is_pack_enabled(&env.user_conn, "test-pack"));

        let deleted = disable_pack(&env.user_conn, "test-pack").unwrap();
        assert!(deleted);
        assert!(!is_pack_enabled(&env.user_conn, "test-pack"));

        // Cards should still exist in app.db
        let count: i64 = env
            .app_conn
            .query_row("SELECT COUNT(*) FROM card_definitions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_list_enabled_packs() {
        let env = TestEnv::new().unwrap();

        // Create and enable two packs
        for i in 1..=2 {
            let pack_dir = env.path().join(format!("pack{}", i));
            fs::create_dir(&pack_dir).unwrap();
            let cards_json = format!(
                r#"{{"cards": [{{"front": "{}", "main_answer": "x", "card_type": "Vowel", "tier": 1, "is_reverse": false}}]}}"#,
                i
            );
            create_test_pack(&pack_dir, &cards_json);
            enable_card_pack(
                &env.app_conn,
                &env.user_conn,
                &format!("pack{}", i),
                &format!("Pack {}", i),
                "1.0.0",
                None,
                &PackScope::Global,
                &pack_dir,
                "cards.json",
            )
            .unwrap();
        }

        let enabled = list_enabled_packs(&env.user_conn);
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&"pack1".to_string()));
        assert!(enabled.contains(&"pack2".to_string()));
    }

    #[test]
    fn test_get_enabled_packs_info() {
        let env = TestEnv::new().unwrap();
        let pack_dir = env.path().join("test-pack");
        fs::create_dir(&pack_dir).unwrap();

        let cards_json = r#"{"cards": [{"front": "A", "main_answer": "a", "card_type": "Vowel", "tier": 1, "is_reverse": false}]}"#;
        create_test_pack(&pack_dir, cards_json);

        enable_card_pack(
            &env.app_conn,
            &env.user_conn,
            "test-pack",
            "Test Pack",
            "1.0.0",
            None,
            &PackScope::Global,
            &pack_dir,
            "cards.json",
        )
        .unwrap();

        let info = get_enabled_packs_info(&env.user_conn);
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].pack_id, "test-pack");
        assert!(info[0].cards_created);
        assert!(!info[0].enabled_at.is_empty());
    }

    #[test]
    fn test_load_cards_with_lesson() {
        let temp = TempDir::new().unwrap();

        let cards_json = r#"{
            "cards": [
                {
                    "front": "사람",
                    "main_answer": "person",
                    "card_type": "Vocabulary",
                    "tier": 5,
                    "is_reverse": false,
                    "lesson": 1
                },
                {
                    "front": "person",
                    "main_answer": "사람",
                    "card_type": "Vocabulary",
                    "tier": 5,
                    "is_reverse": true,
                    "lesson": 1
                }
            ]
        }"#;

        fs::write(temp.path().join("cards.json"), cards_json).unwrap();

        let cards = load_cards_from_pack(temp.path(), "cards.json").unwrap();
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].lesson, Some(1));
        assert_eq!(cards[1].lesson, Some(1));
    }

    #[test]
    fn test_enable_card_pack_with_lessons() {
        let env = TestEnv::new().unwrap();
        let pack_dir = env.path().join("vocab-pack");
        fs::create_dir(&pack_dir).unwrap();

        let cards_json = r#"{
            "cards": [
                {"front": "사람", "main_answer": "person", "card_type": "Vocabulary", "tier": 5, "is_reverse": false, "lesson": 1},
                {"front": "물", "main_answer": "water", "card_type": "Vocabulary", "tier": 5, "is_reverse": false, "lesson": 2}
            ]
        }"#;
        create_test_pack(&pack_dir, cards_json);

        let result = enable_card_pack(
            &env.app_conn,
            &env.user_conn,
            "vocab-pack",
            "Vocab Pack",
            "1.0.0",
            None,
            &PackScope::Global,
            &pack_dir,
            "cards.json",
        )
        .unwrap();
        assert_eq!(result.cards_inserted, 2);

        // Verify lessons are stored
        let lesson1_count: i64 = env
            .app_conn
            .query_row(
                "SELECT COUNT(*) FROM card_definitions WHERE lesson = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let lesson2_count: i64 = env
            .app_conn
            .query_row(
                "SELECT COUNT(*) FROM card_definitions WHERE lesson = 2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lesson1_count, 1);
        assert_eq!(lesson2_count, 1);
    }

    #[test]
    fn test_load_test_exercises_pack_cards() {
        use std::path::PathBuf;

        // Load the test_exercises_pack fixture used by E2E tests
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/integration/fixtures/test_exercises_pack");

        // Skip if fixture doesn't exist
        if !fixture_path.exists() {
            return;
        }

        // This tests that the cards.json format is correct (must be {"cards": [...]})
        let result = load_cards_from_pack(&fixture_path, "cards.json");
        assert!(
            result.is_ok(),
            "Failed to load test_exercises_pack cards.json: {:?}",
            result.err()
        );

        // Empty cards array is valid for exercises-only packs
        let cards = result.unwrap();
        assert!(
            cards.is_empty(),
            "test_exercises_pack should have empty cards array"
        );
    }
}
