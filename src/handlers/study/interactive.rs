//! Interactive study mode with input-based validation.

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use chrono::Utc;
use rusqlite::params;
use serde::Deserialize;

use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::domain::{Card, ReviewDirection, StudyMode};
use crate::handlers::NavContext;
use crate::session;
use crate::srs::{self, select_next_card};
use crate::state::AppState;
use crate::validation::{validate_answer, AnswerResult, HintGenerator};

#[cfg(feature = "profiling")]
use crate::profiling::EventType;

use super::templates::{
  InteractiveCardTemplate, NextCardForm, NoCardsTemplate, ReviewForm, StudyFilterOption,
  StudyInteractiveTemplate, ValidateAnswerForm,
};
use super::{
  generate_choices, get_available_study_cards, get_character_type, get_review_direction,
  get_tracked_character, is_korean,
};

/// Interactive study mode with input-based validation
pub async fn study_start_interactive(
  State(state): State<AppState>,
  auth: AuthContext,
) -> Response {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/study".into(),
    method: "GET".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()).into_response()
    }
  };

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()).into_response()
    }
  };

  // Build study filter options from enabled packs
  let current_filter = db::get_setting(&conn, "study_filter_mode")
    .ok()
    .flatten()
    .unwrap_or_else(|| "all".to_string());

  let mut study_filters = vec![
    StudyFilterOption {
      id: "all".to_string(),
      label: "All Content".to_string(),
      is_selected: current_filter == "all",
      group: None,
      is_lesson: false,
    },
    StudyFilterOption {
      id: "hangul".to_string(),
      label: "Hangul Only".to_string(),
      is_selected: current_filter == "hangul",
      group: None,
      is_lesson: false,
    },
  ];

  // Add filter options for packs the user has permission to access
  // For global packs: permission = access (no need to "enable" in settings)
  if let Ok(packs) = db::get_all_packs_with_lessons(&app_conn) {
    for pack in packs {
      // Skip packs the user doesn't have permission to access
      if !crate::auth::db::can_user_access_pack(&app_conn, auth.user_id, &pack.pack_id)
        .unwrap_or(false)
      {
        continue;
      }
      let pack_label = pack.study_filter_label.clone().unwrap_or(pack.display_name.clone());
      let filter_id = format!("pack:{}", pack.pack_id);

      // Add "All Lessons" option for the pack
      study_filters.push(StudyFilterOption {
        is_selected: current_filter == filter_id,
        id: filter_id,
        label: format!("{} (All)", pack_label),
        group: Some(pack_label.clone()),
        is_lesson: false,
      });

      // Add individual lesson options (only show unlocked lessons)
      if let Some(total_lessons) = pack.total_lessons {
        for lesson_num in 1..=total_lessons {
          // Only show lessons the user has unlocked
          if !db::is_lesson_unlocked(&conn, &pack.pack_id, lesson_num).unwrap_or(false) {
            continue;
          }

          let lesson_filter_id = format!("pack:{}:lesson:{}", pack.pack_id, lesson_num);
          let prefix = if pack.section_prefix.is_empty() { "Lesson" } else { &pack.section_prefix };
          let lesson_label = pack.lesson_labels
            .as_ref()
            .and_then(|labels| labels.get(&lesson_num.to_string()).cloned())
            .map(|label| format!("L{}: {}", lesson_num, label))
            .unwrap_or_else(|| format!("{} {}", prefix, lesson_num));

          study_filters.push(StudyFilterOption {
            is_selected: current_filter == lesson_filter_id,
            id: lesson_filter_id,
            label: lesson_label,
            group: Some(pack_label.clone()),
            is_lesson: true,
          });
        }
      }
    }
  }

  // Check if focus mode is enabled (simple boolean)
  let focus_mode_active = db::is_focus_mode_enabled(&conn).log_warn_default("Failed to get focus mode");

  // Generate a new session ID for this study session
  let session_id = session::generate_session_id();
  let mut study_session = session::get_session(&session_id);

  // Get session stats for display (using current filter mode)
  let filter_mode = super::parse_filter_mode(&current_filter);
  let stats = db::get_session_stats(&conn, &app_conn, auth.user_id, &filter_mode).unwrap_or_default();

  // Get available cards using existing logic
  let available_cards = get_available_study_cards(&conn, &app_conn, auth.user_id);

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

  if let Some(card_id) = selected_card_id
    && let Ok(Some(card)) = db::get_card_by_id(&conn, card_id) {
      let hint_gen = HintGenerator::new(&card.main_answer, card.description.as_deref());

      // Check if answer is Korean (needs multiple choice)
      let is_multiple_choice = is_korean(&card.main_answer);
      let choices = if is_multiple_choice {
        // For vocabulary cards, get choices from same lesson; for Hangul, from same tier
        let all_cards = if let (Some(pack_id), Some(lesson)) = (&card.pack_id, card.lesson) {
          db::get_cards_from_same_lesson(&conn, pack_id, lesson)
            .log_warn_default("Failed to get lesson cards for choices")
        } else {
          db::get_cards_by_tier(&conn, card.tier)
            .log_warn_default("Failed to get tier cards for choices")
        };
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
        is_vocabulary: card.pack_id.is_some(),
        validated: false,
        is_correct: false,
        is_partial: false,
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
        study_filters: study_filters.clone(),
        current_filter: current_filter.clone(),
        stats,
        is_htmx_partial: false,
        nav: NavContext::from_auth(&auth),
      };
      return Html(template.render().unwrap_or_default()).into_response();
    }

  // No cards available
  let template = StudyInteractiveTemplate {
    card_id: 0,
    front: String::new(),
    main_answer: String::new(),
    description: None,
    tier: 0,
    is_reverse: false,
    is_vocabulary: false,
    validated: false,
    is_correct: false,
    is_partial: false,
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
    #[cfg(feature = "testing")]
    testing_mode: true,
    #[cfg(not(feature = "testing"))]
    testing_mode: false,
    focus_mode_active,
    study_filters,
    current_filter,
    stats,
    is_htmx_partial: false,
    nav: NavContext::from_auth(&auth),
  };
  Html(template.render().unwrap_or_default()).into_response()
}

/// Validate user's typed answer and record the review result
pub async fn validate_answer_handler(
  State(state): State<AppState>,
  auth: AuthContext,
  Form(form): Form<ValidateAnswerForm>,
) -> Response {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/validate-answer".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()).into_response()
    }
  };

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()).into_response()
    }
  };

  if let Ok(Some(card)) = db::get_card_by_id(&conn, form.card_id) {
    // Use strict or fuzzy matching based on input method
    let (is_correct, is_partial, quality) = if form.input_method.is_strict() {
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
      (correct, false, q)
    } else {
      // Text input: fuzzy matching allows typos
      let result = validate_answer(&form.answer, &card.main_answer);
      let is_partial = matches!(result, AnswerResult::PartialMatch);
      (result.is_correct(), is_partial, result.to_quality(form.hints_used > 0))
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
    if let Err(e) = db::insert_review_log_enhanced(
      &conn,
      card.id,
      quality,
      is_correct,
      StudyMode::Interactive,
      direction,
      None,
      form.hints_used.into(),
    ) {
      tracing::error!("Failed to insert review log for card {}: {}", card.id, e);
    }

    // Update character stats
    let tracked_char = get_tracked_character(&card);
    let char_type = get_character_type(&card);
    let _ = db::update_character_stats(&conn, tracked_char, char_type, is_correct);

    // Save session state
    session::update_session(&session_id, study_session);

    let hint_gen = HintGenerator::new(&card.main_answer, card.description.as_deref());

    // Check if answer is Korean (for template display purposes)
    let is_multiple_choice = is_korean(&card.main_answer);

    // Get updated session stats for OOB swap
    let current_filter = db::get_setting(&conn, "study_filter_mode")
      .ok()
      .flatten()
      .unwrap_or_else(|| "all".to_string());
    let filter_mode = super::parse_filter_mode(&current_filter);
    let stats = db::get_session_stats(&conn, &app_conn, auth.user_id, &filter_mode).unwrap_or_default();

    let template = InteractiveCardTemplate {
      card_id: card.id,
      front: card.front.clone(),
      main_answer: card.main_answer.clone(),
      description: card.description.clone(),
      tier: card.tier,
      is_reverse: card.is_reverse,
      is_vocabulary: card.pack_id.is_some(),
      validated: true,
      is_correct,
      is_partial,
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
      stats,
      is_htmx_partial: true,
    };
    Html(template.render().unwrap_or_default()).into_response()
  } else {
    let template = NoCardsTemplate { nav: NavContext::from_auth(&auth) };
    Html(template.render().unwrap_or_default()).into_response()
  }
}

/// Get next interactive card (review was already recorded during validation)
pub async fn next_card_interactive(
  State(state): State<AppState>,
  auth: AuthContext,
  Form(form): Form<NextCardForm>,
) -> Response {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/next-card".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()).into_response()
    }
  };

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()).into_response()
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
  let available_cards = get_available_study_cards(&conn, &app_conn, auth.user_id);

  let selected_card_id = if !available_cards.is_empty() {
    select_next_card(&conn, &mut study_session, &available_cards)
      .ok()
      .flatten()
  } else {
    None
  };

  // Save session state
  session::update_session(&session_id, study_session);

  if let Some(card_id) = selected_card_id
    && let Ok(Some(next_card)) = db::get_card_by_id(&conn, card_id) {
      let hint_gen = HintGenerator::new(&next_card.main_answer, next_card.description.as_deref());

      // Check if answer is Korean (needs multiple choice)
      let is_multiple_choice = is_korean(&next_card.main_answer);
      let choices = if is_multiple_choice {
        // For vocabulary cards, get choices from same lesson; for Hangul, from same tier
        let all_cards = if let (Some(pack_id), Some(lesson)) = (&next_card.pack_id, next_card.lesson) {
          db::get_cards_from_same_lesson(&conn, pack_id, lesson)
            .log_warn_default("Failed to get lesson cards for choices")
        } else {
          db::get_cards_by_tier(&conn, next_card.tier)
            .log_warn_default("Failed to get tier cards for choices")
        };
        generate_choices(&next_card, &all_cards)
      } else {
        vec![]
      };

      // Get updated session stats for OOB swap
      let current_filter = db::get_setting(&conn, "study_filter_mode")
        .ok()
        .flatten()
        .unwrap_or_else(|| "all".to_string());
      let filter_mode = super::parse_filter_mode(&current_filter);
      let stats = db::get_session_stats(&conn, &app_conn, auth.user_id, &filter_mode).unwrap_or_default();

      let template = InteractiveCardTemplate {
        card_id: next_card.id,
        front: next_card.front.clone(),
        main_answer: next_card.main_answer.clone(),
        description: next_card.description.clone(),
        tier: next_card.tier,
        is_reverse: next_card.is_reverse,
        is_vocabulary: next_card.pack_id.is_some(),
        validated: false,
        is_correct: false,
        is_partial: false,
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
        stats,
        is_htmx_partial: true,
      };
      return Html(template.render().unwrap_or_default()).into_response();
    }

  // Check if a new tier was unlocked - if so, redirect to home to show notification
  if db::try_auto_unlock_tier(&conn).log_warn("Auto tier unlock failed").flatten().is_some() {
    return Redirect::to("/").into_response();
  }

  // Check if any pack lessons were unlocked
  let _ = db::try_auto_unlock_all_pack_lessons(&conn, &app_conn)
    .log_warn("Auto lesson unlock failed");

  let template = NoCardsTemplate { nav: NavContext::from_auth(&auth) };
  Html(template.render().unwrap_or_default()).into_response()
}

/// Get next interactive card after submitting review
/// DEPRECATED: Review recording now happens in validate_answer_handler.
/// Use next_card_interactive instead. Kept for backwards compatibility.
pub async fn submit_review_interactive(
  State(state): State<AppState>,
  auth: AuthContext,
  Form(form): Form<ReviewForm>,
) -> Response {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/review".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()).into_response()
    }
  };

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()).into_response()
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
  let available_cards = get_available_study_cards(&conn, &app_conn, auth.user_id);

  let selected_card_id = if !available_cards.is_empty() {
    select_next_card(&conn, &mut study_session, &available_cards)
      .ok()
      .flatten()
  } else {
    None
  };

  // Save session state
  session::update_session(&session_id, study_session);

  if let Some(card_id) = selected_card_id
    && let Ok(Some(next_card)) = db::get_card_by_id(&conn, card_id) {
      let hint_gen = HintGenerator::new(&next_card.main_answer, next_card.description.as_deref());

      // Check if answer is Korean (needs multiple choice)
      let is_multiple_choice = is_korean(&next_card.main_answer);
      let choices = if is_multiple_choice {
        // For vocabulary cards, get choices from same lesson; for Hangul, from same tier
        let all_cards = if let (Some(pack_id), Some(lesson)) = (&next_card.pack_id, next_card.lesson) {
          db::get_cards_from_same_lesson(&conn, pack_id, lesson)
            .log_warn_default("Failed to get lesson cards for choices")
        } else {
          db::get_cards_by_tier(&conn, next_card.tier)
            .log_warn_default("Failed to get tier cards for choices")
        };
        generate_choices(&next_card, &all_cards)
      } else {
        vec![]
      };

      // Get updated session stats for OOB swap
      let current_filter = db::get_setting(&conn, "study_filter_mode")
        .ok()
        .flatten()
        .unwrap_or_else(|| "all".to_string());
      let filter_mode = super::parse_filter_mode(&current_filter);
      let stats = db::get_session_stats(&conn, &app_conn, auth.user_id, &filter_mode).unwrap_or_default();

      let template = InteractiveCardTemplate {
        card_id: next_card.id,
        front: next_card.front.clone(),
        main_answer: next_card.main_answer.clone(),
        description: next_card.description.clone(),
        tier: next_card.tier,
        is_reverse: next_card.is_reverse,
        is_vocabulary: next_card.pack_id.is_some(),
        validated: false,
        is_correct: false,
        is_partial: false,
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
        stats,
        is_htmx_partial: true,
      };
      return Html(template.render().unwrap_or_default()).into_response();
    }

  // Check if a new tier was unlocked - if so, redirect to home to show notification
  if db::try_auto_unlock_tier(&conn).log_warn("Auto tier unlock failed").flatten().is_some() {
    return Redirect::to("/").into_response();
  }

  let template = NoCardsTemplate { nav: NavContext::from_auth(&auth) };
  Html(template.render().unwrap_or_default()).into_response()
}

/// Form for changing study filter
#[derive(Deserialize)]
pub struct StudyFilterForm {
  pub filter: String,
}

/// Change the study filter mode
pub async fn set_study_filter(
  State(state): State<AppState>,
  auth: AuthContext,
  Form(form): Form<StudyFilterForm>,
) -> Redirect {
  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/study"),
  };

  // Validate filter value: "all", "hangul", "pack:<id>", or "pack:<id>:lesson:<n>"
  let filter = form.filter.trim();
  if filter == "all" || filter == "hangul" {
    let _ = db::set_setting(&conn, "study_filter_mode", filter);
  } else if let Some(rest) = filter.strip_prefix("pack:") {
    let app_conn = match state.auth_db.lock() {
      Ok(conn) => conn,
      Err(_) => return Redirect::to("/study"),
    };

    // Check if this is a lesson-specific filter: pack:X:lesson:N
    let (pack_id, lesson_num) = if let Some((pack_part, lesson_part)) = rest.split_once(":lesson:") {
      let lesson: Option<u8> = lesson_part.parse().ok();
      (pack_part, lesson)
    } else {
      (rest, None)
    };

    // Validate pack_id exists in content_packs
    let pack_exists: bool = app_conn
      .query_row(
        "SELECT 1 FROM content_packs WHERE id = ?1",
        rusqlite::params![pack_id],
        |_| Ok(true),
      )
      .unwrap_or(false);

    if pack_exists {
      // Store the filter - includes lesson number if specified
      let _ = db::set_setting(&conn, "study_filter_mode", filter);

      // Also update the structured settings for compatibility with StudyFilterMode
      let _ = db::set_setting(&conn, "study_filter_pack", pack_id);
      if let Some(lesson) = lesson_num {
        let _ = db::set_setting(&conn, "study_filter_lessons", &lesson.to_string());
      } else {
        let _ = db::set_setting(&conn, "study_filter_lessons", "");
      }
    }
    // If pack doesn't exist, silently ignore (don't store invalid pack_id)
  }

  Redirect::to("/study")
}

/// Form for toggling focus mode
#[derive(Deserialize)]
pub struct FocusModeForm {
  pub enabled: bool,
}

/// Toggle focus mode on/off
pub async fn toggle_focus_mode(
  auth: AuthContext,
  Form(form): Form<FocusModeForm>,
) -> Redirect {
  if let Ok(conn) = auth.user_db.lock() {
    let _ = db::set_focus_mode_enabled(&conn, form.enabled);
  }
  Redirect::to("/study")
}

// ============================================================================
// OVERRIDE RULING
// ============================================================================

/// Form for overriding a card ruling
#[derive(Deserialize)]
pub struct OverrideForm {
  pub card_id: i64,
  pub quality: u8,
  #[serde(default)]
  pub suggested_answer: String,
  #[serde(default)]
  pub card_front: String,
  #[serde(default)]
  pub expected_answer: String,
  #[serde(default)]
  pub user_answer: String,
  #[serde(default)]
  pub original_result: String,
  #[serde(default)]
  pub pack_id: String,
}

/// Override the ruling on the last reviewed card
pub async fn override_ruling_handler(
  State(state): State<AppState>,
  auth: AuthContext,
  Form(form): Form<OverrideForm>,
) -> StatusCode {
  // Validate quality is in valid range (security: reject invalid values)
  if ![0, 2, 4, 5].contains(&form.quality) {
    tracing::warn!("Invalid quality score {} from user {}", form.quality, auth.username);
    return StatusCode::BAD_REQUEST;
  }

  // Validate card_id is positive (security: reject obviously invalid IDs)
  if form.card_id <= 0 {
    tracing::warn!("Invalid card_id {} from user {}", form.card_id, auth.username);
    return StatusCode::BAD_REQUEST;
  }

  // Get user's learning.db connection
  let user_conn = match auth.user_db.lock() {
    Ok(c) => c,
    Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
  };

  // Verify the card exists and belongs to this user (security: prevent spoofed card IDs)
  let card = match db::get_card_by_id(&user_conn, form.card_id) {
    Ok(Some(c)) => c,
    Ok(None) => {
      tracing::warn!("Card {} not found for user {}", form.card_id, auth.username);
      return StatusCode::NOT_FOUND;
    }
    Err(e) => {
      tracing::error!("Failed to get card {}: {}", form.card_id, e);
      return StatusCode::INTERNAL_SERVER_ERROR;
    }
  };

  // 1. Record corrective review in user's learning.db
  let is_correct = form.quality >= 2;
  if let Err(e) = db::insert_review_log_enhanced(
    &user_conn,
    form.card_id,
    form.quality,
    is_correct,
    StudyMode::Override,
    ReviewDirection::KrToRom, // Default direction for overrides
    None, // response_time_ms
    0,    // hints_used
  ) {
    tracing::error!("Failed to record override review: {}", e);
    return StatusCode::INTERNAL_SERVER_ERROR;
  }

  // 2. Update card SRS state
  if let Err(e) = update_card_srs_for_override(&user_conn, &card, form.quality) {
    tracing::error!("Failed to update SRS for override: {}", e);
    // Continue anyway - review was logged
  }

  // 3. Save suggestion to app.db for admin review (sanitized)
  if let Ok(app_conn) = state.auth_db.lock() {
    if let Err(e) = save_validation_suggestion(&app_conn, &form, &auth.username) {
      tracing::error!("Failed to save validation suggestion: {}", e);
      // Don't fail the request - the override still worked
    }
  }

  tracing::info!(
    "User {} overrode ruling for card {} with quality {}",
    auth.username,
    form.card_id,
    form.quality
  );

  StatusCode::OK
}

/// Save a validation suggestion to app.db for admin review
/// Input sanitization: truncate long strings to prevent abuse
fn save_validation_suggestion(
  conn: &rusqlite::Connection,
  form: &OverrideForm,
  username: &str,
) -> rusqlite::Result<()> {
  // Truncate strings to reasonable lengths (security: prevent storage abuse)
  fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
      s
    } else {
      // Find a valid UTF-8 boundary
      let mut end = max_len;
      while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
      }
      &s[..end]
    }
  }

  let now = Utc::now().to_rfc3339();
  conn.execute(
    "INSERT INTO validation_suggestions
     (card_id, pack_id, card_front, expected_answer, user_answer,
      suggested_answer, original_result, user_quality, username, created_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    params![
      form.card_id,
      truncate(&form.pack_id, 100),
      truncate(&form.card_front, 500),
      truncate(&form.expected_answer, 500),
      truncate(&form.user_answer, 500),
      truncate(&form.suggested_answer, 500),
      truncate(&form.original_result, 50),
      form.quality,
      truncate(username, 100),
      now,
    ],
  )?;
  Ok(())
}

/// Update card SRS state based on override quality
fn update_card_srs_for_override(
  conn: &rusqlite::Connection,
  card: &Card,
  quality: u8,
) -> rusqlite::Result<()> {
  // Use FSRS or SM-2 based on user setting
  let use_fsrs = db::get_use_fsrs(conn).unwrap_or(true);
  let focus_mode = db::is_focus_mode_active(conn).unwrap_or(false);

  if use_fsrs {
    let desired_retention =
      db::get_desired_retention(conn).log_warn_default("Failed to get desired retention");
    let result = srs::calculate_fsrs_review(card, quality, desired_retention, focus_mode);

    db::update_card_after_fsrs_review(
      conn,
      card.id,
      result.next_review,
      result.stability,
      result.difficulty,
      result.state,
      result.learning_step,
      result.repetitions,
      quality >= 2,
    )?;
  } else {
    let result = srs::sm2::calculate_review(
      quality,
      card.ease_factor,
      card.interval_days,
      card.repetitions,
      card.learning_step,
    );

    db::update_card_after_review(
      conn,
      card.id,
      result.ease_factor,
      result.interval_days,
      result.learning_step,
      result.next_review,
      result.repetitions,
      quality >= 2,
    )?;
  }

  Ok(())
}
