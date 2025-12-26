use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &Connection) -> Result<()> {
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
      learning_step INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS review_logs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      card_id INTEGER NOT NULL,
      quality INTEGER NOT NULL,
      reviewed_at TEXT NOT NULL,
      FOREIGN KEY (card_id) REFERENCES cards(id)
    );

    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );

    -- Default settings (INSERT OR IGNORE is safe for existing databases)
    INSERT OR IGNORE INTO settings (key, value) VALUES ('max_unlocked_tier', '1');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('dark_mode', 'false');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('tts_enabled', 'true');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('tts_model', 'mms');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('all_tiers_unlocked', 'false');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('enabled_tiers', '1,2,3,4');

    CREATE INDEX IF NOT EXISTS idx_cards_next_review ON cards(next_review);
    CREATE INDEX IF NOT EXISTS idx_cards_tier ON cards(tier);
    CREATE INDEX IF NOT EXISTS idx_review_logs_card_id ON review_logs(card_id);
    "#,
  )?;

  // Migration: Add learning_step column if it doesn't exist
  let has_learning_step: bool = conn
    .prepare("SELECT learning_step FROM cards LIMIT 1")
    .is_ok();
  if !has_learning_step {
    conn.execute("ALTER TABLE cards ADD COLUMN learning_step INTEGER NOT NULL DEFAULT 0", [])?;
  }

  // Migration: Add FSRS columns if they don't exist (nullable for backward compatibility)
  let has_fsrs_stability: bool = conn
    .prepare("SELECT fsrs_stability FROM cards LIMIT 1")
    .is_ok();
  if !has_fsrs_stability {
    conn.execute("ALTER TABLE cards ADD COLUMN fsrs_stability REAL", [])?;
    conn.execute("ALTER TABLE cards ADD COLUMN fsrs_difficulty REAL", [])?;
    conn.execute("ALTER TABLE cards ADD COLUMN fsrs_state TEXT DEFAULT 'New'", [])?;
  }

  // Migration: Add confusions table for tracking wrong answers
  conn.execute_batch(
    r#"
    CREATE TABLE IF NOT EXISTS confusions (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      card_id INTEGER NOT NULL,
      wrong_answer TEXT NOT NULL,
      count INTEGER NOT NULL DEFAULT 1,
      last_confused_at TEXT NOT NULL,
      FOREIGN KEY (card_id) REFERENCES cards(id)
    );

    CREATE INDEX IF NOT EXISTS idx_confusions_card_id ON confusions(card_id);
    "#,
  )?;

  // Migration: Add FSRS settings
  conn.execute_batch(
    r#"
    INSERT OR IGNORE INTO settings (key, value) VALUES ('desired_retention', '0.9');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('use_fsrs', 'true');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('use_interleaving', 'true');
    "#,
  )?;

  // Migration: Add enhanced review logging columns
  let has_is_correct: bool = conn
    .prepare("SELECT is_correct FROM review_logs LIMIT 1")
    .is_ok();
  if !has_is_correct {
    conn.execute("ALTER TABLE review_logs ADD COLUMN is_correct INTEGER", [])?;
    conn.execute("ALTER TABLE review_logs ADD COLUMN study_mode TEXT", [])?;
    conn.execute("ALTER TABLE review_logs ADD COLUMN direction TEXT", [])?;
    conn.execute("ALTER TABLE review_logs ADD COLUMN response_time_ms INTEGER", [])?;
    conn.execute("ALTER TABLE review_logs ADD COLUMN hints_used INTEGER DEFAULT 0", [])?;

    // Backfill is_correct from quality (quality >= 2 means correct per ReviewQuality)
    conn.execute(
      "UPDATE review_logs SET is_correct = CASE WHEN quality >= 2 THEN 1 ELSE 0 END",
      [],
    )?;
  }

  // Migration: Add character_stats table for aggregated stats
  conn.execute_batch(
    r#"
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

    CREATE INDEX IF NOT EXISTS idx_review_logs_reviewed_at ON review_logs(reviewed_at);
    CREATE INDEX IF NOT EXISTS idx_review_logs_study_mode ON review_logs(study_mode);
    CREATE INDEX IF NOT EXISTS idx_character_stats_type ON character_stats(character_type);
    "#,
  )?;

  // Migration: Fix repetitions for FSRS users (bug: FSRS wasn't updating repetitions)
  // Backfill from correct_reviews where it's higher than current repetitions
  conn.execute(
    "UPDATE cards SET repetitions = correct_reviews WHERE correct_reviews > repetitions",
    [],
  )?;

  Ok(())
}
