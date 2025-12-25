use chrono::{DateTime, Duration, Utc};

const MIN_EASE_FACTOR: f64 = 1.3;

// Learning steps in minutes (Anki-style: short intervals before graduating to SM-2)
// Step 0 = new card, steps 1-4 are learning phase, step 5+ means graduated
const LEARNING_STEPS_MINUTES: [i64; 4] = [1, 10, 60, 240]; // 1min, 10min, 1hr, 4hr

pub struct Sm2Result {
  pub ease_factor: f64,
  pub interval_days: i64,
  pub repetitions: i64,
  pub next_review: DateTime<Utc>,
  pub learning_step: i64,
}

/// Calculate next review using Anki-style learning steps + SM-2 for graduated cards
pub fn calculate_review(
  quality: u8,
  current_ease_factor: f64,
  current_interval: i64,
  current_repetitions: i64,
  current_learning_step: i64,
) -> Sm2Result {
  // In learning phase (step 0-4)
  if current_learning_step < LEARNING_STEPS_MINUTES.len() as i64 {
    return calculate_learning_step(
      quality,
      current_ease_factor,
      current_learning_step,
    );
  }

  // Graduated: use SM-2
  calculate_sm2(
    quality,
    current_ease_factor,
    current_interval,
    current_repetitions,
    current_learning_step,
  )
}

/// Handle learning phase with short intervals
fn calculate_learning_step(
  quality: u8,
  current_ease_factor: f64,
  current_step: i64,
) -> Sm2Result {
  if quality < 3 {
    // Failed: reset to step 0
    let next_review = Utc::now() + Duration::minutes(LEARNING_STEPS_MINUTES[0]);
    Sm2Result {
      ease_factor: current_ease_factor,
      interval_days: 0,
      repetitions: 0,
      next_review,
      learning_step: 0,
    }
  } else {
    // Passed: advance to next step
    let next_step = current_step + 1;

    if next_step >= LEARNING_STEPS_MINUTES.len() as i64 {
      // Graduated! Move to SM-2 with initial interval of 1 day
      let next_review = Utc::now() + Duration::days(1);
      Sm2Result {
        ease_factor: current_ease_factor,
        interval_days: 1,
        repetitions: 1, // Count as first SM-2 repetition
        next_review,
        learning_step: next_step,
      }
    } else {
      // Still in learning phase
      let minutes = LEARNING_STEPS_MINUTES[next_step as usize];
      let next_review = Utc::now() + Duration::minutes(minutes);
      Sm2Result {
        ease_factor: current_ease_factor,
        interval_days: 0,
        repetitions: 0,
        next_review,
        learning_step: next_step,
      }
    }
  }
}

/// SM-2 algorithm for graduated cards
fn calculate_sm2(
  quality: u8,
  current_ease_factor: f64,
  current_interval: i64,
  current_repetitions: i64,
  current_learning_step: i64,
) -> Sm2Result {
  let q = quality as f64;

  // Calculate new ease factor
  // EF' = EF + (0.1 - (5 - q) * (0.08 + (5 - q) * 0.02))
  let ease_delta = 0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02);
  let new_ease_factor = (current_ease_factor + ease_delta).max(MIN_EASE_FACTOR);

  if quality < 3 {
    // Failed: go back to learning phase
    let next_review = Utc::now() + Duration::minutes(LEARNING_STEPS_MINUTES[0]);
    Sm2Result {
      ease_factor: new_ease_factor,
      interval_days: 0,
      repetitions: 0,
      next_review,
      learning_step: 0, // Reset to learning phase
    }
  } else {
    // Successful review - SM-2 intervals based on new repetition count
    let new_repetitions = current_repetitions + 1;
    // rep 1 = just graduated (1 day), rep 2 = 6 days, rep 3+ = exponential
    let interval = match new_repetitions {
      1 => 1,                                                            // Just graduated
      2 => 6,                                                            // Second review
      _ => ((current_interval as f64) * new_ease_factor).round() as i64, // Exponential growth
    };
    let next_review = Utc::now() + Duration::days(interval);

    Sm2Result {
      ease_factor: new_ease_factor,
      interval_days: interval,
      repetitions: new_repetitions,
      next_review,
      learning_step: current_learning_step,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_new_card_learning_step_0() {
    // New card starts at step 0, first correct answer moves to step 1 (10 min)
    let result = calculate_review(4, 2.5, 0, 0, 0);
    assert_eq!(result.learning_step, 1);
    assert_eq!(result.interval_days, 0); // Still in learning, using minutes
    assert_eq!(result.repetitions, 0);
  }

  #[test]
  fn test_learning_step_progression() {
    // Step 1 -> Step 2 (1hr)
    let result = calculate_review(4, 2.5, 0, 0, 1);
    assert_eq!(result.learning_step, 2);

    // Step 2 -> Step 3 (4hr)
    let result = calculate_review(4, 2.5, 0, 0, 2);
    assert_eq!(result.learning_step, 3);

    // Step 3 -> Step 4 (graduated!)
    let result = calculate_review(4, 2.5, 0, 0, 3);
    assert_eq!(result.learning_step, 4);
    assert_eq!(result.interval_days, 1); // First SM-2 interval
    assert_eq!(result.repetitions, 1);
  }

  #[test]
  fn test_learning_step_fail_resets() {
    // Failing at step 2 should reset to step 0
    let result = calculate_review(1, 2.5, 0, 0, 2);
    assert_eq!(result.learning_step, 0);
  }

  #[test]
  fn test_graduated_card_uses_sm2() {
    // Graduated card (step 4+) uses SM-2
    let result = calculate_review(4, 2.5, 1, 1, 4);
    assert_eq!(result.learning_step, 4);
    assert_eq!(result.repetitions, 2);
    assert_eq!(result.interval_days, 6); // SM-2 second interval
  }

  #[test]
  fn test_graduated_fail_returns_to_learning() {
    // Failing a graduated card should go back to learning phase
    let result = calculate_review(1, 2.5, 15, 5, 4);
    assert_eq!(result.learning_step, 0);
    assert_eq!(result.repetitions, 0);
    assert_eq!(result.interval_days, 0);
  }

  #[test]
  fn test_sm2_interval_grows() {
    // After graduation, intervals should grow exponentially
    let mut ef = 2.5;
    let mut interval: i64 = 1;
    let mut reps: i64 = 1;
    let step: i64 = 4;

    for i in 0..3 {
      let result = calculate_review(4, ef, interval, reps, step);
      ef = result.ease_factor;
      interval = result.interval_days;
      reps = result.repetitions;

      match i {
        0 => assert_eq!(interval, 6), // second SM-2 interval
        1 => assert!(interval >= 15), // 6 * 2.5 = 15
        _ => assert!(interval > 30),
      }
    }
  }

  #[test]
  fn test_ease_factor_floor() {
    let mut ef = 2.5;
    let mut interval: i64 = 10;
    let mut reps: i64 = 5;
    let step: i64 = 4;

    for _ in 0..10 {
      // Keep failing
      let result = calculate_review(0, ef, interval, reps, step);
      ef = result.ease_factor;
      interval = result.interval_days;
      reps = result.repetitions;
    }

    assert!(ef >= MIN_EASE_FACTOR);
  }
}
