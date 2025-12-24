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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
  pub id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub card_type: CardType,
  pub tier: u8,
  pub audio_hint: Option<String>,

  // SM-2 fields
  pub ease_factor: f64,
  pub interval_days: i64,
  pub repetitions: i64,
  pub next_review: DateTime<Utc>,

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
      total_reviews: 0,
      correct_reviews: 0,
    }
  }
}
