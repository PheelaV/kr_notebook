//! User-facing settings page and preferences.

use askama::Template;
use axum::extract::{Multipart, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use chrono::Utc;
use rusqlite::Connection;
use serde::Deserialize;

use crate::auth::db as auth_db;
use crate::auth::AuthContext;
use crate::content::{self, PackLocation, PackType};
use crate::db::{self, run_migrations, LogOnError};
use crate::filters;
use crate::services::pack_manager;
use crate::state::AppState;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

use super::admin::RegisteredPathDisplay;
use super::audio::{get_lesson_audio, LessonAudio, TierGraduationStatus};
use super::{count_syllables, has_lesson1, has_lesson2, has_lesson3};
use crate::handlers::NavContext;

/// TOC item for navigation
pub struct TocItem {
  pub id: String,
  pub short_label: String,
  pub full_label: String,
}

/// User info for admin display in templates
#[derive(Debug, Clone)]
pub struct UserDisplay {
  pub id: i64,
  pub username: String,
  pub role: String,
  pub is_guest: bool,
}

/// Group member info for display
#[derive(Debug, Clone)]
pub struct GroupMember {
  pub id: i64,
  pub username: String,
}

/// Group info for admin display
#[derive(Debug, Clone)]
pub struct GroupDisplay {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  pub members: Vec<GroupMember>,
}

/// Allowed user info for pack permissions
#[derive(Debug, Clone)]
pub struct AllowedUser {
  pub id: i64,
  pub username: String,
}

/// Pack info for UI display
#[derive(Debug, Clone)]
pub struct PackInfo {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  pub pack_type: String,
  pub version: Option<String>,
  pub is_enabled: bool,
  pub is_baseline: bool,             // Baseline pack (always enabled, can't disable)
  pub cards_count: Option<usize>,    // For card packs
  pub is_restricted: bool,           // Has any group or user restrictions
  pub allowed_groups: Vec<String>,   // Group IDs that can access this pack
  pub allowed_users: Vec<AllowedUser>, // Users that have direct access
  pub can_manage: bool,              // User can enable/disable this pack
}

impl PackInfo {
  /// Create PackInfo from a discovered pack
  pub fn from_location(
    loc: &PackLocation,
    _enabled_packs: &[String],
    auth_conn: Option<&Connection>,
    is_admin: bool,
  ) -> Self {
    let is_baseline = loc.manifest.id == "baseline";
    let cards_count = if loc.manifest.pack_type == PackType::Cards {
      // Try to count cards in the pack
      loc.manifest.cards.as_ref().and_then(|cfg| {
        let cards_path = loc.path.join(&cfg.file);
        std::fs::read_to_string(&cards_path)
          .ok()
          .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
          .and_then(|v| v["cards"].as_array().map(|a| a.len()))
      })
    } else {
      None
    };

    // Get pack permissions and global enabled state from auth_db
    let (is_restricted, allowed_groups, allowed_users, is_globally_enabled) = auth_conn
      .map(|conn| {
        let restricted = auth_db::is_pack_restricted_for_ui(conn, &loc.manifest.id).unwrap_or(false);
        let groups = auth_db::get_pack_allowed_groups(conn, &loc.manifest.id).unwrap_or_default();
        let users = auth_db::get_pack_allowed_users(conn, &loc.manifest.id)
          .unwrap_or_default()
          .into_iter()
          .map(|(id, username)| AllowedUser { id, username })
          .collect();
        let globally_enabled = auth_db::is_pack_globally_enabled(conn, &loc.manifest.id).unwrap_or(true);
        (restricted, groups, users, globally_enabled)
      })
      .unwrap_or((false, Vec::new(), Vec::new(), true));

    // Determine if user can manage (enable/disable) this pack:
    // - Only admins can enable/disable packs
    // - Baseline pack cannot be disabled by anyone
    let can_manage = !is_baseline && is_admin;

    PackInfo {
      id: loc.manifest.id.clone(),
      name: loc.manifest.name.clone(),
      description: loc.manifest.description.clone(),
      pack_type: loc.manifest.pack_type.as_str().to_string(),
      version: loc.manifest.version.clone(),
      is_enabled: is_baseline || is_globally_enabled,
      is_baseline,
      cards_count,
      is_restricted,
      allowed_groups,
      allowed_users,
      can_manage,
    }
  }
}

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
  // Content packs
  pub card_packs: Vec<PackInfo>,
  // App version
  pub version: &'static str,
  // Admin: users and groups
  pub users: Vec<UserDisplay>,
  pub groups: Vec<GroupDisplay>,
  // Admin: external pack paths
  pub paths: Vec<RegisteredPathDisplay>,
  // TOC navigation
  pub toc_items: Vec<TocItem>,
  pub toc_title: String,
  pub nav: NavContext,
}

/// Error HTML for database unavailable
const DB_ERROR_HTML: &str = r#"<!DOCTYPE html><html><head><title>Error</title></head><body><h1>Database Error</h1><p>Please refresh the page.</p></body></html>"#;

pub async fn settings_page(auth: AuthContext, State(state): State<AppState>) -> Html<String> {
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
  if has_l1
    && let Some(audio) = get_lesson_audio("lesson1", "Lesson 1: Basic Consonants & Vowels") {
      lesson_audio.push(audio);
    }
  if has_l2
    && let Some(audio) = get_lesson_audio("lesson2", "Lesson 2: Additional Consonants") {
      lesson_audio.push(audio);
    }
  if has_l3
    && let Some(audio) = get_lesson_audio("lesson3", "Lesson 3: Diphthongs & Combined Vowels") {
      lesson_audio.push(audio);
    }

  // Get tier graduation status
  let tier_graduation: Vec<TierGraduationStatus> = (1..=4u8)
    .map(|tier| TierGraduationStatus {
      tier,
      is_fully_graduated: db::is_tier_fully_graduated(&conn, tier).unwrap_or(false),
      has_backup: db::has_tier_backup(&conn, tier).unwrap_or(false),
    })
    .collect();

  // Get auth_db connection for pack permissions and external paths lookup
  let auth_conn = state.auth_db.lock().ok();

  // Discover content packs (including external registered paths)
  let enabled_packs = content::list_enabled_packs(&conn);
  let discovered = auth_conn.as_deref()
    .map(pack_manager::discover_all_packs)
    .unwrap_or_default();

  // Filter to card packs only, and for non-admin users, only show packs they can access
  let card_packs: Vec<PackInfo> = discovered
    .iter()
    .filter(|loc| loc.manifest.pack_type == PackType::Cards)
    .filter(|loc| {
      // Admins can see all packs
      if auth.is_admin {
        return true;
      }
      // Non-admins only see packs they have permission to access
      auth_conn.as_deref()
        .map(|db| pack_manager::can_access(db, auth.user_id, &loc.manifest.id))
        .unwrap_or(true) // If no auth_db, allow (shouldn't happen)
    })
    .map(|loc| PackInfo::from_location(loc, &enabled_packs, auth_conn.as_deref(), auth.is_admin))
    .collect();

  // Fetch users, groups, and registered paths for admin
  let (users, groups, registered_paths) = if auth.is_admin {
    let users = auth_conn.as_deref()
      .and_then(|db| auth_db::get_all_users(db).ok())
      .unwrap_or_default()
      .into_iter()
      .map(|u| UserDisplay {
        id: u.id,
        username: u.username,
        role: u.role,
        is_guest: u.is_guest,
      })
      .collect();

    let groups = auth_conn.as_deref()
      .and_then(|db| auth_db::get_all_groups(db).ok())
      .unwrap_or_default()
      .into_iter()
      .map(|g| {
        let members = auth_conn.as_deref()
          .and_then(|db| auth_db::get_group_members(db, &g.id).ok())
          .unwrap_or_default()
          .into_iter()
          .map(|(id, username)| GroupMember { id, username })
          .collect();
        GroupDisplay {
          id: g.id,
          name: g.name,
          description: g.description,
          members,
        }
      })
      .collect();

    // Fetch registered pack paths
    let registered_paths = auth_conn.as_deref()
      .and_then(|db| {
        use crate::auth::db as auth_db;
        use crate::content::count_packs_in_directory;
        auth_db::get_all_registered_paths(db).ok().map(|db_paths| {
          db_paths.into_iter().map(|p| {
            let pack_count = count_packs_in_directory(std::path::Path::new(&p.path));
            RegisteredPathDisplay {
              id: p.id,
              path: p.path,
              name: p.name,
              registered_by: p.registered_by,
              is_active: p.is_active,
              pack_count,
            }
          }).collect()
        })
      })
      .unwrap_or_default();

    (users, groups, registered_paths)
  } else {
    (Vec::new(), Vec::new(), Vec::new())
  };

  // Build TOC items based on available sections
  let mut toc_items = vec![
    TocItem { id: "appearance".into(), short_label: "Appearance".into(), full_label: "Appearance".into() },
    TocItem { id: "learning".into(), short_label: "Learning".into(), full_label: "Learning".into() },
    TocItem { id: "study-tools".into(), short_label: "Tools".into(), full_label: "Study Tools".into() },
    TocItem { id: "data".into(), short_label: "Data".into(), full_label: "Data Management".into() },
  ];
  if auth.is_admin {
    toc_items.push(TocItem { id: "users".into(), short_label: "Users".into(), full_label: "User Management".into() });
    toc_items.push(TocItem { id: "groups".into(), short_label: "Groups".into(), full_label: "User Groups".into() });
    toc_items.push(TocItem { id: "guests".into(), short_label: "Guests".into(), full_label: "Guest Management".into() });
    toc_items.push(TocItem { id: "pack-paths".into(), short_label: "Pack Paths".into(), full_label: "External Pack Paths".into() });
  }
  if !card_packs.is_empty() {
    toc_items.push(TocItem { id: "packs".into(), short_label: "Packs".into(), full_label: "Content Packs".into() });
  }
  toc_items.push(TocItem { id: "audio".into(), short_label: "Audio".into(), full_label: "Pronunciation Audio".into() });

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
    card_packs,
    version: env!("CARGO_PKG_VERSION"),
    users,
    groups,
    paths: registered_paths,
    toc_items,
    toc_title: "Settings".to_string(),
    nav: NavContext::from_auth(&auth),
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
  if db_path.exists()
    && let Err(e) = std::fs::rename(&db_path, &backup_path) {
      tracing::error!("Failed to backup current database: {}", e);
      return import_error_redirect("Failed to backup current data");
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

// ==================== Pack Enable/Disable ====================

use axum::extract::Path as AxumPath;

/// Check if request is from HTMX
fn is_htmx_request(headers: &HeaderMap) -> bool {
  headers.get("HX-Request").is_some()
}

/// Helper to create an error notification HTML (uses hx-swap-oob to update notifications area)
fn error_notification(message: &str) -> String {
  format!(
    r#"<div id="notifications" hx-swap-oob="innerHTML:#notifications">
      <div class="p-4 mb-4 text-sm text-red-700 bg-red-100 dark:bg-red-900/30 dark:text-red-300 rounded-lg flex items-center justify-between" role="alert">
        <span>{}</span>
        <button type="button" onclick="this.parentElement.remove()" class="ml-4 text-red-700 dark:text-red-300 hover:text-red-900 dark:hover:text-red-100">&times;</button>
      </div>
    </div>"#,
    message
  )
}

/// Template for pack card partial (for HTMX updates)
#[derive(Template)]
#[template(path = "partials/settings_pack_card.html")]
struct PackCardTemplate {
  pack: PackInfo,
  is_admin: bool,
  groups: Vec<GroupDisplay>,
  users: Vec<UserDisplay>,
}

/// Enable a card pack for the current user
pub async fn enable_pack(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(pack_id): AxumPath<String>,
) -> Response {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: format!("/settings/pack/{}/enable", pack_id),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  // Get connections (auth_db is the shared app.db with card_definitions)
  let user_conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings?pack=error").into_response();
    }
  };

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings?pack=error").into_response();
    }
  };

  // Find the pack (including external registered paths)
  let pack_loc = match pack_manager::find_pack_by_id(&app_conn, &pack_id) {
    Some(loc) => loc,
    None => {
      tracing::warn!("Pack not found: {}", pack_id);
      if is_htmx_request(&headers) {
        return Html(error_notification("Pack not found")).into_response();
      }
      return Redirect::to("/settings?pack=notfound").into_response();
    }
  };

  // Only allow enabling card packs
  if pack_loc.manifest.pack_type != PackType::Cards {
    tracing::warn!("Cannot enable non-card pack: {}", pack_id);
    if is_htmx_request(&headers) {
      return Html(error_notification("Cannot enable non-card pack")).into_response();
    }
    return Redirect::to("/settings?pack=invalid").into_response();
  }

  // Check if user has permission to access this pack (non-admins only)
  if !auth.is_admin
    && !pack_manager::can_access(&app_conn, auth.user_id, &pack_id) {
      tracing::warn!("User {} tried to enable pack {} without permission", auth.username, pack_id);
      if is_htmx_request(&headers) {
        return Html(error_notification("You don't have permission to enable this pack")).into_response();
      }
      return Redirect::to("/settings?pack=noaccess").into_response();
    }

  let cards_config = match &pack_loc.manifest.cards {
    Some(cfg) => cfg,
    None => {
      tracing::warn!("Card pack missing cards config: {}", pack_id);
      if is_htmx_request(&headers) {
        return Html(error_notification("Pack configuration is invalid")).into_response();
      }
      return Redirect::to("/settings?pack=invalid").into_response();
    }
  };

  // Enable the pack
  match content::enable_card_pack(
    &app_conn,
    &user_conn,
    &pack_id,
    &pack_loc.manifest.name,
    pack_loc.manifest.version.as_deref().unwrap_or("1.0.0"),
    pack_loc.manifest.description.as_deref(),
    &pack_loc.manifest.scope,
    &pack_loc.path,
    &cards_config.file,
  ) {
    Ok(result) => {
      tracing::info!(
        "User {} enabled pack {}: {} cards inserted, {} skipped",
        auth.username,
        pack_id,
        result.cards_inserted,
        result.cards_skipped
      );

      // Store UI metadata if pack has it (for progress page display)
      if let Some(ref ui) = pack_loc.manifest.ui {
        let total_lessons = pack_loc.manifest.lessons.as_ref().map(|l| l.total);
        if let Err(e) = db::store_pack_ui_metadata(&app_conn, &pack_id, ui, total_lessons) {
          tracing::warn!("Failed to store UI metadata for pack {}: {}", pack_id, e);
        }
      }

      // Set global enabled state (for global packs)
      if let Err(e) = auth_db::set_pack_globally_enabled(&app_conn, &pack_id, true) {
        tracing::warn!("Failed to set pack {} globally enabled: {}", pack_id, e);
      }

      // Return HTMX partial or redirect
      if is_htmx_request(&headers) {
        let enabled_packs = Vec::new(); // Not used since we use is_globally_enabled now
        let pack_info = PackInfo::from_location(&pack_loc, &enabled_packs, Some(&app_conn), auth.is_admin);

        // Fetch groups and users for permissions section
        let (groups, users) = if auth.is_admin {
          let groups = auth_db::get_all_groups(&app_conn)
            .unwrap_or_default()
            .into_iter()
            .map(|g| {
              let members = auth_db::get_group_members(&app_conn, &g.id)
                .unwrap_or_default()
                .into_iter()
                .map(|(id, username)| GroupMember { id, username })
                .collect();
              GroupDisplay {
                id: g.id,
                name: g.name,
                description: g.description,
                members,
              }
            })
            .collect();
          let users = auth_db::get_all_users(&app_conn)
            .unwrap_or_default()
            .into_iter()
            .map(|u| UserDisplay {
              id: u.id,
              username: u.username,
              role: u.role,
              is_guest: u.is_guest,
            })
            .collect();
          (groups, users)
        } else {
          (Vec::new(), Vec::new())
        };

        let template = PackCardTemplate {
          pack: pack_info,
          is_admin: auth.is_admin,
          groups,
          users,
        };
        return match template.render() {
          Ok(html) => Html(html).into_response(),
          Err(e) => {
            tracing::error!("Failed to render pack card template: {}", e);
            Html(error_notification(&format!("Failed to render pack card: {}", e))).into_response()
          }
        };
      }

      Redirect::to(&format!("/settings?pack=enabled&id={}", pack_id)).into_response()
    }
    Err(e) => {
      // Log full error details for debugging, but show sanitized message to user
      tracing::error!("Failed to enable pack {}: {}", pack_id, e);
      if is_htmx_request(&headers) {
        return Html(error_notification(&format!("Failed to enable pack: {}", e.user_message()))).into_response();
      }
      Redirect::to("/settings?pack=error").into_response()
    }
  }
}

/// Disable a pack globally (admin only for global packs)
pub async fn disable_pack(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  AxumPath(pack_id): AxumPath<String>,
) -> Response {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: format!("/settings/pack/{}/disable", pack_id),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  // Only admin can disable global packs
  if !auth.is_admin {
    tracing::warn!("Non-admin user {} tried to disable pack {}", auth.username, pack_id);
    if is_htmx_request(&headers) {
      return Html(error_notification("You don't have permission to disable this pack")).into_response();
    }
    return Redirect::to("/settings?pack=noaccess").into_response();
  }

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings?pack=error").into_response();
    }
  };

  // Set global enabled state to false
  if let Err(e) = auth_db::set_pack_globally_enabled(&app_conn, &pack_id, false) {
    tracing::error!("Failed to disable pack {} globally: {}", pack_id, e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to disable pack: {}", e))).into_response();
    }
    return Redirect::to("/settings?pack=error").into_response();
  }

  tracing::info!("Admin {} disabled pack {} globally", auth.username, pack_id);

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    // Find the pack and render updated card
    if let Some(pack_loc) = pack_manager::find_pack_by_id(&app_conn, &pack_id) {
      let enabled_packs = Vec::new(); // Not used since we use is_globally_enabled now
      let pack_info = PackInfo::from_location(&pack_loc, &enabled_packs, Some(&app_conn), auth.is_admin);

      // Fetch groups and users for permissions section (admin only)
      let groups = auth_db::get_all_groups(&app_conn)
        .unwrap_or_default()
        .into_iter()
        .map(|g| {
          let members = auth_db::get_group_members(&app_conn, &g.id)
            .unwrap_or_default()
            .into_iter()
            .map(|(id, username)| GroupMember { id, username })
            .collect();
          GroupDisplay {
            id: g.id,
            name: g.name,
            description: g.description,
            members,
          }
        })
        .collect();
      let users = auth_db::get_all_users(&app_conn)
        .unwrap_or_default()
        .into_iter()
        .map(|u| UserDisplay {
          id: u.id,
          username: u.username,
          role: u.role,
          is_guest: u.is_guest,
        })
        .collect();

      let template = PackCardTemplate {
        pack: pack_info,
        is_admin: auth.is_admin,
        groups,
        users,
      };
      return match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
          tracing::error!("Failed to render pack card template: {}", e);
          Html(error_notification(&format!("Failed to render pack card: {}", e))).into_response()
        }
      };
    }

    return Html(error_notification("Pack not found")).into_response();
  }

  Redirect::to(&format!("/settings?pack=disabled&id={}", pack_id)).into_response()
}
