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

  Ok(())
}
