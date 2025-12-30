//! Template and form structs for study handlers.

use askama::Template;
use serde::Deserialize;

use crate::domain::InputMethod;
use crate::filters;

#[derive(Template)]
#[template(path = "study.html")]
pub struct StudyTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub is_reverse: bool,
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
  pub is_reverse: bool,
}

#[derive(Template)]
#[template(path = "practice_card.html")]
pub struct PracticeCardTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub is_reverse: bool,
}

#[derive(Template)]
#[template(path = "no_cards.html")]
pub struct NoCardsTemplate {}

/// Interactive card template with input-based validation
/// Used for both study mode (tracked) and practice mode (optional tracking)
#[derive(Template)]
#[template(path = "interactive_card.html")]
pub struct InteractiveCardTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub is_reverse: bool,
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
  // Session tracking (study mode)
  pub session_id: String,
  // Mode control
  pub is_tracked: bool,        // true = study mode, false = practice mode
  pub track_progress: bool,    // for practice mode: whether to log progress
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
  pub is_reverse: bool,
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
  // Mode control
  pub is_tracked: bool,
  pub track_progress: bool,
  // Testing mode flag
  pub testing_mode: bool,
  // Focus mode recommendation
  pub focus_mode_active: bool,
  pub focus_tier: u8,
  pub focus_tier_progress: i64,
  pub show_exit_focus_recommendation: bool,
}

#[derive(Template)]
#[template(path = "practice.html")]
pub struct PracticeTemplate {
  pub card_id: i64,
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
  pub tier: u8,
  pub is_reverse: bool,
  pub mode: String,
  // Interactive mode fields
  pub validated: bool,
  pub is_correct: bool,
  pub user_answer: String,
  pub is_multiple_choice: bool,
  pub choices: Vec<String>,
  // Progress tracking
  pub track_progress: bool,
  // Fields for unified interactive_card.html (unused in practice mode but required)
  pub quality: u8,
  pub hints_used: u8,
  pub hint_1: String,
  pub hint_2: String,
  pub hint_final: String,
  pub session_id: String,
  pub is_tracked: bool,
}

// ============================================================================
// Form Structs
// ============================================================================

#[derive(Deserialize)]
pub struct ReviewForm {
  pub card_id: i64,
  pub quality: u8,
  #[serde(default)]
  pub session_id: String,
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

#[derive(Deserialize)]
pub struct NextCardForm {
  pub card_id: i64,
  #[serde(default)]
  pub session_id: String,
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

#[derive(Deserialize)]
pub struct PracticeForm {
  pub card_id: i64,
  #[serde(default)]
  pub track_progress: bool,
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
