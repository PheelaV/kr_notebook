//! User-facing settings page and preferences.

use askama::Template;
use axum::extract::{Multipart, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use chrono::Utc;
use rusqlite::Connection;
use serde::Deserialize;

use crate::auth::AuthContext;
use crate::db::{self, run_migrations, LogOnError};
use crate::filters;
use crate::state::AppState;
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

/// Export user's learning database as a downloadable file
pub async fn export_data(auth: AuthContext, State(state): State<AppState>) -> Response {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/export".into(),
    method: "GET".into(),
    username: Some(auth.username.clone()),
  });

  let db_path = state.user_db_path(&auth.username);

  // Read the database file
  let file_bytes = match std::fs::read(&db_path) {
    Ok(bytes) => bytes,
    Err(e) => {
      tracing::error!("Failed to read database file for export: {}", e);
      return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to export data").into_response();
    }
  };

  // Generate filename with timestamp
  let date = Utc::now().format("%Y%m%d");
  let filename = format!("kr_notebook_{}_{}.db", auth.username, date);

  // Return as downloadable file
  (
    [
      (header::CONTENT_TYPE, "application/x-sqlite3"),
      (
        header::CONTENT_DISPOSITION,
        &format!("attachment; filename=\"{}\"", filename),
      ),
    ],
    file_bytes,
  )
    .into_response()
}

/// Import a learning database from uploaded file
pub async fn import_data(
  auth: AuthContext,
  State(state): State<AppState>,
  mut multipart: Multipart,
) -> Response {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/import".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  // Extract the uploaded file
  let file_bytes = match extract_uploaded_file(&mut multipart).await {
    Ok(bytes) => bytes,
    Err(e) => {
      tracing::warn!("Import failed: {}", e);
      return import_error_redirect(&e);
    }
  };

  // Validate it's a valid SQLite database with expected tables
  if let Err(e) = validate_imported_database(&file_bytes) {
    tracing::warn!("Import validation failed: {}", e);
    return import_error_redirect(&e);
  }

  // Drop the current database connection before file operations
  drop(auth.user_db);

  let db_path = state.user_db_path(&auth.username);
  let backup_path = db_path.with_extension("db.old");

  // Backup current database
  if db_path.exists() {
    if let Err(e) = std::fs::rename(&db_path, &backup_path) {
      tracing::error!("Failed to backup current database: {}", e);
      return import_error_redirect("Failed to backup current data");
    }
  }

  // Write new database file
  if let Err(e) = std::fs::write(&db_path, &file_bytes) {
    tracing::error!("Failed to write imported database: {}", e);
    // Try to restore backup
    if backup_path.exists() {
      let _ = std::fs::rename(&backup_path, &db_path);
    }
    return import_error_redirect("Failed to save imported data");
  }

  // Run migrations on the new database
  match Connection::open(&db_path) {
    Ok(conn) => {
      if let Err(e) = run_migrations(&conn) {
        tracing::error!("Failed to run migrations on imported database: {}", e);
        // Restore backup
        drop(conn);
        let _ = std::fs::remove_file(&db_path);
        if backup_path.exists() {
          let _ = std::fs::rename(&backup_path, &db_path);
        }
        return import_error_redirect("Imported database failed migration");
      }
    }
    Err(e) => {
      tracing::error!("Failed to open imported database: {}", e);
      let _ = std::fs::remove_file(&db_path);
      if backup_path.exists() {
        let _ = std::fs::rename(&backup_path, &db_path);
      }
      return import_error_redirect("Failed to open imported database");
    }
  }

  tracing::info!("User {} successfully imported database", auth.username);

  // Success - redirect with message
  Redirect::to("/settings?import=success").into_response()
}

/// Extract file bytes from multipart upload
async fn extract_uploaded_file(multipart: &mut Multipart) -> Result<Vec<u8>, String> {
  while let Ok(Some(field)) = multipart.next_field().await {
    let name = field.name().unwrap_or_default().to_string();
    if name == "database" {
      return field
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Failed to read upload: {}", e));
    }
  }
  Err("No database file uploaded".to_string())
}

/// Validate that the uploaded bytes are a valid SQLite database with expected schema
fn validate_imported_database(bytes: &[u8]) -> Result<(), String> {
  // Check SQLite magic header
  if bytes.len() < 16 || &bytes[0..16] != b"SQLite format 3\0" {
    return Err("Not a valid SQLite database file".to_string());
  }

  // Write to temp file and try to open
  let temp_path = std::env::temp_dir().join(format!("import_validate_{}.db", std::process::id()));
  std::fs::write(&temp_path, bytes).map_err(|e| format!("Validation error: {}", e))?;

  let result = (|| {
    let conn = Connection::open(&temp_path).map_err(|e| format!("Invalid database: {}", e))?;

    // Check for required tables (accept either legacy 'cards' or new 'card_progress')
    let has_cards: bool = conn
      .query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cards')",
        [],
        |row| row.get(0),
      )
      .unwrap_or(false);

    let has_card_progress: bool = conn
      .query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='card_progress')",
        [],
        |row| row.get(0),
      )
      .unwrap_or(false);

    if !has_cards && !has_card_progress {
      return Err("Database missing 'cards' or 'card_progress' table".to_string());
    }

    // For legacy format, verify the schema
    if has_cards {
      let column_check = conn.prepare("SELECT id, front, main_answer FROM cards LIMIT 1");
      if column_check.is_err() {
        return Err("Cards table missing required columns".to_string());
      }
    }

    Ok(())
  })();

  // Clean up temp file
  let _ = std::fs::remove_file(&temp_path);

  result
}

/// Create redirect response for import error
fn import_error_redirect(error: &str) -> Response {
  let encoded = urlencoding::encode(error);
  Redirect::to(&format!("/settings?import=error&message={}", encoded)).into_response()
}
