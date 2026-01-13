//! Learning database schema and migrations.
//!
//! ## Migration System
//!
//! This module uses a version-gated migration system. Each migration:
//! 1. Checks if the current schema version is less than the target version
//! 2. Runs the migration SQL
//! 3. Records the new version in `db_version` table
//!
//! Migrations only run once - the version check ensures idempotency.
//! Data migrations (like legacy cards → card_progress) have additional
//! guards to prevent re-running.

use chrono::Utc;
use rusqlite::{params, Connection, Result};
use std::path::Path;

/// Current schema version for learning.db
/// Increment this when adding a new migration
pub const LEARNING_DB_VERSION: i32 = 7;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    run_migrations_with_app_db(conn, None)
}

/// Run migrations with optional app.db path for legacy data migration
pub fn run_migrations_with_app_db(conn: &Connection, app_db_path: Option<&Path>) -> Result<()> {
    // Bootstrap: ensure db_version table exists (needed to check version)
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS db_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL,
            description TEXT
        );
        "#,
    )?;

    let current_version = get_schema_version(conn)?;
    tracing::trace!("learning.db schema version: {}", current_version);

    // Run migrations in order, each checks version before executing
    if current_version < 1 {
        migrate_v0_to_v1(conn)?;
    }
    if current_version < 2 {
        migrate_v1_to_v2(conn)?;
    }
    if current_version < 3 {
        migrate_v2_to_v3(conn)?;
    }
    if current_version < 4 {
        migrate_v3_to_v4(conn)?;
    }
    if current_version < 5 {
        migrate_v4_to_v5(conn)?;
    }
    if current_version < 6 {
        migrate_v5_to_v6(conn)?;
    }
    if current_version < 7 {
        migrate_v6_to_v7(conn)?;
    }

    // Legacy cards → card_progress migration (has its own idempotency guards)
    if let Some(app_db) = app_db_path {
        migrate_legacy_cards_to_progress(conn, app_db)?;
    }

    Ok(())
}

// ============================================================
// VERSION-GATED MIGRATIONS
// Each migration runs exactly once based on version check
// ============================================================

/// v0→v1: Create base tables
fn migrate_v0_to_v1(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v0→v1: Create base tables");

    conn.execute_batch(
        r#"
        -- card_progress: User's progress on cards (definitions are in app.db)
        CREATE TABLE IF NOT EXISTS card_progress (
            card_id INTEGER PRIMARY KEY,
            ease_factor REAL NOT NULL DEFAULT 2.5,
            interval_days INTEGER NOT NULL DEFAULT 0,
            repetitions INTEGER NOT NULL DEFAULT 0,
            next_review TEXT NOT NULL,
            total_reviews INTEGER NOT NULL DEFAULT 0,
            correct_reviews INTEGER NOT NULL DEFAULT 0,
            learning_step INTEGER NOT NULL DEFAULT 0,
            fsrs_stability REAL,
            fsrs_difficulty REAL,
            fsrs_state TEXT DEFAULT 'New'
        );

        -- Legacy cards table (kept for migration from old databases)
        CREATE TABLE IF NOT EXISTS cards (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            front TEXT NOT NULL,
            main_answer TEXT NOT NULL,
            description TEXT,
            card_type TEXT NOT NULL,
            tier INTEGER NOT NULL,
            audio_hint TEXT,
            is_reverse INTEGER NOT NULL DEFAULT 0,
            ease_factor REAL NOT NULL DEFAULT 2.5,
            interval_days INTEGER NOT NULL DEFAULT 0,
            repetitions INTEGER NOT NULL DEFAULT 0,
            next_review TEXT NOT NULL,
            total_reviews INTEGER NOT NULL DEFAULT 0,
            correct_reviews INTEGER NOT NULL DEFAULT 0,
            learning_step INTEGER NOT NULL DEFAULT 0,
            fsrs_stability REAL,
            fsrs_difficulty REAL,
            fsrs_state TEXT DEFAULT 'New',
            pack_id TEXT
        );

        CREATE TABLE IF NOT EXISTS review_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            quality INTEGER NOT NULL,
            reviewed_at TEXT NOT NULL,
            is_correct INTEGER,
            study_mode TEXT,
            direction TEXT,
            response_time_ms INTEGER,
            hints_used INTEGER DEFAULT 0,
            FOREIGN KEY (card_id) REFERENCES cards(id)
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS confusions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            wrong_answer TEXT NOT NULL,
            count INTEGER NOT NULL DEFAULT 1,
            last_confused_at TEXT NOT NULL,
            FOREIGN KEY (card_id) REFERENCES cards(id)
        );

        CREATE TABLE IF NOT EXISTS character_stats (
            character TEXT PRIMARY KEY,
            character_type TEXT NOT NULL,
            total_attempts INTEGER DEFAULT 0,
            total_correct INTEGER DEFAULT 0,
            attempts_7d INTEGER DEFAULT 0,
            correct_7d INTEGER DEFAULT 0,
            attempts_1d INTEGER DEFAULT 0,
            correct_1d INTEGER DEFAULT 0,
            last_attempt_at TEXT
        );

        CREATE TABLE IF NOT EXISTS tier_graduation_backups (
            tier INTEGER PRIMARY KEY,
            backup_data TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS enabled_packs (
            pack_id TEXT PRIMARY KEY,
            enabled_at TEXT NOT NULL,
            cards_created INTEGER DEFAULT 0,
            config TEXT
        );

        CREATE TABLE IF NOT EXISTS pack_lesson_progress (
            pack_id TEXT NOT NULL,
            lesson INTEGER NOT NULL,
            unlocked INTEGER NOT NULL DEFAULT 0,
            unlocked_at TEXT,
            PRIMARY KEY (pack_id, lesson)
        );

        -- Default settings
        INSERT OR IGNORE INTO settings (key, value) VALUES ('max_unlocked_tier', '1');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('dark_mode', 'false');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('tts_enabled', 'true');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('tts_model', 'mms');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('all_tiers_unlocked', 'false');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('enabled_tiers', '1,2,3,4');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('desired_retention', '0.9');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('use_fsrs', 'true');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('use_interleaving', 'true');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('accelerated_packs', '');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('study_filter_mode', 'all');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('study_filter_pack', '');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('study_filter_lessons', '');

        -- Indexes (only for columns that exist in base tables)
        -- Note: idx_cards_pack_id created in v3→v4, idx_review_logs_study_mode in v2→v3
        CREATE INDEX IF NOT EXISTS idx_cards_next_review ON cards(next_review);
        CREATE INDEX IF NOT EXISTS idx_cards_tier ON cards(tier);
        CREATE INDEX IF NOT EXISTS idx_card_progress_next_review ON card_progress(next_review);
        CREATE INDEX IF NOT EXISTS idx_review_logs_card_id ON review_logs(card_id);
        CREATE INDEX IF NOT EXISTS idx_review_logs_reviewed_at ON review_logs(reviewed_at);
        CREATE INDEX IF NOT EXISTS idx_confusions_card_id ON confusions(card_id);
        CREATE INDEX IF NOT EXISTS idx_character_stats_type ON character_stats(character_type);
        "#,
    )?;

    record_version(conn, 1, "Create base tables")?;
    Ok(())
}

/// v1→v2: Add learning_step and FSRS columns to legacy cards table
/// (For databases created before these columns existed)
fn migrate_v1_to_v2(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v1→v2: Add FSRS columns to legacy cards");

    add_column_if_missing(conn, "cards", "learning_step", "INTEGER NOT NULL DEFAULT 0")?;
    add_column_if_missing(conn, "cards", "fsrs_stability", "REAL")?;
    add_column_if_missing(conn, "cards", "fsrs_difficulty", "REAL")?;
    add_column_if_missing(conn, "cards", "fsrs_state", "TEXT DEFAULT 'New'")?;

    record_version(conn, 2, "Add FSRS columns to legacy cards")?;
    Ok(())
}

/// v2→v3: Add enhanced review logging and is_reverse column
fn migrate_v2_to_v3(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v2→v3: Add enhanced review logging");

    // Track if column existed before (for backfill decision)
    let had_is_correct = column_exists(conn, "review_logs", "is_correct");
    let had_is_reverse = column_exists(conn, "cards", "is_reverse");

    // Add review logging columns
    add_column_if_missing(conn, "review_logs", "is_correct", "INTEGER")?;
    add_column_if_missing(conn, "review_logs", "study_mode", "TEXT")?;
    add_column_if_missing(conn, "review_logs", "direction", "TEXT")?;
    add_column_if_missing(conn, "review_logs", "response_time_ms", "INTEGER")?;
    add_column_if_missing(conn, "review_logs", "hints_used", "INTEGER DEFAULT 0")?;

    // Add is_reverse column
    add_column_if_missing(conn, "cards", "is_reverse", "INTEGER NOT NULL DEFAULT 0")?;

    // Backfill is_correct from quality (only for existing data)
    if !had_is_correct {
        let has_reviews: bool = conn
            .query_row("SELECT COUNT(*) > 0 FROM review_logs", [], |row| row.get(0))
            .unwrap_or(false);
        if has_reviews {
            conn.execute(
                "UPDATE review_logs SET is_correct = CASE WHEN quality >= 2 THEN 1 ELSE 0 END WHERE is_correct IS NULL",
                [],
            )?;
            tracing::info!("Backfilled is_correct in review_logs");
        }
    }

    // Backfill is_reverse from front text pattern
    if !had_is_reverse {
        let has_cards: bool = conn
            .query_row("SELECT COUNT(*) > 0 FROM cards", [], |row| row.get(0))
            .unwrap_or(false);
        if has_cards {
            conn.execute(
                "UPDATE cards SET is_reverse = 1 WHERE front LIKE 'Which letter sounds like%'",
                [],
            )?;
            tracing::info!("Backfilled is_reverse in cards");
        }
    }

    // Create index on study_mode now that column exists
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_review_logs_study_mode ON review_logs(study_mode);",
    )?;

    record_version(conn, 3, "Add enhanced review logging and is_reverse")?;
    Ok(())
}

/// v3→v4: Add content pack support to legacy cards
fn migrate_v3_to_v4(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v3→v4: Add content pack support");

    add_column_if_missing(conn, "cards", "pack_id", "TEXT")?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS enabled_packs (
            pack_id TEXT PRIMARY KEY,
            enabled_at TEXT NOT NULL,
            cards_created INTEGER DEFAULT 0,
            config TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_cards_pack_id ON cards(pack_id);
        "#,
    )?;

    record_version(conn, 4, "Add content pack support")?;
    Ok(())
}

/// v4→v5: Add pack lesson progression
fn migrate_v4_to_v5(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v4→v5: Add pack lesson progression");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS pack_lesson_progress (
            pack_id TEXT NOT NULL,
            lesson INTEGER NOT NULL,
            unlocked INTEGER NOT NULL DEFAULT 0,
            unlocked_at TEXT,
            PRIMARY KEY (pack_id, lesson)
        );

        INSERT OR IGNORE INTO settings (key, value) VALUES ('accelerated_packs', '');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('study_filter_mode', 'all');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('study_filter_pack', '');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('study_filter_lessons', '');
        "#,
    )?;

    record_version(conn, 5, "Add pack lesson progression")?;
    Ok(())
}

/// v5→v6: Remove foreign key constraints from review_logs and confusions
/// These tables referenced cards(id) but pack cards are in app.card_definitions
fn migrate_v5_to_v6(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v5→v6: Remove legacy foreign key constraints");

    // Recreate review_logs without FK constraint
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS review_logs_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            quality INTEGER NOT NULL,
            reviewed_at TEXT NOT NULL,
            is_correct INTEGER,
            study_mode TEXT,
            direction TEXT,
            response_time_ms INTEGER,
            hints_used INTEGER DEFAULT 0
        );
        INSERT INTO review_logs_new SELECT * FROM review_logs;
        DROP TABLE review_logs;
        ALTER TABLE review_logs_new RENAME TO review_logs;

        CREATE INDEX IF NOT EXISTS idx_review_logs_card_id ON review_logs(card_id);
        CREATE INDEX IF NOT EXISTS idx_review_logs_reviewed_at ON review_logs(reviewed_at);
        CREATE INDEX IF NOT EXISTS idx_review_logs_study_mode ON review_logs(study_mode);
        "#,
    )?;

    // Recreate confusions without FK constraint
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS confusions_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            wrong_answer TEXT NOT NULL,
            count INTEGER NOT NULL DEFAULT 1,
            last_confused_at TEXT NOT NULL
        );
        INSERT INTO confusions_new SELECT * FROM confusions;
        DROP TABLE confusions;
        ALTER TABLE confusions_new RENAME TO confusions;
        "#,
    )?;

    record_version(conn, 6, "Remove legacy FK constraints from review_logs and confusions")?;
    Ok(())
}

/// v6→v7: Add offline study mode settings and tracking
fn migrate_v6_to_v7(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v6→v7: Add offline study mode support");

    conn.execute_batch(
        r#"
        -- Offline mode settings
        INSERT OR IGNORE INTO settings (key, value) VALUES ('offline_mode_enabled', 'false');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('offline_session_duration', '30');

        -- Track offline sessions for sync
        CREATE TABLE IF NOT EXISTS offline_sessions (
            session_id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            card_count INTEGER NOT NULL,
            filter_mode TEXT,
            synced INTEGER NOT NULL DEFAULT 0,
            synced_at TEXT
        );
        "#,
    )?;

    record_version(conn, 7, "Add offline study mode support")?;
    Ok(())
}

// ============================================================
// MIGRATION HELPERS
// ============================================================

/// Get current schema version (0 if no versions recorded)
fn get_schema_version(conn: &Connection) -> Result<i32> {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM db_version",
        [],
        |row| row.get(0),
    )
}

/// Record a schema version after successful migration
fn record_version(conn: &Connection, version: i32, description: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO db_version (version, applied_at, description) VALUES (?1, ?2, ?3)",
        params![version, now, description],
    )?;
    tracing::info!("Recorded learning.db schema version {} - {}", version, description);
    Ok(())
}

/// Check if a column exists in a table
fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn
        .prepare(&format!("SELECT {} FROM {} LIMIT 1", column, table))
        .is_ok()
}

/// Add a column if it doesn't already exist
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, column_def: &str) -> Result<()> {
    if !column_exists(conn, table, column) {
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, column_def),
            [],
        )?;
    }
    Ok(())
}

// ============================================================
// DATA MIGRATION: Legacy cards → card_progress
// ============================================================

/// Migrate progress from legacy cards table to card_progress
/// This matches legacy cards to card_definitions by content and copies SRS state
///
/// NOTE: Match on (main_answer, card_type, tier, is_reverse) NOT front, because
/// legacy reverse cards used "Which letter sounds like 'X'?" format while new
/// card_definitions use just "X" as the front. main_answer is always the Korean
/// character which is stable across formats.
fn migrate_legacy_cards_to_progress(conn: &Connection, app_db_path: &Path) -> Result<()> {
    // Check if we have legacy cards with progress to migrate
    let legacy_cards_with_progress: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE total_reviews > 0",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if legacy_cards_with_progress == 0 {
        return Ok(()); // No legacy progress to migrate
    }

    // Check if card_progress already has data (don't re-migrate)
    let progress_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM card_progress", [], |row| row.get(0))
        .unwrap_or(0);

    if progress_count > 0 {
        return Ok(()); // Already migrated
    }

    tracing::info!(
        "Migrating {} legacy cards to card_progress",
        legacy_cards_with_progress
    );

    // ATTACH app.db to access card_definitions
    let app_db_str = app_db_path
        .to_str()
        .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid app.db path".into()))?;

    conn.execute(&format!("ATTACH DATABASE '{}' AS app", app_db_str), [])?;

    // Migrate: match legacy cards to card_definitions by (main_answer, card_type, tier, is_reverse)
    // NOT by front - legacy reverse cards had different front format
    let migrated = conn.execute(
        r#"
        INSERT INTO card_progress (
            card_id, ease_factor, interval_days, repetitions, next_review,
            total_reviews, correct_reviews, learning_step,
            fsrs_stability, fsrs_difficulty, fsrs_state
        )
        SELECT
            cd.id,
            c.ease_factor,
            c.interval_days,
            c.repetitions,
            c.next_review,
            c.total_reviews,
            c.correct_reviews,
            c.learning_step,
            c.fsrs_stability,
            c.fsrs_difficulty,
            c.fsrs_state
        FROM cards c
        INNER JOIN app.card_definitions cd ON
            c.main_answer = cd.main_answer AND
            c.card_type = cd.card_type AND
            c.tier = cd.tier AND
            c.is_reverse = cd.is_reverse
        WHERE c.total_reviews > 0
        "#,
        [],
    )?;

    conn.execute("DETACH DATABASE app", [])?;

    tracing::info!(
        "Migrated {} legacy cards to card_progress",
        migrated
    );

    Ok(())
}
