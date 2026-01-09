use chrono::{DateTime, Duration, Utc};
use fsrs::{MemoryState, FSRS, DEFAULT_PARAMETERS};

use crate::config;
use crate::domain::{Card, FsrsState};

const GRADUATING_STEP: i64 = 4; // Step at which card graduates to FSRS

/// Result from FSRS scheduling calculation
pub struct FsrsResult {
  pub next_review: DateTime<Utc>,
  pub stability: f64,
  pub difficulty: f64,
  pub state: FsrsState,
  pub learning_step: i64,
  pub repetitions: i64,
}

/// Determine FSRS state based on card history (used for state transition testing)
#[allow(dead_code)]
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

/// Calculate next review using hybrid learning steps + FSRS algorithm
/// Quality: 0=Again, 2=Hard, 4=Good, 5=Easy
///
/// For cards in learning phase (learning_step < 4):
///   - Uses Anki-style learning steps
///   - Normal mode: 1min, 10min, 1hr, 4hr (~5 hours to graduate)
///   - Focus mode: 1min, 5min, 15min, 30min (~50 minutes to graduate)
///   - Failure resets to step 0
///   - Success advances to next step
///   - After step 3, graduates to FSRS (step 4)
///
/// For graduated cards (learning_step >= 4):
///   - Uses FSRS algorithm for optimal long-term spacing
///   - Failure returns card to learning phase (step 0)
pub fn calculate_fsrs_review(
  card: &Card,
  quality: u8,
  desired_retention: f64,
  focus_mode: bool,
) -> FsrsResult {
  // SrsCalculation profiling moved to handler level (requires username)
  let now = Utc::now();
  let is_correct = quality >= 2;
  let learning_steps = config::get_learning_steps(focus_mode);

  // In learning phase (step 0-3): use learning steps
  if card.learning_step < GRADUATING_STEP {
    return calculate_learning_phase(card, quality, is_correct, now, learning_steps);
  }

  // Graduated: use FSRS for long-term scheduling
  // But if failed, return to learning phase
  if !is_correct {
    return return_to_learning(card, now, learning_steps);
  }

  // Correct answer on graduated card: use FSRS
  calculate_fsrs_graduated(card, quality, desired_retention, now)
}

/// Handle learning phase with short intra-day intervals
fn calculate_learning_phase(
  card: &Card,
  quality: u8,
  is_correct: bool,
  now: DateTime<Utc>,
  learning_steps: &[i64; 4],
) -> FsrsResult {
  if !is_correct {
    // Failed: reset to step 0, review in first step interval
    let next_review = now + Duration::minutes(learning_steps[0]);
    return FsrsResult {
      next_review,
      stability: card.fsrs_stability.unwrap_or(0.0),
      difficulty: card.fsrs_difficulty.unwrap_or(5.0),
      state: FsrsState::Learning,
      learning_step: 0,
      repetitions: 0,
    };
  }

  // Passed: advance to next step
  let next_step = card.learning_step + 1;

  if next_step >= GRADUATING_STEP {
    // Graduating! Move to FSRS with initial 1-day interval
    // Initialize FSRS state for the card
    let fsrs = FSRS::new(Some(&DEFAULT_PARAMETERS)).expect("Failed to initialize FSRS");
    let next_states = fsrs
      .next_states(None, 0.9, 0)
      .expect("Failed to calculate initial FSRS state");

    // Use "Good" rating for graduation
    let scheduled = match quality {
      5 => &next_states.easy,
      _ => &next_states.good,
    };

    FsrsResult {
      next_review: now + Duration::days(1),
      stability: scheduled.memory.stability as f64,
      difficulty: scheduled.memory.difficulty as f64,
      state: FsrsState::Review,
      learning_step: GRADUATING_STEP,
      repetitions: 1, // First "real" repetition
    }
  } else {
    // Still in learning phase: schedule next step
    let minutes = learning_steps[next_step as usize];
    let next_review = now + Duration::minutes(minutes);

    FsrsResult {
      next_review,
      stability: card.fsrs_stability.unwrap_or(0.0),
      difficulty: card.fsrs_difficulty.unwrap_or(5.0),
      state: FsrsState::Learning,
      learning_step: next_step,
      repetitions: 0, // Still learning, not counted as repetition
    }
  }
}

/// Return a graduated card to learning phase after failure
fn return_to_learning(card: &Card, now: DateTime<Utc>, learning_steps: &[i64; 4]) -> FsrsResult {
  let next_review = now + Duration::minutes(learning_steps[0]);
  FsrsResult {
    next_review,
    // Preserve existing FSRS state (will be updated when re-graduating)
    stability: card.fsrs_stability.unwrap_or(0.0),
    difficulty: card.fsrs_difficulty.unwrap_or(5.0),
    state: FsrsState::Relearning,
    learning_step: 0, // Back to step 0
    repetitions: 0,   // Reset repetitions
  }
}

/// Calculate FSRS scheduling for graduated cards (correct answers only)
fn calculate_fsrs_graduated(
  card: &Card,
  quality: u8,
  desired_retention: f64,
  now: DateTime<Utc>,
) -> FsrsResult {
  let fsrs = FSRS::new(Some(&DEFAULT_PARAMETERS)).expect("Failed to initialize FSRS");

  // Get current memory state from card
  let current_memory = match (card.fsrs_stability, card.fsrs_difficulty) {
    (Some(stability), Some(difficulty)) => Some(MemoryState {
      stability: stability as f32,
      difficulty: difficulty as f32,
    }),
    _ => None,
  };

  // Calculate elapsed days since last review
  let elapsed_days = (now - card.next_review).num_days().max(0) as u32;

  // Get next states for all possible ratings
  let next_states = fsrs
    .next_states(current_memory, desired_retention as f32, elapsed_days)
    .expect("Failed to calculate FSRS next states");

  // Select the state based on quality rating
  let scheduled = match quality {
    2 => &next_states.hard,  // Hard
    4 => &next_states.good,  // Good
    5 => &next_states.easy,  // Easy
    _ => &next_states.good,  // Default to Good
  };

  // Calculate next review time (FSRS intervals for graduated cards)
  let interval_days = scheduled.interval.round() as i64;
  let next_review = now + Duration::days(interval_days.max(1));

  FsrsResult {
    next_review,
    stability: scheduled.memory.stability as f64,
    difficulty: scheduled.memory.difficulty as f64,
    state: FsrsState::Review,
    learning_step: card.learning_step, // Stays graduated
    repetitions: card.repetitions + 1,
  }
}

/// Migrate a card from SM-2 to FSRS
/// Uses the card's current SM-2 data to estimate initial FSRS state
///
/// TODO: Planned feature - SM2→FSRS migration tool
/// Will be used when enabling FSRS for existing databases with SM-2 history
#[allow(dead_code)]
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

  #[test]
  fn test_new_card_learning_step() {
    let card = make_test_card();
    let result = calculate_fsrs_review(&card, 4, 0.9, false);

    // New card (step 0) should advance to step 1, not graduate
    assert_eq!(result.learning_step, 1);
    assert_eq!(result.repetitions, 0); // Still learning
    assert_eq!(result.state, FsrsState::Learning);
    // Should be scheduled for ~10 minutes (step 1) in normal mode
    let minutes_until = (result.next_review - Utc::now()).num_minutes();
    assert!(minutes_until >= 9 && minutes_until <= 11);
  }

  #[test]
  fn test_learning_step_progression() {
    let mut card = make_test_card();

    // Step 0 -> 1 (10 min in normal mode)
    card.learning_step = 0;
    let result = calculate_fsrs_review(&card, 4, 0.9, false);
    assert_eq!(result.learning_step, 1);

    // Step 1 -> 2 (1 hr)
    card.learning_step = 1;
    let result = calculate_fsrs_review(&card, 4, 0.9, false);
    assert_eq!(result.learning_step, 2);

    // Step 2 -> 3 (4 hr)
    card.learning_step = 2;
    let result = calculate_fsrs_review(&card, 4, 0.9, false);
    assert_eq!(result.learning_step, 3);

    // Step 3 -> 4 (graduated!)
    card.learning_step = 3;
    let result = calculate_fsrs_review(&card, 4, 0.9, false);
    assert_eq!(result.learning_step, 4);
    assert_eq!(result.repetitions, 1); // First real repetition
    assert_eq!(result.state, FsrsState::Review);
    // Should be scheduled for 1 day
    let days_until = (result.next_review - Utc::now()).num_hours();
    assert!(days_until >= 23 && days_until <= 25);
  }

  #[test]
  fn test_focus_mode_faster_steps() {
    let card = make_test_card();
    let result = calculate_fsrs_review(&card, 4, 0.9, true); // Focus mode

    // New card (step 0) should advance to step 1
    assert_eq!(result.learning_step, 1);
    // Should be scheduled for ~5 minutes (step 1) in focus mode
    let minutes_until = (result.next_review - Utc::now()).num_minutes();
    assert!(minutes_until >= 4 && minutes_until <= 6);
  }

  #[test]
  fn test_failed_learning_resets_to_step_0() {
    let mut card = make_test_card();
    card.learning_step = 2; // In learning at step 2

    let result = calculate_fsrs_review(&card, 0, 0.9, false); // Fail

    // Should reset to step 0
    assert_eq!(result.learning_step, 0);
    assert_eq!(result.state, FsrsState::Learning);
    // Should be scheduled for 1 minute
    let minutes_until = (result.next_review - Utc::now()).num_seconds();
    assert!(minutes_until >= 55 && minutes_until <= 65);
  }

  #[test]
  fn test_graduated_card_uses_fsrs() {
    let mut card = make_test_card();
    card.learning_step = 4; // Graduated
    card.repetitions = 1;
    card.fsrs_stability = Some(5.0); // Higher stability for meaningful interval
    card.fsrs_difficulty = Some(5.0);
    card.fsrs_state = Some(FsrsState::Review);

    let result = calculate_fsrs_review(&card, 4, 0.9, false);

    // Should stay graduated and increment repetitions
    assert_eq!(result.learning_step, 4);
    assert_eq!(result.repetitions, 2);
    assert_eq!(result.state, FsrsState::Review);
    // FSRS should schedule for >= 1 day (we enforce .max(1) in the code)
    let hours_until = (result.next_review - Utc::now()).num_hours();
    assert!(hours_until >= 23, "Expected at least 23 hours, got {}", hours_until);
  }

  #[test]
  fn test_graduated_fail_returns_to_learning() {
    let mut card = make_test_card();
    card.learning_step = 4; // Graduated
    card.repetitions = 5;
    card.fsrs_stability = Some(10.0);
    card.fsrs_difficulty = Some(5.0);
    card.fsrs_state = Some(FsrsState::Review);

    let result = calculate_fsrs_review(&card, 0, 0.9, false); // Fail

    // Should return to learning phase
    assert_eq!(result.learning_step, 0);
    assert_eq!(result.repetitions, 0);
    assert_eq!(result.state, FsrsState::Relearning);
    // Should be scheduled for 1 minute
    let minutes_until = (result.next_review - Utc::now()).num_seconds();
    assert!(minutes_until >= 55 && minutes_until <= 65);
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
