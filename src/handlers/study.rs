use askama::Template;
use axum::{
  extract::State,
  response::{Html, IntoResponse},
  Form,
};
use serde::Deserialize;

use crate::db::{self, DbPool};
use crate::domain::{ReviewLog, ReviewQuality};
use crate::srs;

#[derive(Template)]
#[template(path = "study.html")]
pub struct StudyTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub has_card: bool,
}

#[derive(Template)]
#[template(path = "card.html")]
pub struct CardTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
}

#[derive(Template)]
#[template(path = "practice_card.html")]
pub struct PracticeCardTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
}

#[derive(Template)]
#[template(path = "no_cards.html")]
pub struct NoCardsTemplate {}

#[derive(Template)]
#[template(path = "practice.html")]
pub struct PracticeTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
}

pub async fn study_start(State(pool): State<DbPool>) -> impl IntoResponse {
  let conn = pool.lock().unwrap();
  let cards = db::get_due_cards(&conn, 1, None).unwrap_or_default();

  if let Some(card) = cards.first() {
    let template = StudyTemplate {
      card_id: card.id,
      front: card.front.clone(),
      main_answer: card.main_answer.clone(),
      description: card.description.clone(),
      tier: card.tier,
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
      has_card: false,
    };
    Html(template.render().unwrap_or_default())
  }
}

#[derive(Deserialize)]
pub struct ReviewForm {
  pub card_id: i64,
  pub quality: u8,
}

pub async fn submit_review(
  State(pool): State<DbPool>,
  Form(form): Form<ReviewForm>,
) -> impl IntoResponse {
  let conn = pool.lock().unwrap();

  // Get current card
  if let Ok(Some(card)) = db::get_card_by_id(&conn, form.card_id) {
    // Calculate new SM-2 values
    let result = srs::calculate_sm2(
      form.quality,
      card.ease_factor,
      card.interval_days,
      card.repetitions,
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
      correct,
    );

    // Log review
    let log = ReviewLog::new(card.id, form.quality);
    let _ = db::insert_review_log(&conn, &log);
  }

  // Get next card, excluding sibling of the just-reviewed card
  let cards = db::get_due_cards(&conn, 1, Some(form.card_id)).unwrap_or_default();

  if let Some(next_card) = cards.first() {
    let template = CardTemplate {
      card_id: next_card.id,
      front: next_card.front.clone(),
      main_answer: next_card.main_answer.clone(),
      description: next_card.description.clone(),
      tier: next_card.tier,
    };
    Html(template.render().unwrap_or_default())
  } else {
    let template = NoCardsTemplate {};
    Html(template.render().unwrap_or_default())
  }
}

// Practice mode - review cards even when not due
pub async fn practice_start(State(pool): State<DbPool>) -> impl IntoResponse {
  let conn = pool.lock().unwrap();
  let cards = db::get_practice_cards(&conn, 1, None).unwrap_or_default();

  if let Some(card) = cards.first() {
    let template = PracticeTemplate {
      card_id: card.id,
      front: card.front.clone(),
      main_answer: card.main_answer.clone(),
      description: card.description.clone(),
      tier: card.tier,
    };
    Html(template.render().unwrap_or_default())
  } else {
    Html("<p>No cards available for practice.</p>".to_string())
  }
}

#[derive(Deserialize)]
pub struct PracticeForm {
  pub card_id: i64,
}

pub async fn practice_next(
  State(pool): State<DbPool>,
  Form(form): Form<PracticeForm>,
) -> impl IntoResponse {
  let conn = pool.lock().unwrap();

  // Get next random card, excluding sibling of the just-practiced card
  let cards = db::get_practice_cards(&conn, 1, Some(form.card_id)).unwrap_or_default();

  if let Some(next_card) = cards.first() {
    let template = PracticeCardTemplate {
      card_id: next_card.id,
      front: next_card.front.clone(),
      main_answer: next_card.main_answer.clone(),
      description: next_card.description.clone(),
      tier: next_card.tier,
    };
    Html(template.render().unwrap_or_default())
  } else {
    Html("<p>No more cards available.</p>".to_string())
  }
}
