use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Input method indicates how the user provided their answer
/// This determines whether strict or fuzzy matching is used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InputMethod {
  /// User selected from a closed set of options - use strict matching
  MultipleChoice,
  /// User typed the answer - use fuzzy matching to allow typos
  #[default]
  TextInput,
}

impl InputMethod {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::MultipleChoice => "multiple_choice",
      Self::TextInput => "text_input",
    }
  }

  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "multiple_choice" => Some(Self::MultipleChoice),
      "text_input" => Some(Self::TextInput),
      _ => None,
    }
  }

  /// Returns true if this input method should use strict (exact) matching
  pub fn is_strict(&self) -> bool {
    matches!(self, Self::MultipleChoice)
  }
}

/// Study mode indicates which UI/interaction mode was used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StudyMode {
  Classic,           // Flip card, rate 1-4
  Interactive,       // Type/select answer
  Listening,         // Audio recognition
  PracticeFlip,      // Practice mode with flip
  PracticeInteractive, // Practice mode with typing/selection
}

impl StudyMode {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::Classic => "classic",
      Self::Interactive => "interactive",
      Self::Listening => "listening",
      Self::PracticeFlip => "practice_flip",
      Self::PracticeInteractive => "practice_interactive",
    }
  }

  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "classic" => Some(Self::Classic),
      "interactive" => Some(Self::Interactive),
      "listening" => Some(Self::Listening),
      "practice_flip" => Some(Self::PracticeFlip),
      "practice_interactive" => Some(Self::PracticeInteractive),
      _ => None,
    }
  }
}

/// Direction indicates which direction the card was tested
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewDirection {
  KrToRom,   // Korean character → romanization (e.g., ㄱ → "g/k")
  RomToKr,   // Romanization → Korean character (e.g., "g/k" → ㄱ)
  AudioToKr, // Audio → Korean character (listening mode)
}

impl ReviewDirection {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::KrToRom => "kr_to_rom",
      Self::RomToKr => "rom_to_kr",
      Self::AudioToKr => "audio_to_kr",
    }
  }

  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "kr_to_rom" => Some(Self::KrToRom),
      "rom_to_kr" => Some(Self::RomToKr),
      "audio_to_kr" => Some(Self::AudioToKr),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewLog {
  pub id: i64,
  pub card_id: i64,
  pub quality: u8,
  pub reviewed_at: DateTime<Utc>,
  // Enhanced fields (optional for backward compatibility)
  pub is_correct: Option<bool>,
  pub study_mode: Option<StudyMode>,
  pub direction: Option<ReviewDirection>,
  pub response_time_ms: Option<i64>,
  pub hints_used: Option<i32>,
}

impl ReviewLog {
  pub fn new(card_id: i64, quality: u8) -> Self {
    Self {
      id: 0,
      card_id,
      quality,
      reviewed_at: Utc::now(),
      is_correct: None,
      study_mode: None,
      direction: None,
      response_time_ms: None,
      hints_used: None,
    }
  }

  /// Create a new enhanced review log with all fields
  pub fn new_enhanced(
    card_id: i64,
    quality: u8,
    is_correct: bool,
    study_mode: StudyMode,
    direction: ReviewDirection,
    response_time_ms: Option<i64>,
    hints_used: i32,
  ) -> Self {
    Self {
      id: 0,
      card_id,
      quality,
      reviewed_at: Utc::now(),
      is_correct: Some(is_correct),
      study_mode: Some(study_mode),
      direction: Some(direction),
      response_time_ms,
      hints_used: Some(hints_used),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewQuality {
  Again = 0,
  Hard = 2,
  Good = 4,
  Easy = 5,
}

impl ReviewQuality {
  pub fn from_u8(value: u8) -> Option<Self> {
    match value {
      0 => Some(Self::Again),
      2 => Some(Self::Hard),
      4 => Some(Self::Good),
      5 => Some(Self::Easy),
      _ => None,
    }
  }

  pub fn is_correct(&self) -> bool {
    matches!(self, Self::Hard | Self::Good | Self::Easy)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // InputMethod tests

  #[test]
  fn test_input_method_strict() {
    assert!(InputMethod::MultipleChoice.is_strict());
    assert!(!InputMethod::TextInput.is_strict());
  }

  #[test]
  fn test_input_method_default() {
    // Default should be TextInput for backwards compatibility
    assert_eq!(InputMethod::default(), InputMethod::TextInput);
  }

  #[test]
  fn test_input_method_serde() {
    // Test that serde rename_all works correctly
    let mc: InputMethod = serde_json::from_str("\"multiple_choice\"").unwrap();
    assert_eq!(mc, InputMethod::MultipleChoice);

    let ti: InputMethod = serde_json::from_str("\"text_input\"").unwrap();
    assert_eq!(ti, InputMethod::TextInput);

    assert_eq!(serde_json::to_string(&InputMethod::MultipleChoice).unwrap(), "\"multiple_choice\"");
    assert_eq!(serde_json::to_string(&InputMethod::TextInput).unwrap(), "\"text_input\"");
  }

  #[test]
  fn test_input_method_from_str() {
    assert_eq!(InputMethod::from_str("multiple_choice"), Some(InputMethod::MultipleChoice));
    assert_eq!(InputMethod::from_str("text_input"), Some(InputMethod::TextInput));
    assert_eq!(InputMethod::from_str("invalid"), None);
  }

  #[test]
  fn test_input_method_as_str() {
    assert_eq!(InputMethod::MultipleChoice.as_str(), "multiple_choice");
    assert_eq!(InputMethod::TextInput.as_str(), "text_input");
  }

  // StudyMode tests

  #[test]
  fn test_study_mode_from_str_classic() {
    assert_eq!(StudyMode::from_str("classic"), Some(StudyMode::Classic));
  }

  #[test]
  fn test_study_mode_from_str_interactive() {
    assert_eq!(StudyMode::from_str("interactive"), Some(StudyMode::Interactive));
  }

  #[test]
  fn test_study_mode_from_str_listening() {
    assert_eq!(StudyMode::from_str("listening"), Some(StudyMode::Listening));
  }

  #[test]
  fn test_study_mode_from_str_practice_flip() {
    assert_eq!(StudyMode::from_str("practice_flip"), Some(StudyMode::PracticeFlip));
  }

  #[test]
  fn test_study_mode_from_str_practice_interactive() {
    assert_eq!(StudyMode::from_str("practice_interactive"), Some(StudyMode::PracticeInteractive));
  }

  #[test]
  fn test_study_mode_from_str_invalid() {
    assert_eq!(StudyMode::from_str("invalid"), None);
    assert_eq!(StudyMode::from_str(""), None);
    assert_eq!(StudyMode::from_str("Classic"), None); // case sensitive
  }

  #[test]
  fn test_study_mode_as_str() {
    assert_eq!(StudyMode::Classic.as_str(), "classic");
    assert_eq!(StudyMode::Interactive.as_str(), "interactive");
    assert_eq!(StudyMode::Listening.as_str(), "listening");
    assert_eq!(StudyMode::PracticeFlip.as_str(), "practice_flip");
    assert_eq!(StudyMode::PracticeInteractive.as_str(), "practice_interactive");
  }

  #[test]
  fn test_study_mode_roundtrip() {
    let modes = vec![
      StudyMode::Classic,
      StudyMode::Interactive,
      StudyMode::Listening,
      StudyMode::PracticeFlip,
      StudyMode::PracticeInteractive,
    ];

    for mode in modes {
      let s = mode.as_str();
      let parsed = StudyMode::from_str(s);
      assert_eq!(parsed, Some(mode));
    }
  }

  // ReviewDirection tests

  #[test]
  fn test_review_direction_from_str_kr_to_rom() {
    assert_eq!(ReviewDirection::from_str("kr_to_rom"), Some(ReviewDirection::KrToRom));
  }

  #[test]
  fn test_review_direction_from_str_rom_to_kr() {
    assert_eq!(ReviewDirection::from_str("rom_to_kr"), Some(ReviewDirection::RomToKr));
  }

  #[test]
  fn test_review_direction_from_str_audio_to_kr() {
    assert_eq!(ReviewDirection::from_str("audio_to_kr"), Some(ReviewDirection::AudioToKr));
  }

  #[test]
  fn test_review_direction_from_str_invalid() {
    assert_eq!(ReviewDirection::from_str("invalid"), None);
    assert_eq!(ReviewDirection::from_str(""), None);
  }

  #[test]
  fn test_review_direction_as_str() {
    assert_eq!(ReviewDirection::KrToRom.as_str(), "kr_to_rom");
    assert_eq!(ReviewDirection::RomToKr.as_str(), "rom_to_kr");
    assert_eq!(ReviewDirection::AudioToKr.as_str(), "audio_to_kr");
  }

  #[test]
  fn test_review_direction_roundtrip() {
    let directions = vec![
      ReviewDirection::KrToRom,
      ReviewDirection::RomToKr,
      ReviewDirection::AudioToKr,
    ];

    for dir in directions {
      let s = dir.as_str();
      let parsed = ReviewDirection::from_str(s);
      assert_eq!(parsed, Some(dir));
    }
  }

  // ReviewQuality tests

  #[test]
  fn test_review_quality_from_u8_again() {
    assert_eq!(ReviewQuality::from_u8(0), Some(ReviewQuality::Again));
  }

  #[test]
  fn test_review_quality_from_u8_hard() {
    assert_eq!(ReviewQuality::from_u8(2), Some(ReviewQuality::Hard));
  }

  #[test]
  fn test_review_quality_from_u8_good() {
    assert_eq!(ReviewQuality::from_u8(4), Some(ReviewQuality::Good));
  }

  #[test]
  fn test_review_quality_from_u8_easy() {
    assert_eq!(ReviewQuality::from_u8(5), Some(ReviewQuality::Easy));
  }

  #[test]
  fn test_review_quality_from_u8_invalid() {
    assert_eq!(ReviewQuality::from_u8(1), None);
    assert_eq!(ReviewQuality::from_u8(3), None);
    assert_eq!(ReviewQuality::from_u8(6), None);
    assert_eq!(ReviewQuality::from_u8(255), None);
  }

  #[test]
  fn test_review_quality_is_correct() {
    assert!(!ReviewQuality::Again.is_correct());
    assert!(ReviewQuality::Hard.is_correct());
    assert!(ReviewQuality::Good.is_correct());
    assert!(ReviewQuality::Easy.is_correct());
  }

  #[test]
  fn test_review_quality_values() {
    assert_eq!(ReviewQuality::Again as u8, 0);
    assert_eq!(ReviewQuality::Hard as u8, 2);
    assert_eq!(ReviewQuality::Good as u8, 4);
    assert_eq!(ReviewQuality::Easy as u8, 5);
  }

  // ReviewLog tests

  #[test]
  fn test_review_log_new() {
    let log = ReviewLog::new(42, 4);
    assert_eq!(log.id, 0);
    assert_eq!(log.card_id, 42);
    assert_eq!(log.quality, 4);
    assert!(log.is_correct.is_none());
    assert!(log.study_mode.is_none());
    assert!(log.direction.is_none());
    assert!(log.response_time_ms.is_none());
    assert!(log.hints_used.is_none());
  }

  #[test]
  fn test_review_log_new_enhanced() {
    let log = ReviewLog::new_enhanced(
      42,
      4,
      true,
      StudyMode::Interactive,
      ReviewDirection::KrToRom,
      Some(1500),
      0,
    );

    assert_eq!(log.id, 0);
    assert_eq!(log.card_id, 42);
    assert_eq!(log.quality, 4);
    assert_eq!(log.is_correct, Some(true));
    assert_eq!(log.study_mode, Some(StudyMode::Interactive));
    assert_eq!(log.direction, Some(ReviewDirection::KrToRom));
    assert_eq!(log.response_time_ms, Some(1500));
    assert_eq!(log.hints_used, Some(0));
  }

  #[test]
  fn test_review_log_enhanced_with_hints() {
    let log = ReviewLog::new_enhanced(
      1,
      2,
      false,
      StudyMode::Classic,
      ReviewDirection::RomToKr,
      None,
      2,
    );

    assert_eq!(log.is_correct, Some(false));
    assert_eq!(log.hints_used, Some(2));
    assert!(log.response_time_ms.is_none());
  }
}
