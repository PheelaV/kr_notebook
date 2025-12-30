//! User-facing settings page and preferences.

use askama::Template;
use axum::response::{Html, Redirect};
use axum::Form;
use serde::Deserialize;

use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::filters;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

use super::audio::{get_lesson_audio, LessonAudio, TierGraduationStatus};
use super::{count_syllables, has_lesson1, has_lesson2, has_lesson3};

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
  pub is_admin: bool,
  pub all_tiers_unlocked: bool,
  pub enabled_tiers: Vec<u8>,
  pub desired_retention: u8, // 80, 85, 90, or 95
  pub focus_tier: Option<u8>, // Currently focused tier (None = no focus)
  pub max_unlocked_tier: u8,
  pub has_scraped_content: bool,
  pub has_pronunciation: bool,
  // Per-lesson status
  pub has_lesson1: bool,
  pub has_lesson2: bool,
  pub has_lesson3: bool,
  pub lesson1_syllables: usize,
  pub lesson2_syllables: usize,
  pub lesson3_syllables: usize,
  // Audio preview data
  pub lesson_audio: Vec<LessonAudio>,
  // Tier graduation status
  pub tier_graduation: Vec<TierGraduationStatus>,
}

/// Error HTML for database unavailable
const DB_ERROR_HTML: &str = r#"<!DOCTYPE html><html><head><title>Error</title></head><body><h1>Database Error</h1><p>Please refresh the page.</p></body></html>"#;

pub async fn settings_page(auth: AuthContext) -> Html<String> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings".into(),
    method: "GET".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Html(DB_ERROR_HTML.to_string()),
  };
  let all_tiers_unlocked = db::get_all_tiers_unlocked(&conn).log_warn_default("Failed to get all_tiers_unlocked");
  let enabled_tiers = db::get_enabled_tiers(&conn).log_warn_default("Failed to get enabled tiers");
  let desired_retention_f64 = db::get_desired_retention(&conn).log_warn_default("Failed to get desired retention");
  let desired_retention = (desired_retention_f64 * 100.0).round() as u8;
  let focus_tier = db::get_focus_tier(&conn).log_warn_default("Failed to get focus tier");
  let max_unlocked_tier = db::get_max_unlocked_tier(&conn).log_warn_default("Failed to get max unlocked tier");

  let has_l1 = has_lesson1();
  let has_l2 = has_lesson2();
  let has_l3 = has_lesson3();
  let scraped_content_available = has_l1 || has_l2 || has_l3;

  // Get audio preview data
  let mut lesson_audio = Vec::new();
  if has_l1 {
    if let Some(audio) = get_lesson_audio("lesson1", "Lesson 1: Basic Consonants & Vowels") {
      lesson_audio.push(audio);
    }
  }
  if has_l2 {
    if let Some(audio) = get_lesson_audio("lesson2", "Lesson 2: Additional Consonants") {
      lesson_audio.push(audio);
    }
  }
  if has_l3 {
    if let Some(audio) = get_lesson_audio("lesson3", "Lesson 3: Diphthongs & Combined Vowels") {
      lesson_audio.push(audio);
    }
  }

  // Get tier graduation status
  let tier_graduation: Vec<TierGraduationStatus> = (1..=4u8)
    .map(|tier| TierGraduationStatus {
      tier,
      is_fully_graduated: db::is_tier_fully_graduated(&conn, tier).unwrap_or(false),
      has_backup: db::has_tier_backup(&conn, tier).unwrap_or(false),
    })
    .collect();

  let template = SettingsTemplate {
    is_admin: auth.is_admin,
    all_tiers_unlocked,
    enabled_tiers,
    desired_retention,
    focus_tier,
    max_unlocked_tier,
    has_scraped_content: scraped_content_available,
    has_pronunciation: scraped_content_available,
    has_lesson1: has_l1,
    has_lesson2: has_l2,
    has_lesson3: has_l3,
    lesson1_syllables: if has_l1 { count_syllables("lesson1") } else { 0 },
    lesson2_syllables: if has_l2 { count_syllables("lesson2") } else { 0 },
    lesson3_syllables: if has_l3 { count_syllables("lesson3") } else { 0 },
    lesson_audio,
    tier_graduation,
  };
  Html(template.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct SettingsForm {
  #[serde(default)]
  pub all_tiers_unlocked: Option<String>,
  #[serde(default)]
  pub tier_1: Option<String>,
  #[serde(default)]
  pub tier_2: Option<String>,
  #[serde(default)]
  pub tier_3: Option<String>,
  #[serde(default)]
  pub tier_4: Option<String>,
  #[serde(default)]
  pub desired_retention: Option<u8>,
  #[serde(default)]
  pub focus_tier: Option<String>, // "none" or "1", "2", "3", "4"
}

pub async fn update_settings(
  auth: AuthContext,
  Form(form): Form<SettingsForm>,
) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/settings"),
  };

  // Update all_tiers_unlocked
  let all_tiers_unlocked = form.all_tiers_unlocked.is_some();
  db::set_all_tiers_unlocked(&conn, all_tiers_unlocked)
    .log_warn("Failed to save all_tiers_unlocked setting");

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::SettingsUpdate {
    setting: "all_tiers_unlocked".into(),
    value: all_tiers_unlocked.to_string(),
    username: auth.username.clone(),
  });

  // Update enabled tiers
  let mut enabled_tiers = Vec::new();
  if form.tier_1.is_some() {
    enabled_tiers.push(1);
  }
  if form.tier_2.is_some() {
    enabled_tiers.push(2);
  }
  if form.tier_3.is_some() {
    enabled_tiers.push(3);
  }
  if form.tier_4.is_some() {
    enabled_tiers.push(4);
  }

  // Ensure at least tier 1 is enabled
  if enabled_tiers.is_empty() {
    enabled_tiers.push(1);
  }

  db::set_enabled_tiers(&conn, &enabled_tiers)
    .log_warn("Failed to save enabled_tiers setting");

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::SettingsUpdate {
    setting: "enabled_tiers".into(),
    value: format!("{:?}", enabled_tiers),
    username: auth.username.clone(),
  });

  // Update desired retention if provided
  if let Some(retention) = form.desired_retention {
    // Validate and clamp to valid range
    let retention_pct = retention.clamp(80, 95);
    let retention_f64 = f64::from(retention_pct) / 100.0;
    db::set_setting(&conn, "desired_retention", &retention_f64.to_string())
      .log_warn("Failed to save desired_retention setting");

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::SettingsUpdate {
      setting: "desired_retention".into(),
      value: retention_f64.to_string(),
      username: auth.username.clone(),
    });
  }

  // Update focus tier if provided
  if let Some(focus_str) = form.focus_tier {
    let focus_tier = if focus_str == "none" || focus_str.is_empty() {
      None
    } else {
      focus_str.parse::<u8>().ok()
    };
    db::set_focus_tier(&conn, focus_tier)
      .log_warn("Failed to save focus_tier setting");

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::SettingsUpdate {
      setting: "focus_tier".into(),
      value: focus_tier.map(|t| t.to_string()).unwrap_or_else(|| "none".to_string()),
      username: auth.username.clone(),
    });
  }

  Redirect::to("/settings")
}
