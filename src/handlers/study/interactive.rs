//! Interactive study mode with input-based validation.

use askama::Template;
use axum::response::{Html, IntoResponse};
use axum::Form;

use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::domain::StudyMode;
use crate::session;
use crate::srs::{self, select_next_card};
use crate::validation::{validate_answer, HintGenerator};

#[cfg(feature = "profiling")]
use crate::profiling::EventType;

use super::templates::{
  InteractiveCardTemplate, NextCardForm, NoCardsTemplate, ReviewForm, StudyInteractiveTemplate,
  ValidateAnswerForm,
};
use super::{
  generate_choices, get_available_study_cards, get_character_type, get_review_direction,
  get_tracked_character, is_korean,
};

/// Interactive study mode with input-based validation
pub async fn study_start_interactive(auth: AuthContext) -> impl IntoResponse {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/study".into(),
    method: "GET".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  // Check focus mode status for exit recommendation
  let focus_tier = db::get_focus_tier(&conn).log_warn_default("Failed to get focus tier");
  let (focus_mode_active, focus_tier_num, focus_tier_progress, show_exit_focus_recommendation) =
    if let Some(tier) = focus_tier {
      let tiers = db::get_progress_by_tier(&conn).log_warn_default("Failed to get tier progress");
      let progress = tiers
        .iter()
        .find(|t| t.tier == tier)
        .map(|t| t.percentage())
        .unwrap_or(0);
      // Recommend exiting focus mode when tier reaches 50% learned
      let show_recommendation = progress >= 50;
      (true, tier, progress, show_recommendation)
    } else {
      (false, 0, 0, false)
    };

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
        let all_cards = db::get_cards_by_tier(&conn, card.tier)
          .log_warn_default("Failed to get tier cards for choices");
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
        is_reverse: card.is_reverse,
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
        is_tracked: true,
        track_progress: false,
        #[cfg(feature = "testing")]
        testing_mode: true,
        #[cfg(not(feature = "testing"))]
        testing_mode: false,
        focus_mode_active,
        focus_tier: focus_tier_num,
        focus_tier_progress,
        show_exit_focus_recommendation,
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
    is_reverse: false,
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
    is_tracked: true,
    track_progress: false,
    focus_mode_active,
    focus_tier: focus_tier_num,
    focus_tier_progress,
    show_exit_focus_recommendation,
    #[cfg(feature = "testing")]
    testing_mode: true,
    #[cfg(not(feature = "testing"))]
    testing_mode: false,
  };
  Html(template.render().unwrap_or_default())
}

/// Validate user's typed answer and record the review result
pub async fn validate_answer_handler(
  auth: AuthContext,
  Form(form): Form<ValidateAnswerForm>,
) -> impl IntoResponse {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/validate-answer".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  if let Ok(Some(card)) = db::get_card_by_id(&conn, form.card_id) {
    // Use strict or fuzzy matching based on input method
    let (is_correct, quality) = if form.input_method.is_strict() {
      // Multiple choice: exact match only
      let correct = form.answer == card.main_answer;
      let q = if correct {
        if form.hints_used > 0 {
          2
        } else {
          4
        } // Hard or Good
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
      username: auth.username.clone(),
    });

    // Record confusion if incorrect
    if !is_correct && !form.answer.trim().is_empty() {
      let _ = db::record_confusion(&conn, card.id, &form.answer);
    }

    // --- Record the review result immediately ---
    // Update session reinforcement queue
    let session_id = if form.session_id.is_empty() {
      session::generate_session_id()
    } else {
      form.session_id.clone()
    };
    let mut study_session = session::get_session(&session_id);

    if is_correct {
      study_session.remove_from_reinforcement(card.id);
    } else {
      study_session.add_failed_card(card.id);
    }

    // Update SRS based on FSRS or SM-2
    let use_fsrs = db::get_use_fsrs(&conn).log_warn_default("Failed to get FSRS setting");
    let focus_mode = db::is_focus_mode_active(&conn).unwrap_or(false);

    if use_fsrs {
      let desired_retention =
        db::get_desired_retention(&conn).log_warn_default("Failed to get desired retention");
      let result = srs::calculate_fsrs_review(&card, quality, desired_retention, focus_mode);

      #[cfg(feature = "profiling")]
      crate::profile_log!(EventType::SrsCalculation {
        algorithm: "fsrs_hybrid".into(),
        card_id: card.id,
        rating: quality,
        username: auth.username.clone(),
      });

      let _ = db::update_card_after_fsrs_review(
        &conn,
        card.id,
        result.next_review,
        result.stability,
        result.difficulty,
        result.state,
        result.learning_step,
        result.repetitions,
        is_correct,
      );
    } else {
      let result = srs::calculate_review(
        quality,
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
        is_correct,
      );
    }

    // Log review with enhanced tracking
    let direction = get_review_direction(&card);
    let _ = db::insert_review_log_enhanced(
      &conn,
      card.id,
      quality,
      is_correct,
      StudyMode::Interactive,
      direction,
      None,
      form.hints_used.into(),
    );

    // Update character stats
    let tracked_char = get_tracked_character(&card);
    let char_type = get_character_type(&card);
    let _ = db::update_character_stats(&conn, tracked_char, char_type, is_correct);

    // Save session state
    session::update_session(&session_id, study_session);

    let hint_gen = HintGenerator::new(&card.main_answer, card.description.as_deref());

    // Check if answer is Korean (for template display purposes)
    let is_multiple_choice = is_korean(&card.main_answer);

    let template = InteractiveCardTemplate {
      card_id: card.id,
      front: card.front.clone(),
      main_answer: card.main_answer.clone(),
      description: card.description.clone(),
      tier: card.tier,
      is_reverse: card.is_reverse,
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
      session_id,
      is_tracked: true,
      track_progress: false,
    };
    Html(template.render().unwrap_or_default())
  } else {
    let template = NoCardsTemplate {};
    Html(template.render().unwrap_or_default())
  }
}

/// Get next interactive card (review was already recorded during validation)
pub async fn next_card_interactive(
  auth: AuthContext,
  Form(form): Form<NextCardForm>,
) -> impl IntoResponse {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/next-card".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  // Get or create session
  let session_id = if form.session_id.is_empty() {
    session::generate_session_id()
  } else {
    form.session_id.clone()
  };
  let mut study_session = session::get_session(&session_id);

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
        let all_cards = db::get_cards_by_tier(&conn, next_card.tier)
          .log_warn_default("Failed to get tier cards for choices");
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
        is_reverse: next_card.is_reverse,
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
        is_tracked: true,
        track_progress: false,
      };
      return Html(template.render().unwrap_or_default());
    }
  }

  let template = NoCardsTemplate {};
  Html(template.render().unwrap_or_default())
}

/// Get next interactive card after submitting review
/// DEPRECATED: Review recording now happens in validate_answer_handler.
/// Use next_card_interactive instead. Kept for backwards compatibility.
pub async fn submit_review_interactive(
  auth: AuthContext,
  Form(form): Form<ReviewForm>,
) -> impl IntoResponse {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/review".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  // Get or create session
  let session_id = if form.session_id.is_empty() {
    session::generate_session_id()
  } else {
    form.session_id.clone()
  };
  let mut study_session = session::get_session(&session_id);

  // NOTE: Review is now recorded during validation, so we skip the SRS update here.
  // This handler is kept for backwards compatibility but only fetches next card.

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
        let all_cards = db::get_cards_by_tier(&conn, next_card.tier)
          .log_warn_default("Failed to get tier cards for choices");
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
        is_reverse: next_card.is_reverse,
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
        is_tracked: true,
        track_progress: false,
      };
      return Html(template.render().unwrap_or_default());
    }
  }

  let template = NoCardsTemplate {};
  Html(template.render().unwrap_or_default())
}
