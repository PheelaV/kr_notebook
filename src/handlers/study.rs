use askama::Template;
use axum::{
  extract::{Query, State},
  response::{Html, IntoResponse},
  Form,
};
use rand::seq::SliceRandom;
use serde::Deserialize;

use crate::db::{self, DbPool};
use crate::domain::{Card, InputMethod, ReviewDirection, ReviewQuality, StudyMode};
use crate::session;
use crate::srs::{self, select_next_card};
use crate::validation::{validate_answer, HintGenerator};

/// Determine the review direction based on card front text
fn get_review_direction(card: &Card) -> ReviewDirection {
  if card.front.starts_with("Which letter sounds like") {
    // Question asking for Korean character from romanization
    ReviewDirection::RomToKr
  } else if is_korean(&card.front) {
    // Korean character shown, asking for romanization
    ReviewDirection::KrToRom
  } else {
    // Default to Korean to romanization
    ReviewDirection::KrToRom
  }
}

/// Get character type string for stats tracking
fn get_character_type(card: &Card) -> &'static str {
  card.card_type.as_str()
}

/// Get the character to track stats for (the Korean character being learned)
fn get_tracked_character(card: &Card) -> &str {
  if is_korean(&card.front) {
    // Front is Korean, track it
    &card.front
  } else if is_korean(&card.main_answer) {
    // Answer is Korean (e.g., "Which letter sounds like 'g'?" -> "ㄱ")
    &card.main_answer
  } else {
    // Fallback to front
    &card.front
  }
}
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

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
  // Session tracking
  pub session_id: String,
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
  // Session tracking
  pub session_id: String,
  // Testing mode flag
  pub testing_mode: bool,
}

#[derive(Template)]
#[template(path = "practice.html")]
pub struct PracticeTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub mode: String,
  // Interactive mode fields
  pub validated: bool,
  pub is_correct: bool,
  pub user_answer: String,
  pub is_multiple_choice: bool,
  pub choices: Vec<String>,
  // Progress tracking
  pub track_progress: bool,
}

#[derive(Template)]
#[template(path = "practice_interactive_card.html")]
pub struct PracticeInteractiveCardTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub validated: bool,
  pub is_correct: bool,
  pub user_answer: String,
  pub is_multiple_choice: bool,
  pub choices: Vec<String>,
  // Progress tracking
  pub track_progress: bool,
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
  #[serde(default)]
  pub session_id: String,
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

/// Interactive study mode with input-based validation
pub async fn study_start_interactive(State(pool): State<DbPool>) -> impl IntoResponse {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/study".into(),
    method: "GET".into(),
  });

  let conn = pool.lock().unwrap();

  // Generate a new session ID for this study session
  let session_id = session::generate_session_id();
  let mut study_session = session::get_session(&session_id);

  // Get available cards using existing logic
  let available_cards = get_available_study_cards(&conn);

  // Use weighted selection
  let selected_card_id = if !available_cards.is_empty() {
    select_next_card(&conn, &mut study_session, &available_cards)
      .ok()
      .flatten()
  } else {
    None
  };

  // Save session state
  session::update_session(&session_id, study_session);

  if let Some(card_id) = selected_card_id {
    if let Ok(Some(card)) = db::get_card_by_id(&conn, card_id) {
      let hint_gen = HintGenerator::new(&card.main_answer, card.description.as_deref());

      // Check if answer is Korean (needs multiple choice)
      let is_multiple_choice = is_korean(&card.main_answer);
      let choices = if is_multiple_choice {
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
        session_id,
        #[cfg(feature = "testing")]
        testing_mode: true,
        #[cfg(not(feature = "testing"))]
        testing_mode: false,
      };
      return Html(template.render().unwrap_or_default());
    }
  }

  // No cards available
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
    session_id,
    #[cfg(feature = "testing")]
    testing_mode: true,
    #[cfg(not(feature = "testing"))]
    testing_mode: false,
  };
  Html(template.render().unwrap_or_default())
}

/// Get all available cards for study (due + unreviewed in accelerated mode)
fn get_available_study_cards(conn: &std::sync::MutexGuard<'_, rusqlite::Connection>) -> Vec<Card> {
  let use_interleaving = db::get_use_interleaving(conn).unwrap_or(true);
  let accelerated = db::get_all_tiers_unlocked(conn).unwrap_or(false);

  let mut cards = Vec::new();

  // Get due cards
  let due = if use_interleaving {
    db::get_due_cards_interleaved(conn, 50, None).unwrap_or_default()
  } else {
    db::get_due_cards(conn, 50, None).unwrap_or_default()
  };
  cards.extend(due);

  // In accelerated mode, also get unreviewed cards
  if accelerated {
    let unreviewed = db::get_unreviewed_today(conn, 50, None).unwrap_or_default();
    // Avoid duplicates
    for card in unreviewed {
      if !cards.iter().any(|c| c.id == card.id) {
        cards.push(card);
      }
    }
  }

  cards
}

#[derive(Deserialize)]
pub struct ValidateAnswerForm {
  pub card_id: i64,
  pub answer: String,
  pub hints_used: u8,
  #[serde(default)]
  pub session_id: String,
  #[serde(default)]
  pub input_method: InputMethod,
}

/// Validate user's typed answer
pub async fn validate_answer_handler(
  State(pool): State<DbPool>,
  Form(form): Form<ValidateAnswerForm>,
) -> impl IntoResponse {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/validate-answer".into(),
    method: "POST".into(),
  });

  let conn = pool.lock().unwrap();

  if let Ok(Some(card)) = db::get_card_by_id(&conn, form.card_id) {
    // Use strict or fuzzy matching based on input method
    let (is_correct, quality) = if form.input_method.is_strict() {
      // Multiple choice: exact match only
      let correct = form.answer == card.main_answer;
      let q = if correct {
        if form.hints_used > 0 { 2 } else { 4 } // Hard or Good
      } else {
        0 // Again
      };
      (correct, q)
    } else {
      // Text input: fuzzy matching allows typos
      let result = validate_answer(&form.answer, &card.main_answer);
      (result.is_correct(), result.to_quality(form.hints_used > 0))
    };

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::AnswerValidation {
      card_id: card.id,
      is_correct,
      hints_used: Some(form.hints_used),
    });

    // Record confusion if incorrect
    if !is_correct && !form.answer.trim().is_empty() {
      let _ = db::record_confusion(&conn, card.id, &form.answer);
    }

    let hint_gen = HintGenerator::new(&card.main_answer, card.description.as_deref());

    // Check if answer is Korean (for template display purposes)
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
      session_id: form.session_id,
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
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/review".into(),
    method: "POST".into(),
  });

  let conn = pool.lock().unwrap();

  // Get or create session
  let session_id = if form.session_id.is_empty() {
    session::generate_session_id()
  } else {
    form.session_id.clone()
  };
  let mut study_session = session::get_session(&session_id);

  // Process the review
  let correct = form.quality >= 2;

  if let Ok(Some(card)) = db::get_card_by_id(&conn, form.card_id) {
    // Update reinforcement queue based on result
    if correct {
      study_session.remove_from_reinforcement(card.id);
    } else {
      study_session.add_failed_card(card.id);
    }

    // Check if FSRS is enabled
    let use_fsrs = db::get_use_fsrs(&conn).unwrap_or(true);

    if use_fsrs {
      // Use FSRS scheduling
      let desired_retention = db::get_desired_retention(&conn).unwrap_or(0.9);
      let result = srs::calculate_fsrs_review(&card, form.quality, desired_retention);

      #[cfg(feature = "profiling")]
      crate::profile_log!(EventType::SrsCalculation {
        algorithm: "fsrs".into(),
        card_id: card.id,
        rating: form.quality,
      });

      let _ = db::update_card_after_fsrs_review(
        &conn,
        card.id,
        result.next_review,
        result.stability,
        result.difficulty,
        result.state,
        correct,
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

    // Log review with enhanced tracking
    let direction = get_review_direction(&card);
    let _ = db::insert_review_log_enhanced(
      &conn,
      card.id,
      form.quality,
      correct,
      StudyMode::Interactive,
      direction,
      None,
      0,
    );

    // Update character stats
    let tracked_char = get_tracked_character(&card);
    let char_type = get_character_type(&card);
    let _ = db::update_character_stats(&conn, tracked_char, char_type, correct);
  }

  // Get available cards and select next using weighted selection
  let available_cards = get_available_study_cards(&conn);

  let selected_card_id = if !available_cards.is_empty() {
    select_next_card(&conn, &mut study_session, &available_cards)
      .ok()
      .flatten()
  } else {
    None
  };

  // Save session state
  session::update_session(&session_id, study_session);

  if let Some(card_id) = selected_card_id {
    if let Ok(Some(next_card)) = db::get_card_by_id(&conn, card_id) {
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
        session_id,
      };
      return Html(template.render().unwrap_or_default());
    }
  }

  let template = NoCardsTemplate {};
  Html(template.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct PracticeQuery {
  pub mode: Option<String>,
  #[serde(default = "default_track_progress")]
  pub track: Option<bool>,
}

fn default_track_progress() -> Option<bool> {
  Some(true) // Default to tracking progress
}

// Practice mode - review cards even when not due
pub async fn practice_start(
  State(pool): State<DbPool>,
  Query(query): Query<PracticeQuery>,
) -> impl IntoResponse {
  let conn = pool.lock().unwrap();
  let cards = db::get_practice_cards(&conn, 1, None).unwrap_or_default();
  let mode = query.mode.unwrap_or_else(|| "flip".to_string());
  let track_progress = query.track.unwrap_or(true);

  if let Some(card) = cards.first() {
    let is_korean = is_korean(&card.main_answer);
    let choices = if is_korean && mode == "interactive" {
      let all_cards = db::get_unlocked_cards(&conn).unwrap_or_default();
      generate_choices(card, &all_cards)
    } else {
      vec![]
    };

    let template = PracticeTemplate {
      card_id: card.id,
      front: card.front.clone(),
      main_answer: card.main_answer.clone(),
      description: card.description.clone(),
      tier: card.tier,
      mode,
      validated: false,
      is_correct: false,
      user_answer: String::new(),
      is_multiple_choice: is_korean,
      choices,
      track_progress,
    };
    Html(template.render().unwrap_or_default())
  } else {
    Html("<p>No cards available for practice.</p>".to_string())
  }
}

#[derive(Deserialize)]
pub struct PracticeForm {
  pub card_id: i64,
  #[serde(default)]
  pub track_progress: bool,
}

pub async fn practice_next(
  State(pool): State<DbPool>,
  Query(query): Query<PracticeQuery>,
  Form(form): Form<PracticeForm>,
) -> impl IntoResponse {
  let conn = pool.lock().unwrap();
  let mode = query.mode.unwrap_or_else(|| "flip".to_string());
  // Use form value if present, otherwise query param, otherwise default true
  let track_progress = if form.track_progress {
    true
  } else {
    query.track.unwrap_or(true)
  };

  // Get next random card, excluding sibling of the just-practiced card
  let cards = db::get_practice_cards(&conn, 1, Some(form.card_id)).unwrap_or_default();

  if let Some(next_card) = cards.first() {
    if mode == "interactive" {
      let is_korean = is_korean(&next_card.main_answer);
      let choices = if is_korean {
        let all_cards = db::get_unlocked_cards(&conn).unwrap_or_default();
        generate_choices(next_card, &all_cards)
      } else {
        vec![]
      };

      let template = PracticeInteractiveCardTemplate {
        card_id: next_card.id,
        front: next_card.front.clone(),
        main_answer: next_card.main_answer.clone(),
        description: next_card.description.clone(),
        tier: next_card.tier,
        validated: false,
        is_correct: false,
        user_answer: String::new(),
        is_multiple_choice: is_korean,
        choices,
        track_progress,
      };
      Html(template.render().unwrap_or_default())
    } else {
      let template = PracticeCardTemplate {
        card_id: next_card.id,
        front: next_card.front.clone(),
        main_answer: next_card.main_answer.clone(),
        description: next_card.description.clone(),
        tier: next_card.tier,
      };
      Html(template.render().unwrap_or_default())
    }
  } else {
    Html("<p>No more cards available.</p>".to_string())
  }
}

#[derive(Deserialize)]
pub struct PracticeValidateForm {
  pub card_id: i64,
  pub answer: String,
  #[serde(default)]
  pub track_progress: bool,
  #[serde(default)]
  pub input_method: InputMethod,
}

/// Validate answer in practice mode (optionally logs to stats)
pub async fn practice_validate(
  State(pool): State<DbPool>,
  Form(form): Form<PracticeValidateForm>,
) -> impl IntoResponse {
  let conn = pool.lock().unwrap();

  let card = match db::get_card_by_id(&conn, form.card_id) {
    Ok(Some(c)) => c,
    _ => return Html("<p>Card not found.</p>".to_string()),
  };

  // Use strict or fuzzy matching based on input method
  let is_correct = if form.input_method.is_strict() {
    // Multiple choice: exact match only
    form.answer == card.main_answer
  } else {
    // Text input: fuzzy matching allows typos
    let result = validate_answer(&form.answer, &card.main_answer);
    matches!(result, crate::validation::AnswerResult::Correct | crate::validation::AnswerResult::CloseEnough)
  };

  // Log to stats if track_progress is enabled
  if form.track_progress {
    let direction = get_review_direction(&card);
    let quality = if is_correct { 4 } else { 0 }; // Good or Again
    let _ = db::insert_review_log_enhanced(
      &conn,
      card.id,
      quality,
      is_correct,
      StudyMode::PracticeInteractive,
      direction,
      None,
      0,
    );

    // Update character stats
    let tracked_char = get_tracked_character(&card);
    let char_type = get_character_type(&card);
    let _ = db::update_character_stats(&conn, tracked_char, char_type, is_correct);
  }

  let is_korean = is_korean(&card.main_answer);
  let choices = if is_korean {
    let all_cards = db::get_unlocked_cards(&conn).unwrap_or_default();
    generate_choices(&card, &all_cards)
  } else {
    vec![]
  };

  let template = PracticeInteractiveCardTemplate {
    card_id: card.id,
    front: card.front.clone(),
    main_answer: card.main_answer.clone(),
    description: card.description.clone(),
    tier: card.tier,
    validated: true,
    is_correct,
    user_answer: form.answer,
    is_multiple_choice: is_korean,
    choices,
    track_progress: form.track_progress,
  };

  Html(template.render().unwrap_or_default())
}
