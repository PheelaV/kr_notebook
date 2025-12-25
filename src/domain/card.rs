use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardType {
  Consonant,
  Vowel,
  TenseConsonant,
  AspiratedConsonant,
  CompoundVowel,
  Syllable,
}

impl CardType {
  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "consonant" => Some(Self::Consonant),
      "vowel" => Some(Self::Vowel),
      "tense_consonant" => Some(Self::TenseConsonant),
      "aspirated_consonant" => Some(Self::AspiratedConsonant),
      "compound_vowel" => Some(Self::CompoundVowel),
      "syllable" => Some(Self::Syllable),
      _ => None,
    }
  }

  pub fn as_str(&self) -> &'static str {
    match self {
      Self::Consonant => "consonant",
      Self::Vowel => "vowel",
      Self::TenseConsonant => "tense_consonant",
      Self::AspiratedConsonant => "aspirated_consonant",
      Self::CompoundVowel => "compound_vowel",
      Self::Syllable => "syllable",
    }
  }
}

/// FSRS memory state for a card
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FsrsState {
  New,
  Learning,
  Review,
  Relearning,
}

impl FsrsState {
  pub fn from_str(s: &str) -> Self {
    match s {
      "Learning" => Self::Learning,
      "Review" => Self::Review,
      "Relearning" => Self::Relearning,
      _ => Self::New,
    }
  }

  pub fn as_str(&self) -> &'static str {
    match self {
      Self::New => "New",
      Self::Learning => "Learning",
      Self::Review => "Review",
      Self::Relearning => "Relearning",
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
  pub id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub card_type: CardType,
  pub tier: u8,
  pub audio_hint: Option<String>,

  // SM-2 fields (kept for backward compatibility and fallback)
  pub ease_factor: f64,
  pub interval_days: i64,
  pub repetitions: i64,
  pub next_review: DateTime<Utc>,

  // Learning steps (Anki-style: 0=new, 1-4=learning, 5+=graduated to SM-2)
  pub learning_step: i64,

  // FSRS fields (optional - None means card hasn't been migrated yet)
  pub fsrs_stability: Option<f64>,
  pub fsrs_difficulty: Option<f64>,
  pub fsrs_state: Option<FsrsState>,

  // Stats
  pub total_reviews: i64,
  pub correct_reviews: i64,
}

impl Card {
  pub fn new(
    front: String,
    main_answer: String,
    description: Option<String>,
    card_type: CardType,
    tier: u8,
  ) -> Self {
    Self {
      id: 0,
      front,
      main_answer,
      description,
      card_type,
      tier,
      audio_hint: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: Utc::now(),
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
      total_reviews: 0,
      correct_reviews: 0,
    }
  }

  /// Check if this card is a reverse card (sound->letter question format)
  #[allow(dead_code)]
  pub fn is_reverse_card(&self) -> bool {
    self.front.starts_with("Which letter")
  }
}
