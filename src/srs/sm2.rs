use chrono::{DateTime, Duration, Utc};

const MIN_EASE_FACTOR: f64 = 1.3;

pub struct Sm2Result {
  pub ease_factor: f64,
  pub interval_days: i64,
  pub repetitions: i64,
  pub next_review: DateTime<Utc>,
}

pub fn calculate_sm2(
  quality: u8,
  current_ease_factor: f64,
  current_interval: i64,
  current_repetitions: i64,
) -> Sm2Result {
  let q = quality as f64;

  // Calculate new ease factor
  // EF' = EF + (0.1 - (5 - q) * (0.08 + (5 - q) * 0.02))
  let ease_delta = 0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02);
  let new_ease_factor = (current_ease_factor + ease_delta).max(MIN_EASE_FACTOR);

  let (new_interval, new_repetitions) = if quality < 3 {
    // Failed review: reset
    (1, 0)
  } else {
    // Successful review
    let interval = match current_repetitions {
      0 => 1,
      1 => 6,
      _ => ((current_interval as f64) * new_ease_factor).round() as i64,
    };
    (interval, current_repetitions + 1)
  };

  let next_review = Utc::now() + Duration::days(new_interval);

  Sm2Result {
    ease_factor: new_ease_factor,
    interval_days: new_interval,
    repetitions: new_repetitions,
    next_review,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_first_review_good() {
    let result = calculate_sm2(4, 2.5, 0, 0);
    assert_eq!(result.repetitions, 1);
    assert_eq!(result.interval_days, 1);
    assert!((result.ease_factor - 2.5).abs() < 0.01);
  }

  #[test]
  fn test_second_review_good() {
    let result = calculate_sm2(4, 2.5, 1, 1);
    assert_eq!(result.repetitions, 2);
    assert_eq!(result.interval_days, 6);
  }

  #[test]
  fn test_third_review_good() {
    let result = calculate_sm2(4, 2.5, 6, 2);
    assert_eq!(result.repetitions, 3);
    // 6 * 2.5 = 15
    assert_eq!(result.interval_days, 15);
  }

  #[test]
  fn test_failed_review_resets() {
    let result = calculate_sm2(0, 2.5, 15, 5);
    assert_eq!(result.repetitions, 0);
    assert_eq!(result.interval_days, 1);
    // Ease factor decreases for failed review
    assert!(result.ease_factor < 2.5);
  }

  #[test]
  fn test_hard_review() {
    let result = calculate_sm2(2, 2.5, 6, 2);
    assert_eq!(result.repetitions, 0);
    assert_eq!(result.interval_days, 1);
  }

  #[test]
  fn test_easy_review_increases_ease() {
    let result = calculate_sm2(5, 2.5, 1, 1);
    assert!(result.ease_factor > 2.5);
    assert_eq!(result.interval_days, 6);
  }

  #[test]
  fn test_ease_factor_floor() {
    // Multiple failed reviews should not go below 1.3
    let mut ef = 2.5;
    let mut interval = 10;
    let mut reps = 5;

    for _ in 0..10 {
      let result = calculate_sm2(0, ef, interval, reps);
      ef = result.ease_factor;
      interval = result.interval_days;
      reps = result.repetitions;
    }

    assert!(ef >= MIN_EASE_FACTOR);
    assert!((ef - MIN_EASE_FACTOR).abs() < 0.01);
  }

  #[test]
  fn test_interval_grows_exponentially() {
    let mut ef = 2.5;
    let mut interval = 0;
    let mut reps = 0;

    // Simulate 5 "Good" reviews
    for i in 0..5 {
      let result = calculate_sm2(4, ef, interval, reps);
      ef = result.ease_factor;
      interval = result.interval_days;
      reps = result.repetitions;

      match i {
        0 => assert_eq!(interval, 1),
        1 => assert_eq!(interval, 6),
        _ => assert!(interval > 6),
      }
    }

    // After 5 good reviews, interval should be substantial
    assert!(interval > 30);
  }
}
