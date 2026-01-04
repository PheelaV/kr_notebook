use chrono::Utc;
use rusqlite::{params, Connection, Result};
use std::path::Path;

/// Current schema version for learning.db
pub const LEARNING_DB_VERSION: i32 = 3;

pub fn run_migrations(conn: &Connection) -> Result<()> {
  run_migrations_with_app_db(conn, None)
}

/// Run migrations with optional app.db path for legacy data migration
pub fn run_migrations_with_app_db(conn: &Connection, app_db_path: Option<&Path>) -> Result<()> {
  // Create db_version table first
  conn.execute_batch(
    r#"
    CREATE TABLE IF NOT EXISTS db_version (
      version INTEGER PRIMARY KEY,
      applied_at TEXT NOT NULL,
      description TEXT
    );
    "#,
  )?;

  // Create tables with COMPLETE schema for new databases
  // Migrations below handle upgrades for existing databases

  // card_progress: User's progress on cards (definitions are in app.db)
  conn.execute_batch(
    r#"
    CREATE TABLE IF NOT EXISTS card_progress (
      card_id INTEGER PRIMARY KEY,  -- FK to app.db card_definitions.id
      ease_factor REAL NOT NULL DEFAULT 2.5,
      interval_days INTEGER NOT NULL DEFAULT 0,
      repetitions INTEGER NOT NULL DEFAULT 0,
      next_review TEXT NOT NULL,
      total_reviews INTEGER NOT NULL DEFAULT 0,
      correct_reviews INTEGER NOT NULL DEFAULT 0,
      learning_step INTEGER NOT NULL DEFAULT 0,
      -- FSRS columns
      fsrs_stability REAL,
      fsrs_difficulty REAL,
      fsrs_state TEXT DEFAULT 'New'
    );
    "#,
  )?;

  // Legacy cards table - kept for migration, will be removed in future version
  conn.execute_batch(
    r#"
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
      -- FSRS columns
      fsrs_stability REAL,
      fsrs_difficulty REAL,
      fsrs_state TEXT DEFAULT 'New',
      -- Content pack columns
      pack_id TEXT  -- NULL = baseline content, otherwise = pack that created this card
    );

    CREATE TABLE IF NOT EXISTS review_logs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      card_id INTEGER NOT NULL,
      quality INTEGER NOT NULL,
      reviewed_at TEXT NOT NULL,
      -- Enhanced logging columns
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

    -- Content pack management
    CREATE TABLE IF NOT EXISTS enabled_packs (
      pack_id TEXT PRIMARY KEY,
      enabled_at TEXT NOT NULL,
      cards_created INTEGER DEFAULT 0,  -- 1 if cards were created from this pack
      config TEXT  -- JSON: user-specific settings for this pack
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

    "#,
  )?;

  // ============================================================
  // MIGRATIONS FOR EXISTING DATABASES
  // These are no-ops for new databases (columns already exist)
  // ============================================================

  // Migration: Add learning_step column (added to cards in early version)
  add_column_if_missing(conn, "cards", "learning_step", "INTEGER NOT NULL DEFAULT 0")?;

  // Migration: Add FSRS columns
  add_column_if_missing(conn, "cards", "fsrs_stability", "REAL")?;
  add_column_if_missing(conn, "cards", "fsrs_difficulty", "REAL")?;
  add_column_if_missing(conn, "cards", "fsrs_state", "TEXT DEFAULT 'New'")?;

  // Migration: Add enhanced review logging columns
  let had_is_correct = column_exists(conn, "review_logs", "is_correct");
  add_column_if_missing(conn, "review_logs", "is_correct", "INTEGER")?;
  add_column_if_missing(conn, "review_logs", "study_mode", "TEXT")?;
  add_column_if_missing(conn, "review_logs", "direction", "TEXT")?;
  add_column_if_missing(conn, "review_logs", "response_time_ms", "INTEGER")?;
  add_column_if_missing(conn, "review_logs", "hints_used", "INTEGER DEFAULT 0")?;

  // Backfill is_correct ONLY if we just added the column to an existing database with data
  if !had_is_correct {
    let has_reviews: bool = conn
      .query_row("SELECT COUNT(*) > 0 FROM review_logs", [], |row| row.get(0))
      .unwrap_or(false);
    if has_reviews {
      conn.execute(
        "UPDATE review_logs SET is_correct = CASE WHEN quality >= 2 THEN 1 ELSE 0 END WHERE is_correct IS NULL",
        [],
      )?;
    }
  }

  // Migration: Add is_reverse column for explicit direction tracking
  let had_is_reverse = column_exists(conn, "cards", "is_reverse");
  add_column_if_missing(conn, "cards", "is_reverse", "INTEGER NOT NULL DEFAULT 0")?;

  // Backfill is_reverse based on front text pattern (migrating from string-based detection)
  if !had_is_reverse {
    let has_cards: bool = conn
      .query_row("SELECT COUNT(*) > 0 FROM cards", [], |row| row.get(0))
      .unwrap_or(false);
    if has_cards {
      conn.execute(
        "UPDATE cards SET is_reverse = 1 WHERE front LIKE 'Which letter sounds like%'",
        [],
      )?;
    }
  }

  // Migration: Add pack_id column for content pack tracking
  add_column_if_missing(conn, "cards", "pack_id", "TEXT")?;

  // Migration: Create enabled_packs table if it doesn't exist
  // (handles databases created before pack system)
  conn.execute_batch(
    r#"
    CREATE TABLE IF NOT EXISTS enabled_packs (
      pack_id TEXT PRIMARY KEY,
      enabled_at TEXT NOT NULL,
      cards_created INTEGER DEFAULT 0,
      config TEXT
    );
    "#,
  )?;

  // ============================================================
  // INDEXES - Created after all migrations so columns exist
  // ============================================================
  conn.execute_batch(
    r#"
    CREATE INDEX IF NOT EXISTS idx_cards_next_review ON cards(next_review);
    CREATE INDEX IF NOT EXISTS idx_cards_tier ON cards(tier);
    CREATE INDEX IF NOT EXISTS idx_cards_pack_id ON cards(pack_id);
    CREATE INDEX IF NOT EXISTS idx_card_progress_next_review ON card_progress(next_review);
    CREATE INDEX IF NOT EXISTS idx_review_logs_card_id ON review_logs(card_id);
    CREATE INDEX IF NOT EXISTS idx_review_logs_reviewed_at ON review_logs(reviewed_at);
    CREATE INDEX IF NOT EXISTS idx_review_logs_study_mode ON review_logs(study_mode);
    CREATE INDEX IF NOT EXISTS idx_confusions_card_id ON confusions(card_id);
    CREATE INDEX IF NOT EXISTS idx_character_stats_type ON character_stats(character_type);
    "#,
  )?;

  // Record schema version
  record_learning_version(conn, LEARNING_DB_VERSION, "Schema with card_progress table")?;

  // ============================================================
  // LEGACY CARDS → CARD_PROGRESS MIGRATION
  // Migrates progress from legacy cards table to card_progress
  // ============================================================
  if let Some(app_db) = app_db_path {
    migrate_legacy_cards_to_progress(conn, app_db)?;
  }

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

/// Record a schema version for learning.db (idempotent - won't duplicate)
fn record_learning_version(conn: &Connection, version: i32, description: &str) -> Result<()> {
  let now = Utc::now().to_rfc3339();
  conn.execute(
    "INSERT OR IGNORE INTO db_version (version, applied_at, description) VALUES (?1, ?2, ?3)",
    params![version, now, description],
  )?;
  Ok(())
}

/// Migrate progress from legacy cards table to card_progress
/// This matches legacy cards to card_definitions by content and copies SRS state
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

  // ATTACH app.db to access card_definitions
  let app_db_str = app_db_path
    .to_str()
    .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid app.db path".into()))?;

  conn.execute(&format!("ATTACH DATABASE '{}' AS app", app_db_str), [])?;

  // Migrate: match legacy cards to card_definitions by content, copy progress
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
      c.front = cd.front AND
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
