use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
