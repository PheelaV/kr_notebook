use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewLog {
  pub id: i64,
  pub card_id: i64,
  pub quality: u8,
  pub reviewed_at: DateTime<Utc>,
}

impl ReviewLog {
  pub fn new(card_id: i64, quality: u8) -> Self {
    Self {
      id: 0,
      card_id,
      quality,
      reviewed_at: Utc::now(),
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
