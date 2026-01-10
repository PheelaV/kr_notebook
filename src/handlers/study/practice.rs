//! Practice mode handlers - review cards even when not due.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse};
use axum::Form;

use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::domain::StudyMode;
use crate::handlers::NavContext;
use crate::state::AppState;
use crate::validation::validate_answer;

use super::templates::{
  InteractiveCardTemplate, PracticeCardTemplate, PracticeForm, PracticeQuery, PracticeTemplate,
  PracticeValidateForm, StudyFilterOption,
};
use super::{generate_choices, get_character_type, get_review_direction, get_tracked_character, is_korean, parse_filter_mode};

/// Build study filter options for practice mode
fn build_study_filters(
  _conn: &std::sync::MutexGuard<'_, rusqlite::Connection>,
  app_conn: &std::sync::MutexGuard<'_, rusqlite::Connection>,
  user_id: i64,
  current_filter: &str,
) -> Vec<StudyFilterOption> {
  let mut filters = vec![
    StudyFilterOption {
      id: "all".to_string(),
      label: "All Content".to_string(),
      is_selected: current_filter == "all",
    },
    StudyFilterOption {
      id: "hangul".to_string(),
      label: "Hangul Only".to_string(),
      is_selected: current_filter == "hangul",
    },
  ];

  // Add filter options for packs the user has permission to access
  // For global packs: permission = access (no need to "enable" in settings)
  if let Ok(packs) = db::get_all_packs_with_lessons(app_conn) {
    for pack in packs {
      // Skip packs the user doesn't have permission to access
      if !crate::auth::db::can_user_access_pack(app_conn, user_id, &pack.pack_id)
        .unwrap_or(false)
      {
        continue;
      }
      let filter_id = format!("pack:{}", pack.pack_id);
      let label = pack.study_filter_label.unwrap_or(pack.display_name);
      filters.push(StudyFilterOption {
        is_selected: current_filter == filter_id,
        id: filter_id,
        label,
      });
    }
  }

  filters
}

// Practice mode - review cards even when not due
pub async fn practice_start(
  State(state): State<AppState>,
  auth: AuthContext,
  Query(query): Query<PracticeQuery>,
) -> impl IntoResponse {
  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  // Get current filter from settings
  let current_filter = db::get_setting(&conn, "study_filter_mode")
    .ok()
    .flatten()
    .unwrap_or_else(|| "all".to_string());
  let filter = parse_filter_mode(&current_filter);

  // Build study filter options
  let study_filters = build_study_filters(&conn, &app_conn, auth.user_id, &current_filter);

  // Get practice cards with filter applied
  let cards = db::get_practice_cards_filtered(&conn, &app_conn, auth.user_id, 1, None, &filter)
    .log_warn_default("Failed to get practice cards");
  let mode = query.mode.unwrap_or_else(|| "flip".to_string());
  let track_progress = query.track.unwrap_or(true);

  if let Some(card) = cards.first() {
    let is_korean = is_korean(&card.main_answer);
    let choices = if is_korean && mode == "interactive" {
      // For vocabulary cards, get choices from same lesson; for Hangul, from unlocked cards
      let all_cards = if let (Some(pack_id), Some(lesson)) = (&card.pack_id, card.lesson) {
        db::get_cards_from_same_lesson(&conn, pack_id, lesson)
          .log_warn_default("Failed to get lesson cards for choices")
      } else {
        db::get_unlocked_cards(&conn).log_warn_default("Failed to get unlocked cards for choices")
      };
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
      is_reverse: card.is_reverse,
      is_vocabulary: card.pack_id.is_some(),
      mode,
      validated: false,
      is_correct: false,
      user_answer: String::new(),
      is_multiple_choice: is_korean,
      choices,
      track_progress,
      study_filters,
      // Unused in practice mode (is_tracked: false)
      quality: 0,
      hints_used: 0,
      hint_1: String::new(),
      hint_2: String::new(),
      hint_final: String::new(),
      session_id: String::new(),
      is_tracked: false,
      nav: NavContext::from_auth(&auth),
    };
    Html(template.render().unwrap_or_default())
  } else {
    Html("<p>No cards available for practice.</p>".to_string())
  }
}

pub async fn practice_next(
  State(state): State<AppState>,
  auth: AuthContext,
  Query(query): Query<PracticeQuery>,
  Form(form): Form<PracticeForm>,
) -> impl IntoResponse {
  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

  let mode = query.mode.unwrap_or_else(|| "flip".to_string());
  // Use form value if present, otherwise query param, otherwise default true
  let track_progress = if form.track_progress {
    true
  } else {
    query.track.unwrap_or(true)
  };

  // Get current filter from settings
  let current_filter = db::get_setting(&conn, "study_filter_mode")
    .ok()
    .flatten()
    .unwrap_or_else(|| "all".to_string());
  let filter = parse_filter_mode(&current_filter);

  // Get next random card, excluding sibling of the just-practiced card
  let cards = db::get_practice_cards_filtered(&conn, &app_conn, auth.user_id, 1, form.card_id, &filter)
    .log_warn_default("Failed to get practice cards");

  if let Some(next_card) = cards.first() {
    if mode == "interactive" {
      let is_korean = is_korean(&next_card.main_answer);
      let choices = if is_korean {
        // For vocabulary cards, get choices from same lesson; for Hangul, from unlocked cards
        let all_cards = if let (Some(pack_id), Some(lesson)) = (&next_card.pack_id, next_card.lesson) {
          db::get_cards_from_same_lesson(&conn, pack_id, lesson)
            .log_warn_default("Failed to get lesson cards for choices")
        } else {
          db::get_unlocked_cards(&conn)
            .log_warn_default("Failed to get unlocked cards for choices")
        };
        generate_choices(next_card, &all_cards)
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
        is_vocabulary: next_card.pack_id.is_some(),
        validated: false,
        is_correct: false,
        user_answer: String::new(),
        quality: 0,
        hints_used: 0,
        hint_1: String::new(),
        hint_2: String::new(),
        hint_final: String::new(),
        is_multiple_choice: is_korean,
        choices,
        session_id: String::new(),
        is_tracked: false,
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
        is_reverse: next_card.is_reverse,
        is_vocabulary: next_card.pack_id.is_some(),
      };
      Html(template.render().unwrap_or_default())
    }
  } else {
    Html("<p>No more cards available.</p>".to_string())
  }
}

/// Validate answer in practice mode (optionally logs to stats)
pub async fn practice_validate(
  auth: AuthContext,
  Form(form): Form<PracticeValidateForm>,
) -> impl IntoResponse {
  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
    }
  };

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
    matches!(
      result,
      crate::validation::AnswerResult::Correct | crate::validation::AnswerResult::CloseEnough
    )
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
    // For vocabulary cards, get choices from same lesson; for Hangul, from unlocked cards
    let all_cards = if let (Some(pack_id), Some(lesson)) = (&card.pack_id, card.lesson) {
      db::get_cards_from_same_lesson(&conn, pack_id, lesson)
        .log_warn_default("Failed to get lesson cards for choices")
    } else {
      db::get_unlocked_cards(&conn).log_warn_default("Failed to get unlocked cards for choices")
    };
    generate_choices(&card, &all_cards)
  } else {
    vec![]
  };

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
    user_answer: form.answer,
    quality: 0,
    hints_used: 0,
    hint_1: String::new(),
    hint_2: String::new(),
    hint_final: String::new(),
    is_multiple_choice: is_korean,
    choices,
    session_id: String::new(),
    is_tracked: false,
    track_progress: form.track_progress,
  };

  Html(template.render().unwrap_or_default())
}
