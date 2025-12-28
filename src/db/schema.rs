use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &Connection) -> Result<()> {
  // Create tables with COMPLETE schema for new databases
  // Migrations below handle upgrades for existing databases
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

    -- Indexes
    CREATE INDEX IF NOT EXISTS idx_cards_next_review ON cards(next_review);
    CREATE INDEX IF NOT EXISTS idx_cards_tier ON cards(tier);
    CREATE INDEX IF NOT EXISTS idx_review_logs_card_id ON review_logs(card_id);
    CREATE INDEX IF NOT EXISTS idx_review_logs_reviewed_at ON review_logs(reviewed_at);
    CREATE INDEX IF NOT EXISTS idx_review_logs_study_mode ON review_logs(study_mode);
    CREATE INDEX IF NOT EXISTS idx_confusions_card_id ON confusions(card_id);
    CREATE INDEX IF NOT EXISTS idx_character_stats_type ON character_stats(character_type);
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

  // OBSOLETE MIGRATIONS - These were one-time fixes applied to production.
  // Keeping them active interferes with test scenarios where we intentionally
  // reset card states. Commented out 2024-12-28.
  //
  // // Migration: Fix repetitions for FSRS users (bug: FSRS wasn't updating repetitions)
  // conn.execute(
  //   "UPDATE cards SET repetitions = correct_reviews WHERE correct_reviews > repetitions",
  //   [],
  // )?;
  //
  // // Migration: Graduate existing FSRS cards to step 4 (learning steps integration)
  // conn.execute(
  //   "UPDATE cards SET learning_step = 4 WHERE fsrs_state IS NOT NULL AND fsrs_state != 'New' AND learning_step < 4",
  //   [],
  // )?;

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
