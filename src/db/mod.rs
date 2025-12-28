pub mod cards;
pub mod reviews;
pub mod schema;
pub mod stats;
pub mod tiers;

use rusqlite::{Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::domain::{Card, CardType};

// Re-export all public items from submodules
pub use cards::*;
pub use reviews::*;
pub use schema::run_migrations;
pub use stats::*;
pub use tiers::*;

pub type DbPool = Arc<Mutex<Connection>>;

/// Extension trait for logging errors before discarding them
pub trait LogOnError<T> {
    /// Log the error at warn level and return None
    fn log_warn(self, context: &str) -> Option<T>;
    /// Log the error at warn level and return the default
    fn log_warn_default(self, context: &str) -> T
    where
        T: Default;
}

impl<T, E: std::fmt::Display> LogOnError<T> for std::result::Result<T, E> {
    fn log_warn(self, context: &str) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("{}: {}", context, e);
                None
            }
        }
    }

    fn log_warn_default(self, context: &str) -> T
    where
        T: Default,
    {
        match self {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("{}: {}", context, e);
                T::default()
            }
        }
    }
}

/// Error returned when database lock cannot be acquired
#[derive(Debug)]
pub struct DbLockError;

impl std::fmt::Display for DbLockError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Database unavailable")
  }
}

impl std::error::Error for DbLockError {}

/// Try to acquire the database lock, returning an error if poisoned
pub fn try_lock(pool: &DbPool) -> std::result::Result<MutexGuard<'_, Connection>, DbLockError> {
  pool.lock().map_err(|_: PoisonError<_>| {
    eprintln!("ERROR: Database mutex poisoned - a thread panicked while holding the lock");
    DbLockError
  })
}

pub fn init_db(path: &Path) -> Result<DbPool> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }

  // Create backup before migrations if database exists
  if path.exists() {
    let backup_path = path.with_extension("db.backup");
    if let Err(e) = std::fs::copy(path, &backup_path) {
      eprintln!("Warning: Could not create database backup: {}", e);
    }
  }

  let conn = Connection::open(path)?;
  run_migrations(&conn)?;
  Ok(Arc::new(Mutex::new(conn)))
}

/// Create a backup of the database using VACUUM INTO
#[allow(dead_code)]
pub fn backup_database(conn: &Connection, backup_path: &Path) -> Result<()> {
  conn.execute("VACUUM INTO ?1", [backup_path.to_str().unwrap()])?;
  Ok(())
}

pub fn seed_hangul_cards(conn: &Connection) -> Result<()> {
  let count: i64 = conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
  if count > 0 {
    return Ok(());
  }

  let cards = get_hangul_seed_data();
  for card in cards {
    insert_card(conn, &card)?;
  }
  Ok(())
}

// Helper to create a card with main answer and description
fn card(front: &str, main: &str, desc: Option<&str>, card_type: CardType, tier: u8) -> Card {
  Card::new(
    front.to_string(),
    main.to_string(),
    desc.map(|s| s.to_string()),
    card_type,
    tier,
  )
}

fn get_hangul_seed_data() -> Vec<Card> {
  let mut cards = Vec::new();

  // Tier 1: Basic Consonants (letter -> sound)
  let tier1_consonants = [
    ("ㄱ", "g / k", "Like 'g' in 'go' at the start, 'k' in 'kite' at the end"),
    ("ㄴ", "n", "Like 'n' in 'no'"),
    ("ㄷ", "d / t", "Like 'd' in 'do' at the start, 't' in 'top' at the end"),
    ("ㄹ", "r / l", "Like 'r' in 'run' at the start, 'l' in 'ball' at the end"),
    ("ㅁ", "m", "Like 'm' in 'mom'"),
    ("ㅂ", "b / p", "Like 'b' in 'boy' at the start, 'p' in 'put' at the end"),
    ("ㅅ", "s", "Like 's' in 'sun'"),
    ("ㅈ", "j", "Like 'j' in 'just'"),
    ("ㅎ", "h", "Like 'h' in 'hi'"),
  ];

  for (front, main, desc) in tier1_consonants {
    cards.push(card(front, main, Some(desc), CardType::Consonant, 1));
    // Reverse card: sound -> letter
    cards.push(card(
      &format!("Which letter sounds like '{}'?", main),
      front,
      None,
      CardType::Consonant,
      1,
    ));
  }

  // Tier 1: Basic Vowels (letter -> sound)
  let tier1_vowels = [
    ("ㅏ", "a", "Like 'a' in 'father'"),
    ("ㅓ", "eo", "Like 'u' in 'fun' or 'uh'"),
    ("ㅗ", "o", "Like 'o' in 'go'"),
    ("ㅜ", "u", "Like 'oo' in 'moon'"),
    ("ㅡ", "eu", "Like 'u' in 'put', with unrounded lips"),
    ("ㅣ", "i", "Like 'ee' in 'see'"),
  ];

  for (front, main, desc) in tier1_vowels {
    cards.push(card(front, main, Some(desc), CardType::Vowel, 1));
    cards.push(card(
      &format!("Which letter sounds like '{}'?", main),
      front,
      None,
      CardType::Vowel,
      1,
    ));
  }

  // Tier 2: ㅇ and Y-vowels
  cards.push(card(
    "ㅇ (initial)",
    "Silent",
    Some("No sound when at the start of a syllable"),
    CardType::Consonant,
    2,
  ));
  cards.push(card(
    "ㅇ (final)",
    "ng",
    Some("Like 'ng' in 'sing' when at the end"),
    CardType::Consonant,
    2,
  ));

  let tier2_vowels = [
    ("ㅑ", "ya", "Like 'ya' in 'yacht'"),
    ("ㅕ", "yeo", "Like 'yu' in 'yuck'"),
    ("ㅛ", "yo", "Like 'yo' in 'yoga'"),
    ("ㅠ", "yu", "Like 'you'"),
    ("ㅐ", "ae", "Like 'a' in 'can' or 'e' in 'bed'"),
    ("ㅔ", "e", "Like 'e' in 'bed' (sounds same as ㅐ in modern Korean)"),
  ];

  for (front, main, desc) in tier2_vowels {
    cards.push(card(front, main, Some(desc), CardType::Vowel, 2));
    cards.push(card(
      &format!("Which letter sounds like '{}'?", main),
      front,
      None,
      CardType::Vowel,
      2,
    ));
  }

  // Tier 3: Aspirated Consonants
  let tier3_aspirated = [
    ("ㅋ", "k (aspirated)", "Stronger 'k' with a puff of breath, like 'k' in 'kick'"),
    ("ㅍ", "p (aspirated)", "Stronger 'p' with a puff of breath, like 'p' in 'pop'"),
    ("ㅌ", "t (aspirated)", "Stronger 't' with a puff of breath, like 't' in 'top'"),
    ("ㅊ", "ch (aspirated)", "Stronger 'ch' with a puff of breath, like 'ch' in 'church'"),
  ];

  for (front, main, desc) in tier3_aspirated {
    cards.push(card(front, main, Some(desc), CardType::AspiratedConsonant, 3));
    cards.push(card(
      &format!("Which letter sounds like '{}'?", main),
      front,
      None,
      CardType::AspiratedConsonant,
      3,
    ));
  }

  // Tier 3: Tense Consonants
  let tier3_tense = [
    ("ㄲ", "kk (tense)", "Tense 'k' with no breath, like 'ck' in 'sticky'"),
    ("ㅃ", "pp (tense)", "Tense 'p' with no breath, like 'pp' in 'happy'"),
    ("ㄸ", "tt (tense)", "Tense 't' with no breath, like 'tt' in 'butter'"),
    ("ㅆ", "ss (tense)", "Tense 's', like 'ss' in 'hiss'"),
    ("ㅉ", "jj (tense)", "Tense 'j', like 'dg' in 'edge'"),
  ];

  for (front, main, desc) in tier3_tense {
    cards.push(card(front, main, Some(desc), CardType::TenseConsonant, 3));
    cards.push(card(
      &format!("Which letter sounds like '{}'?", main),
      front,
      None,
      CardType::TenseConsonant,
      3,
    ));
  }

  // Tier 4: Compound Vowels
  let tier4_compound = [
    ("ㅘ", "wa", "Like 'wa' in 'want'"),
    ("ㅝ", "wo", "Like 'wo' in 'won'"),
    ("ㅟ", "wi", "Like 'wee'"),
    ("ㅚ", "oe", "Like 'we' in 'wet'"),
    ("ㅢ", "ui", "Like 'oo-ee' said quickly"),
    ("ㅙ", "wae", "Like 'wa' in 'wax'"),
    ("ㅞ", "we", "Like 'we' in 'wet'"),
    ("ㅒ", "yae", "Like 'ya' in 'yam'"),
    ("ㅖ", "ye", "Like 'ye' in 'yes'"),
  ];

  for (front, main, desc) in tier4_compound {
    cards.push(card(front, main, Some(desc), CardType::CompoundVowel, 4));
    cards.push(card(
      &format!("Which letter sounds like '{}'?", main),
      front,
      None,
      CardType::CompoundVowel,
      4,
    ));
  }

  cards
}
