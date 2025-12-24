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
      correct_reviews INTEGER NOT NULL DEFAULT 0
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

    -- Default: only tier 1 unlocked
    INSERT OR IGNORE INTO settings (key, value) VALUES ('max_unlocked_tier', '1');

    CREATE INDEX IF NOT EXISTS idx_cards_next_review ON cards(next_review);
    CREATE INDEX IF NOT EXISTS idx_cards_tier ON cards(tier);
    CREATE INDEX IF NOT EXISTS idx_review_logs_card_id ON review_logs(card_id);
    "#,
  )?;
  Ok(())
}
