use askama::Template;
use axum::{
  extract::State,
  response::{Html, IntoResponse},
  Form,
};
use rand::seq::SliceRandom;
use serde::Deserialize;

use crate::db::{self, DbPool};
use crate::domain::{Card, ReviewLog, ReviewQuality};
use crate::srs;
use crate::validation::{validate_answer, HintGenerator};

/// Check if a string contains Korean characters (Hangul)
fn is_korean(s: &str) -> bool {
  s.chars().any(|c| {
    // Hangul Syllables: U+AC00 to U+D7A3
    // Hangul Jamo: U+1100 to U+11FF
    // Hangul Compatibility Jamo: U+3130 to U+318F
    let code = c as u32;
    (0xAC00..=0xD7A3).contains(&code)
      || (0x1100..=0x11FF).contains(&code)
      || (0x3130..=0x318F).contains(&code)
  })
}

/// Generate multiple choice options for a card
fn generate_choices(card: &Card, all_cards: &[Card]) -> Vec<String> {
  let correct = card.main_answer.clone();

  // Get other cards from the same tier with Korean answers
  let mut distractors: Vec<String> = all_cards
    .iter()
    .filter(|c| c.id != card.id && c.tier == card.tier && is_korean(&c.main_answer))
    .map(|c| c.main_answer.clone())
    .collect();

  // Shuffle and take up to 3 distractors
  let mut rng = rand::rng();
  distractors.shuffle(&mut rng);
  distractors.truncate(3);

  // Combine correct answer with distractors
  let mut choices = vec![correct];
  choices.extend(distractors);

  // Shuffle final choices
  choices.shuffle(&mut rng);

  choices
}

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

/// Interactive card template with input-based validation
#[derive(Template)]
#[template(path = "interactive_card.html")]
pub struct InteractiveCardTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub validated: bool,
  pub is_correct: bool,
  pub user_answer: String,
  pub quality: u8,
  pub hints_used: u8,
  pub hint_1: String,
  pub hint_2: String,
  pub hint_final: String,
  // Multiple choice fields
  pub is_multiple_choice: bool,
  pub choices: Vec<String>,
}

/// Wrapper template for initial interactive study page load
#[derive(Template)]
#[template(path = "study_interactive.html")]
pub struct StudyInteractiveTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub validated: bool,
  pub is_correct: bool,
  pub user_answer: String,
  pub quality: u8,
  pub hints_used: u8,
  pub hint_1: String,
  pub hint_2: String,
  pub hint_final: String,
  pub is_multiple_choice: bool,
  pub choices: Vec<String>,
  pub has_card: bool,
}

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

/// Get the next card to study based on FSA logic:
/// - Normal mode: due cards only, then practice mode
/// - Accelerated mode: due cards first, then unreviewed today, then practice mode
fn get_next_study_card(conn: &std::sync::MutexGuard<'_, rusqlite::Connection>, exclude_sibling_of: Option<i64>) -> Option<Card> {
  let use_interleaving = db::get_use_interleaving(conn).unwrap_or(true);
  let accelerated = db::get_all_tiers_unlocked(conn).unwrap_or(false);

  // Step 1: Try to get due cards first (both modes)
  let due_cards = if use_interleaving {
    db::get_due_cards_interleaved(conn, 1, exclude_sibling_of).unwrap_or_default()
  } else {
    db::get_due_cards(conn, 1, exclude_sibling_of).unwrap_or_default()
  };

  if let Some(card) = due_cards.into_iter().next() {
    return Some(card);
  }

  // Step 2: In accelerated mode, try unreviewed cards (not reviewed today)
  if accelerated {
    let unreviewed = db::get_unreviewed_today(conn, 1, exclude_sibling_of).unwrap_or_default();
    if let Some(card) = unreviewed.into_iter().next() {
      return Some(card);
    }
  }

  // Step 3: No cards available - will show "all done" / practice mode
  None
}

/// Interactive study mode with input-based validation
pub async fn study_start_interactive(State(pool): State<DbPool>) -> impl IntoResponse {
  let conn = pool.lock().unwrap();

  if let Some(card) = get_next_study_card(&conn, None) {
    let hint_gen = HintGenerator::new(&card.main_answer, card.description.as_deref());

    // Check if answer is Korean (needs multiple choice)
    let is_multiple_choice = is_korean(&card.main_answer);
    let choices = if is_multiple_choice {
      // Get all cards from this tier for generating choices
      let all_cards = db::get_cards_by_tier(&conn, card.tier).unwrap_or_default();
      generate_choices(&card, &all_cards)
    } else {
      vec![]
    };

    let template = StudyInteractiveTemplate {
      card_id: card.id,
      front: card.front.clone(),
      main_answer: card.main_answer.clone(),
      description: card.description.clone(),
      tier: card.tier,
      validated: false,
      is_correct: false,
      user_answer: String::new(),
      quality: 0,
      hints_used: 0,
      hint_1: hint_gen.hint_level_1(),
      hint_2: hint_gen.hint_level_2(),
      hint_final: hint_gen.hint_final(),
      is_multiple_choice,
      choices,
      has_card: true,
    };
    Html(template.render().unwrap_or_default())
  } else {
    let template = StudyInteractiveTemplate {
      card_id: 0,
      front: String::new(),
      main_answer: String::new(),
      description: None,
      tier: 0,
      validated: false,
      is_correct: false,
      user_answer: String::new(),
      quality: 0,
      hints_used: 0,
      hint_1: String::new(),
      hint_2: String::new(),
      hint_final: String::new(),
      is_multiple_choice: false,
      choices: vec![],
      has_card: false,
    };
    Html(template.render().unwrap_or_default())
  }
}

#[derive(Deserialize)]
pub struct ValidateAnswerForm {
  pub card_id: i64,
  pub answer: String,
  pub hints_used: u8,
}

/// Validate user's typed answer
pub async fn validate_answer_handler(
  State(pool): State<DbPool>,
  Form(form): Form<ValidateAnswerForm>,
) -> impl IntoResponse {
  let conn = pool.lock().unwrap();

  if let Ok(Some(card)) = db::get_card_by_id(&conn, form.card_id) {
    let result = validate_answer(&form.answer, &card.main_answer);
    let is_correct = result.is_correct();
    let quality = result.to_quality(form.hints_used > 0);

    // Record confusion if incorrect
    if !is_correct && !form.answer.trim().is_empty() {
      let _ = db::record_confusion(&conn, card.id, &form.answer);
    }

    let hint_gen = HintGenerator::new(&card.main_answer, card.description.as_deref());

    // Check if answer is Korean (was multiple choice)
    let is_multiple_choice = is_korean(&card.main_answer);

    let template = InteractiveCardTemplate {
      card_id: card.id,
      front: card.front.clone(),
      main_answer: card.main_answer.clone(),
      description: card.description.clone(),
      tier: card.tier,
      validated: true,
      is_correct,
      user_answer: form.answer,
      quality,
      hints_used: form.hints_used,
      hint_1: hint_gen.hint_level_1(),
      hint_2: hint_gen.hint_level_2(),
      hint_final: hint_gen.hint_final(),
      is_multiple_choice,
      choices: vec![], // Not needed after validation
    };
    Html(template.render().unwrap_or_default())
  } else {
    let template = NoCardsTemplate {};
    Html(template.render().unwrap_or_default())
  }
}

/// Get next interactive card after submitting review
pub async fn submit_review_interactive(
  State(pool): State<DbPool>,
  Form(form): Form<ReviewForm>,
) -> impl IntoResponse {
  let conn = pool.lock().unwrap();

  // Process the review (same as regular submit_review)
  if let Ok(Some(card)) = db::get_card_by_id(&conn, form.card_id) {
    // Check if FSRS is enabled
    let use_fsrs = db::get_use_fsrs(&conn).unwrap_or(true);

    if use_fsrs {
      // Use FSRS scheduling
      let desired_retention = db::get_desired_retention(&conn).unwrap_or(0.9);
      let result = srs::calculate_fsrs_review(&card, form.quality, desired_retention);

      let _ = db::update_card_after_fsrs_review(
        &conn,
        card.id,
        result.next_review,
        result.stability,
        result.difficulty,
        result.state,
        form.quality >= 2,
      );
    } else {
      // Use SM-2 scheduling (fallback)
      let result = srs::calculate_review(
        form.quality,
        card.ease_factor,
        card.interval_days,
        card.repetitions,
        card.learning_step,
      );

      let correct = ReviewQuality::from_u8(form.quality)
        .map(|q| q.is_correct())
        .unwrap_or(false);

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
    }

    // Log review
    let log = ReviewLog::new(card.id, form.quality);
    let _ = db::insert_review_log(&conn, &log);
  }

  // Get next card using FSA logic
  if let Some(next_card) = get_next_study_card(&conn, Some(form.card_id)) {
    let hint_gen = HintGenerator::new(&next_card.main_answer, next_card.description.as_deref());

    // Check if answer is Korean (needs multiple choice)
    let is_multiple_choice = is_korean(&next_card.main_answer);
    let choices = if is_multiple_choice {
      let all_cards = db::get_cards_by_tier(&conn, next_card.tier).unwrap_or_default();
      generate_choices(&next_card, &all_cards)
    } else {
      vec![]
    };

    let template = InteractiveCardTemplate {
      card_id: next_card.id,
      front: next_card.front.clone(),
      main_answer: next_card.main_answer.clone(),
      description: next_card.description.clone(),
      tier: next_card.tier,
      validated: false,
      is_correct: false,
      user_answer: String::new(),
      quality: 0,
      hints_used: 0,
      hint_1: hint_gen.hint_level_1(),
      hint_2: hint_gen.hint_level_2(),
      hint_final: hint_gen.hint_final(),
      is_multiple_choice,
      choices,
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
