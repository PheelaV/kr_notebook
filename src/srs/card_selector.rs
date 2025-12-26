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
}
