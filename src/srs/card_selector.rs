//! Weighted card selection with reinforcement queue for failed cards.
//!
//! This module provides intelligent card selection that prioritizes:
//! - Cards with lower success rates
//! - Cards that were recently failed (reinforcement)
//! - Cards that haven't been reviewed much
//! - Cards that haven't been seen in a while

use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use rusqlite::{params, Connection, Result};
use std::collections::VecDeque;

use crate::domain::Card;

/// Represents a card with its calculated selection weight
#[derive(Debug, Clone)]
pub struct CardWeight {
  pub card_id: i64,
  pub weight: f64,
}

/// Session state for tracking reinforcement queue
/// Failed cards are added to the queue and shown again within 3-5 cards
#[derive(Debug, Clone, Default)]
pub struct StudySession {
  /// Queue of card IDs that need reinforcement (recently failed)
  pub reinforcement_queue: VecDeque<i64>,
  /// Counter since last reinforcement card was shown
  pub cards_since_reinforce: u32,
  /// Last card ID shown (to avoid immediate repeats)
  pub last_card_id: Option<i64>,
}

impl StudySession {
  pub fn new() -> Self {
    Self::default()
  }

  /// Add a failed card to the reinforcement queue
  pub fn add_failed_card(&mut self, card_id: i64) {
    // Avoid duplicates in queue
    if !self.reinforcement_queue.contains(&card_id) {
      self.reinforcement_queue.push_back(card_id);
    }
  }

  /// Remove a card from reinforcement queue (when answered correctly)
  pub fn remove_from_reinforcement(&mut self, card_id: i64) {
    self.reinforcement_queue.retain(|&id| id != card_id);
  }

  /// Check if it's time to show a reinforcement card
  pub fn should_show_reinforcement(&self) -> bool {
    !self.reinforcement_queue.is_empty() && self.cards_since_reinforce >= 3
  }

  /// Get next reinforcement card if available and due
  pub fn pop_reinforcement(&mut self) -> Option<i64> {
    if self.should_show_reinforcement() {
      self.cards_since_reinforce = 0;
      self.reinforcement_queue.pop_front()
    } else {
      None
    }
  }

  /// Increment the counter after showing a regular card
  pub fn increment_counter(&mut self) {
    self.cards_since_reinforce += 1;
  }
}

/// Review data for weight calculation
#[derive(Debug)]
pub struct RecentReview {
  pub reviewed_at: DateTime<Utc>,
  pub is_correct: bool,
}

/// Calculate the selection weight for a card based on recent performance
pub fn calculate_card_weight(
  total_reviews: i64,
  correct_reviews: i64,
  recent_reviews: &[RecentReview],
) -> f64 {
  let mut weight = 1.0;

  // Factor 1: Success rate (lower success = higher weight)
  // Range: 1.0 (100% success) to 2.0 (0% success)
  let success_rate = if total_reviews > 0 {
    correct_reviews as f64 / total_reviews as f64
  } else {
    0.5 // New cards get neutral weight
  };
  weight *= 2.0 - success_rate;

  // Factor 2: Recency of last failure (recent failure = much higher weight)
  if let Some(last_fail) = recent_reviews.iter().filter(|r| !r.is_correct).last() {
    let minutes_since_fail = (Utc::now() - last_fail.reviewed_at).num_minutes();
    if minutes_since_fail < 5 {
      weight *= 10.0; // Failed in last 5 min → 10x weight
    } else if minutes_since_fail < 30 {
      weight *= 3.0; // Failed in last 30 min → 3x weight
    } else if minutes_since_fail < 60 {
      weight *= 1.5; // Failed in last hour → 1.5x weight
    }
  }

  // Factor 3: Review count (less reviewed = higher weight)
  // New or barely reviewed cards get priority
  if total_reviews == 0 {
    weight *= 2.0; // Never reviewed → 2x
  } else if total_reviews < 3 {
    weight *= 1.5; // Barely reviewed → 1.5x
  } else if total_reviews < 5 {
    weight *= 1.2; // Few reviews → 1.2x
  }

  // Factor 4: Time since last review (longer = slightly higher)
  if let Some(last) = recent_reviews.last() {
    let hours_since = (Utc::now() - last.reviewed_at).num_hours();
    // Gradually increase weight up to 2x for cards not seen in 10+ hours
    weight *= 1.0 + (hours_since as f64 * 0.1).min(1.0);
  } else {
    // Never reviewed - give it a boost
    weight *= 1.5;
  }

  weight
}

/// Get recent reviews for a card (last 7 days)
pub fn get_recent_reviews(conn: &Connection, card_id: i64) -> Result<Vec<RecentReview>> {
  let seven_days_ago = (Utc::now() - Duration::days(7)).to_rfc3339();

  let mut stmt = conn.prepare(
    r#"
    SELECT reviewed_at, is_correct
    FROM review_logs
    WHERE card_id = ?1 AND reviewed_at >= ?2
    ORDER BY reviewed_at ASC
    "#,
  )?;

  let reviews = stmt
    .query_map(params![card_id, seven_days_ago], |row| {
      let reviewed_at_str: String = row.get(0)?;
      let is_correct: Option<i32> = row.get(1)?;
      Ok(RecentReview {
        reviewed_at: DateTime::parse_from_rfc3339(&reviewed_at_str)
          .map(|dt| dt.with_timezone(&Utc))
          .unwrap_or_else(|_| Utc::now()),
        is_correct: is_correct.unwrap_or(1) == 1,
      })
    })?
    .collect::<Result<Vec<_>>>()?;

  Ok(reviews)
}

/// Calculate weights for all due cards
pub fn calculate_all_weights(conn: &Connection, cards: &[Card]) -> Result<Vec<CardWeight>> {
  let mut weights = Vec::with_capacity(cards.len());

  for card in cards {
    let recent = get_recent_reviews(conn, card.id)?;
    let weight = calculate_card_weight(card.total_reviews, card.correct_reviews, &recent);
    weights.push(CardWeight {
      card_id: card.id,
      weight,
    });
  }

  Ok(weights)
}

/// Select a card using weighted random selection
/// Higher weight = more likely to be selected
pub fn weighted_random_select(weights: &[CardWeight], exclude_id: Option<i64>) -> Option<i64> {
  // Filter out excluded card
  let available: Vec<_> = weights
    .iter()
    .filter(|w| exclude_id.map_or(true, |id| w.card_id != id))
    .collect();

  if available.is_empty() {
    return None;
  }

  // If only one card, return it
  if available.len() == 1 {
    return Some(available[0].card_id);
  }

  // Calculate total weight
  let total_weight: f64 = available.iter().map(|w| w.weight).sum();

  if total_weight <= 0.0 {
    // Fallback to random if weights are invalid
    let idx = rand::rng().random_range(0..available.len());
    return Some(available[idx].card_id);
  }

  // Weighted random selection
  let mut rng = rand::rng();
  let mut target = rng.random_range(0.0..total_weight);

  for w in &available {
    target -= w.weight;
    if target <= 0.0 {
      return Some(w.card_id);
    }
  }

  // Fallback to last card
  Some(available.last().unwrap().card_id)
}

/// Main entry point: get next card considering reinforcement queue and weights
pub fn select_next_card(
  conn: &Connection,
  session: &mut StudySession,
  available_cards: &[Card],
) -> Result<Option<i64>> {
  // First, check if we should show a reinforcement card
  if let Some(reinforce_id) = session.pop_reinforcement() {
    // Verify the card is still in our available set
    if available_cards.iter().any(|c| c.id == reinforce_id) {
      session.last_card_id = Some(reinforce_id);
      return Ok(Some(reinforce_id));
    }
    // Card not available anymore, try next in queue
  }

  // Calculate weights for available cards
  let weights = calculate_all_weights(conn, available_cards)?;

  // Select using weighted random, excluding last shown card
  if let Some(card_id) = weighted_random_select(&weights, session.last_card_id) {
    session.increment_counter();
    session.last_card_id = Some(card_id);
    Ok(Some(card_id))
  } else {
    Ok(None)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_weight_calculation_new_card() {
    let weight = calculate_card_weight(0, 0, &[]);
    // New card should have boosted weight
    assert!(weight > 2.0);
  }

  #[test]
  fn test_weight_calculation_struggling_card() {
    let weight = calculate_card_weight(10, 2, &[]);
    // Low success rate should have high weight
    assert!(weight > 1.5);
  }

  #[test]
  fn test_weight_calculation_mastered_card() {
    let weight = calculate_card_weight(20, 19, &[]);
    // High success rate should have lower weight than struggling cards (< 2.0)
    // Note: Even mastered cards get a small boost if no recent reviews provided
    assert!(weight < 2.0);
  }

  #[test]
  fn test_session_reinforcement() {
    let mut session = StudySession::new();

    // Add failed card
    session.add_failed_card(42);
    assert!(!session.should_show_reinforcement()); // Need 3 cards first

    // Simulate showing 3 cards
    session.increment_counter();
    session.increment_counter();
    session.increment_counter();

    assert!(session.should_show_reinforcement());

    let reinforced = session.pop_reinforcement();
    assert_eq!(reinforced, Some(42));
    assert!(session.reinforcement_queue.is_empty());
  }

  // Additional StudySession tests

  #[test]
  fn test_session_new_is_empty() {
    let session = StudySession::new();
    assert!(session.reinforcement_queue.is_empty());
    assert_eq!(session.cards_since_reinforce, 0);
    assert!(session.last_card_id.is_none());
  }

  #[test]
  fn test_add_failed_card_no_duplicates() {
    let mut session = StudySession::new();

    session.add_failed_card(42);
    session.add_failed_card(42); // Duplicate
    session.add_failed_card(42); // Duplicate

    assert_eq!(session.reinforcement_queue.len(), 1);
  }

  #[test]
  fn test_add_multiple_failed_cards() {
    let mut session = StudySession::new();

    session.add_failed_card(1);
    session.add_failed_card(2);
    session.add_failed_card(3);

    assert_eq!(session.reinforcement_queue.len(), 3);
  }

  #[test]
  fn test_remove_from_reinforcement() {
    let mut session = StudySession::new();

    session.add_failed_card(1);
    session.add_failed_card(2);
    session.add_failed_card(3);

    session.remove_from_reinforcement(2);

    assert_eq!(session.reinforcement_queue.len(), 2);
    assert!(!session.reinforcement_queue.contains(&2));
  }

  #[test]
  fn test_remove_nonexistent_card() {
    let mut session = StudySession::new();

    session.add_failed_card(1);
    session.remove_from_reinforcement(999); // Not in queue

    assert_eq!(session.reinforcement_queue.len(), 1);
  }

  #[test]
  fn test_should_show_reinforcement_empty_queue() {
    let mut session = StudySession::new();

    // Even with counter at 3, empty queue = false
    session.increment_counter();
    session.increment_counter();
    session.increment_counter();

    assert!(!session.should_show_reinforcement());
  }

  #[test]
  fn test_should_show_reinforcement_needs_3_cards() {
    let mut session = StudySession::new();
    session.add_failed_card(42);

    assert!(!session.should_show_reinforcement()); // counter = 0

    session.increment_counter();
    assert!(!session.should_show_reinforcement()); // counter = 1

    session.increment_counter();
    assert!(!session.should_show_reinforcement()); // counter = 2

    session.increment_counter();
    assert!(session.should_show_reinforcement()); // counter = 3
  }

  #[test]
  fn test_pop_reinforcement_resets_counter() {
    let mut session = StudySession::new();
    session.add_failed_card(42);

    session.increment_counter();
    session.increment_counter();
    session.increment_counter();

    let _ = session.pop_reinforcement();

    assert_eq!(session.cards_since_reinforce, 0);
  }

  #[test]
  fn test_pop_reinforcement_fifo_order() {
    let mut session = StudySession::new();
    session.add_failed_card(1);
    session.add_failed_card(2);
    session.add_failed_card(3);

    // Get to 3 cards first
    session.increment_counter();
    session.increment_counter();
    session.increment_counter();

    // Should return in FIFO order
    assert_eq!(session.pop_reinforcement(), Some(1));

    // Need to wait for 3 more cards
    session.increment_counter();
    session.increment_counter();
    session.increment_counter();
    assert_eq!(session.pop_reinforcement(), Some(2));
  }

  // Weight calculation tests

  #[test]
  fn test_weight_perfect_success_rate() {
    let weight = calculate_card_weight(10, 10, &[]);
    // 100% success rate: weight = 1.0 * (2.0 - 1.0) = 1.0
    // Plus review count factor (>=5 reviews = no boost)
    // Plus no recent reviews boost: 1.5x
    assert!(weight >= 1.0 && weight < 2.0);
  }

  #[test]
  fn test_weight_zero_success_rate() {
    let weight = calculate_card_weight(10, 0, &[]);
    // 0% success rate: factor = 2.0 - 0.0 = 2.0
    // This gives higher weight than perfect success
    assert!(weight >= 2.0);
  }

  #[test]
  fn test_weight_recent_failure_boost() {
    let recent_fail = RecentReview {
      reviewed_at: Utc::now() - Duration::minutes(2),
      is_correct: false,
    };
    let weight = calculate_card_weight(5, 4, &[recent_fail]);

    // Recent failure (< 5 min) gives 10x multiplier
    assert!(weight >= 10.0);
  }

  #[test]
  fn test_weight_30min_failure_boost() {
    let recent_fail = RecentReview {
      reviewed_at: Utc::now() - Duration::minutes(15),
      is_correct: false,
    };
    let weight = calculate_card_weight(5, 4, &[recent_fail]);

    // 15 min ago failure gives 3x multiplier
    assert!(weight >= 3.0);
  }

  #[test]
  fn test_weight_hour_failure_boost() {
    let recent_fail = RecentReview {
      reviewed_at: Utc::now() - Duration::minutes(45),
      is_correct: false,
    };
    let weight = calculate_card_weight(5, 4, &[recent_fail]);

    // 45 min ago failure gives 1.5x multiplier
    assert!(weight >= 1.5);
  }

  #[test]
  fn test_weight_barely_reviewed_boost() {
    // Card with 1 review
    let weight1 = calculate_card_weight(1, 1, &[]);
    // Card with 10 reviews
    let weight10 = calculate_card_weight(10, 10, &[]);

    // Barely reviewed cards should have higher weight
    assert!(weight1 > weight10);
  }

  #[test]
  fn test_weight_time_since_last_review() {
    let recent = RecentReview {
      reviewed_at: Utc::now() - Duration::hours(1),
      is_correct: true,
    };
    let old = RecentReview {
      reviewed_at: Utc::now() - Duration::hours(10),
      is_correct: true,
    };

    let weight_recent = calculate_card_weight(5, 5, &[recent]);
    let weight_old = calculate_card_weight(5, 5, &[old]);

    // Older last review should have higher weight
    assert!(weight_old > weight_recent);
  }

  // CardWeight struct tests

  #[test]
  fn test_card_weight_struct() {
    let cw = CardWeight {
      card_id: 42,
      weight: 2.5,
    };
    assert_eq!(cw.card_id, 42);
    assert!((cw.weight - 2.5).abs() < f64::EPSILON);
  }

  // Weighted selection tests

  #[test]
  fn test_weighted_select_single_card() {
    let weights = vec![CardWeight { card_id: 42, weight: 1.0 }];
    let result = weighted_random_select(&weights, None);
    assert_eq!(result, Some(42));
  }

  #[test]
  fn test_weighted_select_excludes_card() {
    let weights = vec![
      CardWeight { card_id: 1, weight: 1.0 },
      CardWeight { card_id: 2, weight: 1.0 },
    ];
    // Always select when excluding the other option
    let result = weighted_random_select(&weights, Some(1));
    assert_eq!(result, Some(2));
  }

  #[test]
  fn test_weighted_select_empty() {
    let weights: Vec<CardWeight> = vec![];
    let result = weighted_random_select(&weights, None);
    assert_eq!(result, None);
  }

  #[test]
  fn test_weighted_select_all_excluded() {
    let weights = vec![CardWeight { card_id: 42, weight: 1.0 }];
    let result = weighted_random_select(&weights, Some(42));
    assert_eq!(result, None);
  }
}
