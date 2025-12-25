use chrono::{DateTime, Duration, Utc};
use fsrs::{MemoryState, FSRS, DEFAULT_PARAMETERS};

use crate::domain::{Card, FsrsState};

/// Result from FSRS scheduling calculation
pub struct FsrsResult {
  pub next_review: DateTime<Utc>,
  pub stability: f64,
  pub difficulty: f64,
  pub state: FsrsState,
}

/// Determine FSRS state based on card history
fn determine_fsrs_state(card: &Card, is_correct: bool) -> FsrsState {
  match (card.fsrs_state.as_ref(), is_correct) {
    // New card getting first review
    (None, _) | (Some(FsrsState::New), _) => {
      if is_correct {
        FsrsState::Learning
      } else {
        FsrsState::New
      }
    }
    // Learning card
    (Some(FsrsState::Learning), true) => FsrsState::Review,
    (Some(FsrsState::Learning), false) => FsrsState::Learning,
    // Review card
    (Some(FsrsState::Review), true) => FsrsState::Review,
    (Some(FsrsState::Review), false) => FsrsState::Relearning,
    // Relearning card
    (Some(FsrsState::Relearning), true) => FsrsState::Review,
    (Some(FsrsState::Relearning), false) => FsrsState::Relearning,
  }
}

/// Calculate next review using FSRS algorithm
/// Quality: 0=Again, 2=Hard, 4=Good, 5=Easy
pub fn calculate_fsrs_review(card: &Card, quality: u8, desired_retention: f64) -> FsrsResult {
  let fsrs = FSRS::new(Some(&DEFAULT_PARAMETERS)).expect("Failed to initialize FSRS");
  let now = Utc::now();

  // Get current memory state from card (if exists)
  let current_memory = match (card.fsrs_stability, card.fsrs_difficulty) {
    (Some(stability), Some(difficulty)) => Some(MemoryState {
      stability: stability as f32,
      difficulty: difficulty as f32,
    }),
    _ => None,
  };

  // Calculate elapsed days since last review (as u32)
  let elapsed_days = (now - card.next_review).num_days().max(0) as u32;

  // Get next states for all possible ratings
  let next_states = fsrs
    .next_states(current_memory, desired_retention as f32, elapsed_days)
    .expect("Failed to calculate FSRS next states");

  // Select the state based on our quality rating
  // FSRS uses: 1=Again, 2=Hard, 3=Good, 4=Easy
  let scheduled = match quality {
    0 => &next_states.again, // Again (failed)
    2 => &next_states.hard,  // Hard
    4 => &next_states.good,  // Good
    5 => &next_states.easy,  // Easy
    _ => &next_states.good,  // Default to Good
  };

  // Calculate next review time
  let interval_days = scheduled.interval.round() as i64;
  let next_review = now + Duration::days(interval_days.max(1));

  // Determine state transition
  let is_correct = quality >= 2;
  let new_state = determine_fsrs_state(card, is_correct);

  FsrsResult {
    next_review,
    stability: scheduled.memory.stability as f64,
    difficulty: scheduled.memory.difficulty as f64,
    state: new_state,
  }
}

/// Migrate a card from SM-2 to FSRS
/// Uses the card's current SM-2 data to estimate initial FSRS state
pub fn migrate_from_sm2(card: &Card, desired_retention: f64) -> Option<(f64, f64, FsrsState)> {
  // Only migrate graduated cards (those with valid SM-2 data)
  if card.interval_days <= 0 || card.ease_factor <= 0.0 {
    return None;
  }

  let fsrs = FSRS::new(Some(&DEFAULT_PARAMETERS)).ok()?;

  // Use FSRS's built-in SM-2 migration
  let estimated_retention = (desired_retention as f32).min(0.99).max(0.7);

  let memory_state = fsrs
    .memory_state_from_sm2(
      card.ease_factor as f32,
      card.interval_days as f32,
      estimated_retention,
    )
    .ok()?;

  Some((
    memory_state.stability as f64,
    memory_state.difficulty as f64,
    FsrsState::Review, // Graduated SM-2 cards are in Review state
  ))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::CardType;

  fn make_test_card() -> Card {
    Card {
      id: 1,
      front: "ㄱ".to_string(),
      main_answer: "g / k".to_string(),
      description: None,
      card_type: CardType::Consonant,
      tier: 1,
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

  #[test]
  fn test_new_card_fsrs_review() {
    let card = make_test_card();
    let result = calculate_fsrs_review(&card, 4, 0.9);

    // New card should get some stability and difficulty
    assert!(result.stability > 0.0);
    assert!(result.difficulty > 0.0);
    assert!(result.next_review > Utc::now());
  }

  #[test]
  fn test_failed_review_shorter_interval() {
    let card = make_test_card();

    let good_result = calculate_fsrs_review(&card, 4, 0.9);
    let fail_result = calculate_fsrs_review(&card, 0, 0.9);

    // Failed review should have shorter interval than good review
    assert!(fail_result.next_review <= good_result.next_review);
  }

  #[test]
  fn test_sm2_migration() {
    let mut card = make_test_card();
    card.ease_factor = 2.5;
    card.interval_days = 10;

    let result = migrate_from_sm2(&card, 0.9);
    assert!(result.is_some());

    let (stability, difficulty, state) = result.unwrap();
    assert!(stability > 0.0);
    assert!(difficulty > 0.0);
    assert_eq!(state, FsrsState::Review);
  }

  #[test]
  fn test_state_transitions() {
    // New card -> Learning on success
    let mut card = make_test_card();
    assert_eq!(determine_fsrs_state(&card, true), FsrsState::Learning);

    // Learning -> Review on success
    card.fsrs_state = Some(FsrsState::Learning);
    assert_eq!(determine_fsrs_state(&card, true), FsrsState::Review);

    // Review -> Relearning on failure
    card.fsrs_state = Some(FsrsState::Review);
    assert_eq!(determine_fsrs_state(&card, false), FsrsState::Relearning);

    // Relearning -> Review on success
    card.fsrs_state = Some(FsrsState::Relearning);
    assert_eq!(determine_fsrs_state(&card, true), FsrsState::Review);
  }
}
