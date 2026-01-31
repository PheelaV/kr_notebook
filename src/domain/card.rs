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
  Vocabulary,
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
      "Vocabulary" | "vocabulary" => Some(Self::Vocabulary),
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
      Self::Vocabulary => "vocabulary",
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
  /// True if this is a reverse card (romanization->Korean, "Which letter sounds like...?")
  pub is_reverse: bool,
  /// Pack ID if this card comes from a content pack (None for baseline Hangul)
  pub pack_id: Option<String>,
  /// Lesson number within the pack (None for baseline Hangul)
  pub lesson: Option<u8>,

  // SM-2 fields (kept for backward compatibility and fallback)
  pub ease_factor: f64,
  pub interval_days: i64,
  pub repetitions: i64,
  pub next_review: DateTime<Utc>,

  // Learning steps: 0-3=learning, 4+=graduated to FSRS
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
      is_reverse: false,
      pack_id: None,
      lesson: None,
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
}

#[cfg(test)]
mod tests {
  use super::*;

  // CardType tests

  #[test]
  fn test_card_type_from_str_consonant() {
    assert_eq!(CardType::from_str("consonant"), Some(CardType::Consonant));
  }

  #[test]
  fn test_card_type_from_str_vowel() {
    assert_eq!(CardType::from_str("vowel"), Some(CardType::Vowel));
  }

  #[test]
  fn test_card_type_from_str_tense_consonant() {
    assert_eq!(CardType::from_str("tense_consonant"), Some(CardType::TenseConsonant));
  }

  #[test]
  fn test_card_type_from_str_aspirated_consonant() {
    assert_eq!(CardType::from_str("aspirated_consonant"), Some(CardType::AspiratedConsonant));
  }

  #[test]
  fn test_card_type_from_str_compound_vowel() {
    assert_eq!(CardType::from_str("compound_vowel"), Some(CardType::CompoundVowel));
  }

  #[test]
  fn test_card_type_from_str_syllable() {
    assert_eq!(CardType::from_str("syllable"), Some(CardType::Syllable));
  }

  #[test]
  fn test_card_type_from_str_vocabulary_lowercase() {
    assert_eq!(CardType::from_str("vocabulary"), Some(CardType::Vocabulary));
  }

  #[test]
  fn test_card_type_from_str_vocabulary_capitalized() {
    assert_eq!(CardType::from_str("Vocabulary"), Some(CardType::Vocabulary));
  }

  #[test]
  fn test_card_type_from_str_invalid() {
    assert_eq!(CardType::from_str("invalid"), None);
    assert_eq!(CardType::from_str(""), None);
    assert_eq!(CardType::from_str("CONSONANT"), None);
  }

  #[test]
  fn test_card_type_as_str_roundtrip() {
    let types = vec![
      CardType::Consonant,
      CardType::Vowel,
      CardType::TenseConsonant,
      CardType::AspiratedConsonant,
      CardType::CompoundVowel,
      CardType::Syllable,
      CardType::Vocabulary,
    ];

    for ct in types {
      let s = ct.as_str();
      let parsed = CardType::from_str(s);
      assert_eq!(parsed, Some(ct));
    }
  }

  // FsrsState tests

  #[test]
  fn test_fsrs_state_from_str_new() {
    assert_eq!(FsrsState::from_str("New"), FsrsState::New);
  }

  #[test]
  fn test_fsrs_state_from_str_learning() {
    assert_eq!(FsrsState::from_str("Learning"), FsrsState::Learning);
  }

  #[test]
  fn test_fsrs_state_from_str_review() {
    assert_eq!(FsrsState::from_str("Review"), FsrsState::Review);
  }

  #[test]
  fn test_fsrs_state_from_str_relearning() {
    assert_eq!(FsrsState::from_str("Relearning"), FsrsState::Relearning);
  }

  #[test]
  fn test_fsrs_state_from_str_default() {
    // Unknown strings default to New
    assert_eq!(FsrsState::from_str("unknown"), FsrsState::New);
    assert_eq!(FsrsState::from_str(""), FsrsState::New);
    assert_eq!(FsrsState::from_str("new"), FsrsState::New); // lowercase != "New"
  }

  #[test]
  fn test_fsrs_state_as_str() {
    assert_eq!(FsrsState::New.as_str(), "New");
    assert_eq!(FsrsState::Learning.as_str(), "Learning");
    assert_eq!(FsrsState::Review.as_str(), "Review");
    assert_eq!(FsrsState::Relearning.as_str(), "Relearning");
  }

  #[test]
  fn test_fsrs_state_as_str_roundtrip() {
    let states = vec![
      FsrsState::New,
      FsrsState::Learning,
      FsrsState::Review,
      FsrsState::Relearning,
    ];

    for state in states {
      let s = state.as_str();
      let parsed = FsrsState::from_str(s);
      assert_eq!(parsed, state);
    }
  }

  // Card constructor tests

  #[test]
  fn test_card_new_defaults() {
    let card = Card::new(
      "ㄱ".to_string(),
      "g / k".to_string(),
      Some("First consonant".to_string()),
      CardType::Consonant,
      1,
    );

    assert_eq!(card.id, 0);
    assert_eq!(card.front, "ㄱ");
    assert_eq!(card.main_answer, "g / k");
    assert_eq!(card.description, Some("First consonant".to_string()));
    assert_eq!(card.card_type, CardType::Consonant);
    assert_eq!(card.tier, 1);
    assert!(card.audio_hint.is_none());
    assert!(!card.is_reverse);
    assert!(card.pack_id.is_none());
    assert!(card.lesson.is_none());
    assert!((card.ease_factor - 2.5).abs() < f64::EPSILON);
    assert_eq!(card.interval_days, 0);
    assert_eq!(card.repetitions, 0);
    assert_eq!(card.learning_step, 0);
    assert!(card.fsrs_stability.is_none());
    assert!(card.fsrs_difficulty.is_none());
    assert!(card.fsrs_state.is_none());
    assert_eq!(card.total_reviews, 0);
    assert_eq!(card.correct_reviews, 0);
  }

  #[test]
  fn test_card_new_no_description() {
    let card = Card::new(
      "ㅏ".to_string(),
      "a".to_string(),
      None,
      CardType::Vowel,
      1,
    );

    assert_eq!(card.front, "ㅏ");
    assert!(card.description.is_none());
  }

  #[test]
  fn test_card_type_equality() {
    assert_eq!(CardType::Consonant, CardType::Consonant);
    assert_ne!(CardType::Consonant, CardType::Vowel);
  }

  #[test]
  fn test_fsrs_state_equality() {
    assert_eq!(FsrsState::New, FsrsState::New);
    assert_ne!(FsrsState::New, FsrsState::Learning);
  }
}
