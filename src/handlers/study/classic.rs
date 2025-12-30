//! Classic flip-card study mode handlers.

use askama::Template;
use axum::response::{Html, IntoResponse};
use axum::Form;

use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::domain::{ReviewQuality, StudyMode};
use crate::srs;

use super::templates::{CardTemplate, NoCardsTemplate, ReviewForm, StudyTemplate};
use super::{get_character_type, get_review_direction, get_tracked_character};

pub async fn study_start(auth: AuthContext) -> impl IntoResponse {
  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };
  let cards = db::get_due_cards(&conn, 1, None).log_warn_default("Failed to get due cards");

  if let Some(card) = cards.first() {
    let template = StudyTemplate {
      card_id: card.id,
      front: card.front.clone(),
      main_answer: card.main_answer.clone(),
      description: card.description.clone(),
      tier: card.tier,
      is_reverse: card.is_reverse,
      has_card: true,
    };
    Html(template.render().unwrap_or_default())
  } else {
    let template = StudyTemplate {
      card_id: 0,
      front: String::new(),
      main_answer: String::new(),
      description: None,
      tier: 0,
      is_reverse: false,
      has_card: false,
    };
    Html(template.render().unwrap_or_default())
  }
}

pub async fn submit_review(auth: AuthContext, Form(form): Form<ReviewForm>) -> impl IntoResponse {
  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  // Get current card
  if let Ok(Some(card)) = db::get_card_by_id(&conn, form.card_id) {
    // Calculate new review values (learning steps + SM-2)
    let result = srs::calculate_review(
      form.quality,
      card.ease_factor,
      card.interval_days,
      card.repetitions,
      card.learning_step,
    );

    // Determine if answer was correct
    let correct = ReviewQuality::from_u8(form.quality)
      .map(|q| q.is_correct())
      .unwrap_or(false);

    // Update card
    let _ = db::update_card_after_review(
      &conn,
      card.id,
      result.ease_factor,
      result.interval_days,
      result.repetitions,
      result.next_review,
      result.learning_step,
      correct,
    );

    // Log review with enhanced tracking
    let direction = get_review_direction(&card);
    let _ = db::insert_review_log_enhanced(
      &conn,
      card.id,
      form.quality,
      correct,
      StudyMode::Classic,
      direction,
      None, // response_time_ms not tracked in classic mode
      0,    // hints not available in classic mode
    );

    // Update character stats
    let tracked_char = get_tracked_character(&card);
    let char_type = get_character_type(&card);
    let _ = db::update_character_stats(&conn, tracked_char, char_type, correct);
  }

  // Get next card, excluding sibling of the just-reviewed card
  let cards =
    db::get_due_cards(&conn, 1, Some(form.card_id)).log_warn_default("Failed to get due cards");

  if let Some(next_card) = cards.first() {
    let template = CardTemplate {
      card_id: next_card.id,
      front: next_card.front.clone(),
      main_answer: next_card.main_answer.clone(),
      description: next_card.description.clone(),
      tier: next_card.tier,
      is_reverse: next_card.is_reverse,
    };
    Html(template.render().unwrap_or_default())
  } else {
    let template = NoCardsTemplate {};
    Html(template.render().unwrap_or_default())
  }
}
