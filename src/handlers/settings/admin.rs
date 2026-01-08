//! Admin-only operations: scraper, segmentation, tier graduation, guest management, user/group management.

use askama::Template;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use std::process::Command;

use crate::auth::db as auth_db;
use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::paths;
use crate::state::AppState;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

use super::audio::{get_audio_row, get_lesson_audio, AudioRow, SegmentParams};
use super::user::{GroupDisplay, GroupMember, UserDisplay, AllowedUser, PackInfo};
use crate::content::{discover_packs, PackType};

// ============================================================================
// Scraper Operations
// ============================================================================

/// Scrape all lessons (admin only)
pub async fn trigger_scrape(auth: AuthContext) -> Redirect {
  if !auth.is_admin {
    return Redirect::to("/settings");
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/scrape".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  // Run the scraper commands for all lessons
  let cmd = format!(
    "cd {} && uv run kr-scraper lesson1 && uv run kr-scraper lesson2 && uv run kr-scraper lesson3 && uv run kr-scraper segment --padding 75",
    paths::PY_SCRIPTS_DIR
  );
  match Command::new("sh").args(["-c", &cmd]).output() {
    Ok(output) if !output.status.success() => {
      tracing::warn!(
        "Scrape command failed with status {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
      );
    }
    Err(e) => tracing::warn!("Failed to run scrape command: {}", e),
    _ => {}
  }

  Redirect::to("/settings")
}

/// Scrape a specific lesson (admin only)
pub async fn trigger_scrape_lesson(auth: AuthContext, Path(lesson): Path<String>) -> Redirect {
  if !auth.is_admin {
    return Redirect::to("/settings");
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: format!("/settings/scrape/{}", lesson),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let cmd = match lesson.as_str() {
    "1" => format!(
      "cd {} && uv run kr-scraper lesson1 && uv run kr-scraper segment -l 1 --padding 75",
      paths::PY_SCRIPTS_DIR
    ),
    "2" => format!(
      "cd {} && uv run kr-scraper lesson2 && uv run kr-scraper segment -l 2 --padding 75",
      paths::PY_SCRIPTS_DIR
    ),
    "3" => format!(
      "cd {} && uv run kr-scraper lesson3 && uv run kr-scraper segment -l 3 --padding 75",
      paths::PY_SCRIPTS_DIR
    ),
    _ => return Redirect::to("/settings"),
  };

  match Command::new("sh").args(["-c", &cmd]).output() {
    Ok(output) if !output.status.success() => {
      tracing::warn!(
        "Scrape lesson {} failed with status {}: {}",
        lesson,
        output.status,
        String::from_utf8_lossy(&output.stderr)
      );
    }
    Err(e) => tracing::warn!("Failed to run scrape command for lesson {}: {}", lesson, e),
    _ => {}
  }

  Redirect::to("/settings")
}

/// Delete all scraped content (admin only)
pub async fn delete_scraped(auth: AuthContext) -> Redirect {
  if !auth.is_admin {
    return Redirect::to("/settings");
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/delete-scraped".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  // Run the clean command
  let cmd = format!("cd {} && uv run kr-scraper clean --yes", paths::PY_SCRIPTS_DIR);
  match Command::new("sh").args(["-c", &cmd]).output() {
    Ok(output) if !output.status.success() => {
      tracing::warn!(
        "Clean command failed with status {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
      );
    }
    Err(e) => tracing::warn!("Failed to run clean command: {}", e),
    _ => {}
  }

  Redirect::to("/settings")
}

/// Delete a specific lesson's content (admin only)
pub async fn delete_scraped_lesson(auth: AuthContext, Path(lesson): Path<String>) -> Redirect {
  if !auth.is_admin {
    return Redirect::to("/settings");
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: format!("/settings/delete-scraped/{}", lesson),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let path = match lesson.as_str() {
    "1" => paths::lesson_dir("lesson1"),
    "2" => paths::lesson_dir("lesson2"),
    "3" => paths::lesson_dir("lesson3"),
    _ => return Redirect::to("/settings"),
  };

  if let Err(e) = std::fs::remove_dir_all(&path) {
    tracing::warn!("Failed to remove lesson {} directory: {}", lesson, e);
  }

  Redirect::to("/settings")
}

// ============================================================================
// Segmentation Operations
// ============================================================================

/// Re-segment syllables with custom padding
#[derive(Deserialize)]
pub struct SegmentForm {
  #[serde(default = "default_segment_padding")]
  pub padding: u32,
}

fn default_segment_padding() -> u32 {
  75
}

/// Template for a single audio row (HTMX partial)
#[derive(Template)]
#[template(path = "partials/audio_row.html")]
pub struct AudioRowTemplate {
  pub lesson_id: String,
  pub row: AudioRow,
  pub show_params: bool,
  pub status_message: String, // Empty string means no message
  pub status_success: bool,
}

/// Re-segment all lessons (admin only)
pub async fn trigger_segment(auth: AuthContext, Form(form): Form<SegmentForm>) -> Html<String> {
  if !auth.is_admin {
    return Html(r#"<span class="text-red-600 dark:text-red-400">Admin access required</span>"#.to_string());
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(
    EventType::Custom {
      name: "segment_all".into(),
      data: serde_json::json!({
        "padding": form.padding,
      }),
    }
  );

  // Use --reset to ignore saved manifest params and apply CLI values
  let cmd = format!(
    "cd {} && uv run kr-scraper segment --padding {} --reset 2>&1",
    paths::PY_SCRIPTS_DIR,
    form.padding
  );

  match Command::new("sh").args(["-c", &cmd]).output() {
    Ok(output) if output.status.success() => {
      let stdout = String::from_utf8_lossy(&output.stdout);
      // Count "OK" occurrences for a rough success count
      let ok_count = stdout.matches(" OK").count();

      // Build response with status message + out-of-band row updates
      let mut html = format!(
        r#"<span class="text-green-600 dark:text-green-400">{} rows segmented with P={}ms</span>"#,
        ok_count, form.padding
      );

      // Add out-of-band swaps for all rows in all lessons
      for lesson_id in ["lesson1", "lesson2", "lesson3"] {
        if let Some(lesson_audio) = get_lesson_audio(lesson_id, "") {
          for row in lesson_audio.rows {
            let row_template = AudioRowTemplate {
              lesson_id: lesson_id.to_string(),
              row,
              show_params: false,
              status_message: String::new(),
              status_success: false,
            };
            if let Ok(row_html) = row_template.render() {
              // Wrap with hx-swap-oob to update each row in place
              html.push_str(&format!(
                r#"<div hx-swap-oob="outerHTML:#audio-row-{}-{}">{}</div>"#,
                lesson_id, row_template.row.romanization, row_html
              ));
            }
          }
        }
      }

      Html(html)
    }
    Ok(output) => {
      let stderr = String::from_utf8_lossy(&output.stderr);
      let stdout = String::from_utf8_lossy(&output.stdout);
      let error = stderr.lines().chain(stdout.lines()).next().unwrap_or("unknown error");
      Html(format!(
        r#"<span class="text-red-600 dark:text-red-400">Failed: {}</span>"#,
        error
      ))
    }
    Err(e) => Html(format!(
      r#"<span class="text-red-600 dark:text-red-400">Failed: {}</span>"#,
      e
    )),
  }
}

/// Re-segment a single row with custom parameters
#[derive(Deserialize)]
pub struct RowSegmentForm {
  pub lesson: String,
  pub row: String,
  #[serde(default = "row_default_min_silence")]
  pub min_silence: i32,
  #[serde(default = "row_default_threshold")]
  pub threshold: i32,
  #[serde(default = "row_default_padding")]
  pub padding: i32,
  #[serde(default)]
  pub skip_first: i32,
  #[serde(default)]
  pub skip_last: i32,
}

fn row_default_min_silence() -> i32 {
  200
}

fn row_default_threshold() -> i32 {
  -40
}

fn row_default_padding() -> i32 {
  75
}

/// Re-segment a single row (admin only)
pub async fn trigger_row_segment(auth: AuthContext, Form(form): Form<RowSegmentForm>) -> Html<String> {
  if !auth.is_admin {
    return Html(r#"<span class="text-red-600 dark:text-red-400">Admin access required</span>"#.to_string());
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(
    EventType::Custom {
      name: "segment_row".into(),
      data: serde_json::json!({
        "lesson": form.lesson,
        "row": form.row,
        "min_silence": form.min_silence,
        "threshold": form.threshold,
        "padding": form.padding,
        "skip_first": form.skip_first,
        "skip_last": form.skip_last,
      }),
    }
  );

  // Use the segment-row CLI command for cleaner invocation
  let cmd = format!(
    "cd {} && uv run kr-scraper segment-row {} {} -s {} -t {} -P {} --skip-first {} --skip-last {} --json",
    paths::PY_SCRIPTS_DIR,
    form.lesson,
    form.row,
    form.min_silence,
    form.threshold,
    form.padding,
    form.skip_first,
    form.skip_last
  );

  let (status_message, status_success) = match Command::new("sh").args(["-c", &cmd]).output() {
    Ok(output) if output.status.success() => {
      let stdout = String::from_utf8_lossy(&output.stdout);
      if let Ok(result) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        let saved = result["saved"].as_u64().unwrap_or(0);
        let found = result["found"].as_u64().unwrap_or(0);
        (format!("{}/{} segments", saved, found), true)
      } else {
        ("Segmented".to_string(), true)
      }
    }
    Ok(output) => {
      let stderr = String::from_utf8_lossy(&output.stderr);
      (
        format!("Failed: {}", stderr.lines().next().unwrap_or("unknown error")),
        false,
      )
    }
    Err(e) => (format!("Failed: {}", e), false),
  };

  // Re-read the updated row data from manifest
  let row_data = get_audio_row(&form.lesson, &form.row);

  let template = AudioRowTemplate {
    lesson_id: form.lesson,
    row: row_data.unwrap_or_else(|| AudioRow {
      character: form.row.clone(),
      romanization: form.row,
      syllables: vec![],
      available_count: 0,
      segments_json: "[]".to_string(),
      params: SegmentParams::default(),
    }),
    show_params: true, // Keep params visible after re-segment
    status_message,
    status_success,
  };

  Html(template.render().unwrap_or_default())
}

/// Apply manual segment timestamps
#[derive(Deserialize)]
pub struct ManualSegmentForm {
  pub lesson: String,
  pub syllable: String,      // Korean character
  pub romanization: String,  // Romanized name for audio file
  pub row: String,           // Row romanization for refreshing UI
  pub start_ms: i32,
  pub end_ms: i32,
}

/// Apply manual segment timestamps (admin only)
pub async fn trigger_manual_segment(auth: AuthContext, Form(form): Form<ManualSegmentForm>) -> Html<String> {
  if !auth.is_admin {
    return Html(r#"<span class="text-red-600 dark:text-red-400">Admin access required</span>"#.to_string());
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(
    EventType::Custom {
      name: "segment_manual".into(),
      data: serde_json::json!({
        "lesson": form.lesson,
        "syllable": form.syllable,
        "start_ms": form.start_ms,
        "end_ms": form.end_ms,
      }),
    }
  );

  // Call Python apply-manual command
  let cmd = format!(
    "cd {} && uv run kr-scraper apply-manual {} {} --start {} --end {}",
    paths::PY_SCRIPTS_DIR,
    form.lesson,
    form.syllable,
    form.start_ms,
    form.end_ms
  );

  let (status_message, status_success) = match Command::new("sh").args(["-c", &cmd]).output() {
    Ok(output) if output.status.success() => {
      ("Manual applied".to_string(), true)
    }
    Ok(output) => {
      let stderr = String::from_utf8_lossy(&output.stderr);
      (
        format!("Failed: {}", stderr.lines().next().unwrap_or("unknown error")),
        false,
      )
    }
    Err(e) => (format!("Failed: {}", e), false),
  };

  // Re-read the updated row data from manifest
  let row_data = get_audio_row(&form.lesson, &form.row);

  let template = AudioRowTemplate {
    lesson_id: form.lesson,
    row: row_data.unwrap_or_else(|| AudioRow {
      character: form.row.clone(),
      romanization: form.row,
      syllables: vec![],
      available_count: 0,
      segments_json: "[]".to_string(),
      params: SegmentParams::default(),
    }),
    show_params: false,
    status_message,
    status_success,
  };

  Html(template.render().unwrap_or_default())
}

/// Reset manual segment timestamps to baseline
#[derive(Deserialize)]
pub struct ResetSegmentForm {
  pub lesson: String,
  pub syllable: String,      // Korean character
  pub romanization: String,  // Romanized name for audio file
  pub row: String,           // Row romanization for refreshing UI
}

/// Reset manual segment timestamps to baseline (admin only)
pub async fn trigger_reset_segment(auth: AuthContext, Form(form): Form<ResetSegmentForm>) -> Html<String> {
  if !auth.is_admin {
    return Html(r#"<span class="text-red-600 dark:text-red-400">Admin access required</span>"#.to_string());
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(
    EventType::Custom {
      name: "segment_reset".into(),
      data: serde_json::json!({
        "lesson": form.lesson,
        "syllable": form.syllable,
      }),
    }
  );

  // Call Python reset-manual command
  let cmd = format!(
    "cd {} && uv run kr-scraper reset-manual {} {}",
    paths::PY_SCRIPTS_DIR,
    form.lesson,
    form.syllable
  );

  let (status_message, status_success) = match Command::new("sh").args(["-c", &cmd]).output() {
    Ok(output) if output.status.success() => {
      ("Reset to baseline".to_string(), true)
    }
    Ok(output) => {
      let stderr = String::from_utf8_lossy(&output.stderr);
      (
        format!("Failed: {}", stderr.lines().next().unwrap_or("unknown error")),
        false,
      )
    }
    Err(e) => (format!("Failed: {}", e), false),
  };

  // Re-read the updated row data from manifest
  let row_data = get_audio_row(&form.lesson, &form.row);

  let template = AudioRowTemplate {
    lesson_id: form.lesson,
    row: row_data.unwrap_or_else(|| AudioRow {
      character: form.row.clone(),
      romanization: form.row,
      syllables: vec![],
      available_count: 0,
      segments_json: "[]".to_string(),
      params: SegmentParams::default(),
    }),
    show_params: false,
    status_message,
    status_success,
  };

  Html(template.render().unwrap_or_default())
}

// ============================================================================
// Learning State Operations
// ============================================================================

/// Make all cards due now for accelerated learning/testing
pub async fn make_all_due(auth: AuthContext) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/make-all-due".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/settings"),
  };
  let _count = db::make_all_cards_due(&conn).log_warn_default("Failed to make all cards due");

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::Custom {
    name: "make_all_due".into(),
    data: serde_json::json!({ "cards_updated": _count }),
  });

  Redirect::to("/settings")
}

/// Graduate all cards in a tier (escape hatch for users who know the material)
pub async fn graduate_tier(auth: AuthContext, Path(tier): Path<u8>) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: format!("/settings/graduate-tier/{}", tier),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/settings"),
  };

  let _count = db::graduate_tier(&conn, tier).log_warn_default("Failed to graduate tier");

  // Try to unlock next tier if applicable
  db::try_auto_unlock_tier(&conn).log_warn("Failed to auto-unlock next tier");

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::Custom {
    name: "graduate_tier".into(),
    data: serde_json::json!({ "tier": tier, "cards_graduated": _count }),
  });

  Redirect::to("/settings")
}

/// Restore a tier to its pre-graduation state (undo graduation)
pub async fn restore_tier(auth: AuthContext, Path(tier): Path<u8>) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: format!("/settings/restore-tier/{}", tier),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/settings"),
  };

  let _count = db::restore_tier_state(&conn, tier).log_warn_default("Failed to restore tier");

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::Custom {
    name: "restore_tier".into(),
    data: serde_json::json!({ "tier": tier, "cards_restored": _count }),
  });

  Redirect::to("/settings")
}

// ============================================================================
// Guest Management (Admin Only)
// ============================================================================

/// Cleanup expired guest accounts
pub async fn cleanup_guests(auth: AuthContext, State(state): State<AppState>) -> Redirect {
  if !auth.is_admin {
    return Redirect::to("/settings");
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/cleanup-guests".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/settings"),
  };

  let expiry_hours = auth_db::get_guest_expiry_hours(&auth_db).unwrap_or(24);
  if let Ok(expired_usernames) = auth_db::cleanup_expired_guests(&auth_db, expiry_hours) {
    for username in &expired_usernames {
      let user_dir = state.user_dir(username);
      let _ = std::fs::remove_dir_all(&user_dir);
    }
    tracing::info!("Cleaned up {} expired guest accounts", expired_usernames.len());
  }

  Redirect::to("/settings")
}

/// Delete all guest accounts
pub async fn delete_all_guests(auth: AuthContext, State(state): State<AppState>) -> Redirect {
  if !auth.is_admin {
    return Redirect::to("/settings");
  }

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/delete-all-guests".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/settings"),
  };

  if let Ok(deleted_usernames) = auth_db::delete_all_guests(&auth_db) {
    for username in &deleted_usernames {
      let user_dir = state.user_dir(username);
      let _ = std::fs::remove_dir_all(&user_dir);
    }
    tracing::info!("Deleted {} guest accounts", deleted_usernames.len());
  }

  Redirect::to("/settings")
}

// ============================================================================
// HTMX Templates for async updates
// ============================================================================

/// Check if request is from HTMX
fn is_htmx_request(headers: &HeaderMap) -> bool {
  headers.get("HX-Request").is_some()
}

/// Group card partial template
#[derive(Template)]
#[template(path = "partials/settings_group_card.html")]
pub struct GroupCardTemplate {
  pub group: GroupDisplay,
  pub users: Vec<UserDisplay>,
}

/// Pack permission panel partial template
#[derive(Template)]
#[template(path = "partials/settings_pack_permissions.html")]
pub struct PackPermissionsTemplate {
  pub pack: PackInfo,
  pub groups: Vec<GroupDisplay>,
  pub users: Vec<UserDisplay>,
}

/// Helper to render pack permissions partial for HTMX response
fn render_pack_permissions(conn: &rusqlite::Connection, pack_id: &str) -> Option<String> {
  use std::path::Path;

  // Get all discovered packs to find this one
  let shared_packs_dir = Path::new(paths::SHARED_PACKS_DIR);
  let packs = discover_packs(shared_packs_dir, None, None);
  let pack_loc = packs.iter().find(|p| p.manifest.id == pack_id)?;

  // Build PackInfo
  let allowed_groups = auth_db::get_pack_allowed_groups(conn, pack_id).unwrap_or_default();
  let allowed_users_raw = auth_db::get_pack_allowed_users(conn, pack_id).unwrap_or_default();
  let allowed_users: Vec<AllowedUser> = allowed_users_raw
    .into_iter()
    .map(|(id, username)| AllowedUser { id, username })
    .collect();
  let is_restricted = !allowed_groups.is_empty() || !allowed_users.is_empty();
  let is_baseline = pack_loc.manifest.id == "baseline";

  let pack = PackInfo {
    id: pack_loc.manifest.id.clone(),
    name: pack_loc.manifest.name.clone(),
    description: pack_loc.manifest.description.clone(),
    version: pack_loc.manifest.version.clone(),
    pack_type: match pack_loc.manifest.pack_type {
      PackType::Cards => "cards".to_string(),
      PackType::Audio => "audio".to_string(),
      PackType::Generator => "generator".to_string(),
    },
    is_enabled: false, // Not needed for permissions partial
    is_baseline,
    cards_count: None,
    is_restricted,
    allowed_groups,
    allowed_users,
  };

  // Get groups for the dropdown
  let groups: Vec<GroupDisplay> = auth_db::get_all_groups(conn)
    .unwrap_or_default()
    .into_iter()
    .map(|g| {
      let members = auth_db::get_group_members(conn, &g.id)
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

  // Get users for the dropdown
  let users: Vec<UserDisplay> = auth_db::get_all_users(conn)
    .unwrap_or_default()
    .into_iter()
    .map(|u| UserDisplay {
      id: u.id,
      username: u.username,
      role: u.role,
      is_guest: u.is_guest,
    })
    .collect();

  let template = PackPermissionsTemplate { pack, groups, users };
  template.render().ok()
}

// ============================================================================
// User Role Management (Admin Only)
// ============================================================================

#[derive(Deserialize)]
pub struct SetRoleForm {
  pub user_id: i64,
  pub role: String,
}

/// User row partial template
#[derive(Template)]
#[template(path = "partials/settings_user_row.html")]
pub struct UserRowTemplate {
  pub user: UserDisplay,
}

/// Change a user's role (admin only)
pub async fn set_user_role(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<SetRoleForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  // Validate role
  if form.role != "user" && form.role != "admin" {
    if is_htmx_request(&headers) {
      return Html(error_notification("Invalid role")).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  if let Err(e) = auth_db::set_user_role(&auth_db, form.user_id, &form.role) {
    tracing::warn!("Failed to set user role: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to set role: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    // Fetch updated user info
    if let Ok(Some(user)) = auth_db::get_user_by_id(&auth_db, form.user_id) {
      let template = UserRowTemplate {
        user: UserDisplay {
          id: user.id,
          username: user.username,
          role: user.role,
          is_guest: user.is_guest,
        },
      };
      return Html(template.render().unwrap_or_default()).into_response();
    }
  }

  Redirect::to("/settings").into_response()
}

// ============================================================================
// User Group Management (Admin Only)
// ============================================================================

#[derive(Deserialize)]
pub struct CreateGroupForm {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
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

/// Helper to create a success notification HTML
fn success_notification(message: &str) -> String {
  format!(
    r#"<div id="notifications" hx-swap-oob="innerHTML:#notifications">
      <div class="p-4 mb-4 text-sm text-green-700 bg-green-100 dark:bg-green-900/30 dark:text-green-300 rounded-lg flex items-center justify-between" role="alert">
        <span>{}</span>
        <button type="button" onclick="this.parentElement.remove()" class="ml-4 text-green-700 dark:text-green-300 hover:text-green-900 dark:hover:text-green-100">&times;</button>
      </div>
    </div>"#,
    message
  )
}

/// Create a new user group (admin only)
pub async fn create_group(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<CreateGroupForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  // Validate group ID
  if form.id.trim().is_empty() || form.name.trim().is_empty() {
    if is_htmx_request(&headers) {
      return Html(error_notification("Group ID and name are required")).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  if let Err(e) = auth_db::create_user_group(&auth_db, &form.id, &form.name, form.description.as_deref()) {
    tracing::warn!("Failed to create group: {}", e);
    if is_htmx_request(&headers) {
      let msg = if e.to_string().contains("UNIQUE constraint") {
        format!("Group '{}' already exists", form.id)
      } else {
        format!("Failed to create group: {}", e)
      };
      return Html(error_notification(&msg)).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    // Render the new group card plus remove the "no groups" message if present
    if let Some(html) = render_group_card(&auth_db, &form.id) {
      let mut response = html;
      // Remove "no groups" message via OOB swap
      response.push_str(r#"<p id="no-groups-msg" hx-swap-oob="delete"></p>"#);
      return Html(response).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

/// Delete a user group (admin only)
pub async fn delete_group(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(group_id): Path<String>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  if let Err(e) = auth_db::delete_group(&auth_db, &group_id) {
    tracing::warn!("Failed to delete group: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to delete group: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return empty response for HTMX (element will be removed) or redirect
  if is_htmx_request(&headers) {
    return Html("").into_response();
  }
  Redirect::to("/settings").into_response()
}

#[derive(Deserialize)]
pub struct GroupMemberForm {
  pub user_id: i64,
  pub group_id: String,
}

/// Helper to render a group card for HTMX response
fn render_group_card(conn: &rusqlite::Connection, group_id: &str) -> Option<String> {
  let group = auth_db::get_group(conn, group_id).ok()??;
  let members = auth_db::get_group_members(conn, group_id)
    .unwrap_or_default()
    .into_iter()
    .map(|(id, username)| GroupMember { id, username })
    .collect();
  let group_display = GroupDisplay {
    id: group.id,
    name: group.name,
    description: group.description,
    members,
  };
  let users: Vec<UserDisplay> = auth_db::get_all_users(conn)
    .unwrap_or_default()
    .into_iter()
    .map(|u| UserDisplay {
      id: u.id,
      username: u.username,
      role: u.role,
      is_guest: u.is_guest,
    })
    .collect();

  let template = GroupCardTemplate { group: group_display, users };
  template.render().ok()
}

/// Add a user to a group (admin only)
pub async fn add_to_group(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<GroupMemberForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  if let Err(e) = auth_db::add_user_to_group(&auth_db, form.user_id, &form.group_id) {
    tracing::warn!("Failed to add user to group: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to add user: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_group_card(&auth_db, &form.group_id) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

/// Remove a user from a group (admin only)
pub async fn remove_from_group(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<GroupMemberForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  if let Err(e) = auth_db::remove_user_from_group(&auth_db, form.user_id, &form.group_id) {
    tracing::warn!("Failed to remove user from group: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to remove user: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_group_card(&auth_db, &form.group_id) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

// ============================================================================
// Pack Permissions (Admin Only)
// ============================================================================

#[derive(Deserialize)]
pub struct PackPermissionForm {
  pub pack_id: String,
  pub group_id: String,  // Empty string = all users
}

#[derive(Deserialize)]
pub struct PackUserPermissionForm {
  pub pack_id: String,
  pub user_id: i64,
}

/// Restrict a pack to specific groups (admin only)
pub async fn restrict_pack_to_group(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<PackPermissionForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  // Set permission: this group can access this pack
  if let Err(e) = auth_db::set_pack_permission(&auth_db, &form.pack_id, &form.group_id, true) {
    tracing::warn!("Failed to set pack permission: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to add group access: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_pack_permissions(&auth_db, &form.pack_id) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

/// Remove pack restriction for a group (admin only)
pub async fn remove_pack_restriction(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<PackPermissionForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  if let Err(e) = auth_db::remove_pack_permission(&auth_db, &form.pack_id, &form.group_id) {
    tracing::warn!("Failed to remove pack permission: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to remove group access: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_pack_permissions(&auth_db, &form.pack_id) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

/// Make a pack available to all users (remove all restrictions)
pub async fn make_pack_public(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(pack_id): Path<String>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  // Remove all permissions for this pack (makes it available to all)
  if let Err(e) = auth_db::clear_pack_permissions(&auth_db, &pack_id) {
    tracing::warn!("Failed to clear pack permissions: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to make pack public: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_pack_permissions(&auth_db, &pack_id) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

/// Restrict a pack to a specific user (admin only)
pub async fn restrict_pack_to_user(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<PackUserPermissionForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  // Set permission: this user can access this pack
  if let Err(e) = auth_db::set_pack_user_permission(&auth_db, &form.pack_id, form.user_id, true) {
    tracing::warn!("Failed to set pack user permission: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to add user access: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_pack_permissions(&auth_db, &form.pack_id) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

/// Remove pack restriction for a user (admin only)
pub async fn remove_pack_user_restriction(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<PackUserPermissionForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  if let Err(e) = auth_db::remove_pack_user_permission(&auth_db, &form.pack_id, form.user_id) {
    tracing::warn!("Failed to remove pack user permission: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to remove user access: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_pack_permissions(&auth_db, &form.pack_id) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

// ============================================================================
// External Pack Path Registration (Admin Only)
// ============================================================================

use crate::content::count_packs_in_directory;

/// Directory entry for browser
#[derive(Clone)]
pub struct DirectoryEntry {
  pub name: String,
  pub path: String,
  pub is_dir: bool,
  pub has_packs: bool, // Contains pack.json files (for highlighting)
}

/// Template for directory browser (HTMX partial)
#[derive(Template)]
#[template(path = "partials/settings_directory_browser.html")]
pub struct DirectoryBrowserTemplate {
  pub current_path: String,
  pub parent_path: Option<String>,
  pub entries: Vec<DirectoryEntry>,
  pub error: Option<String>,
}

/// Browse directories on the server (admin only)
pub async fn browse_directories(
  auth: AuthContext,
  headers: HeaderMap,
  Form(form): Form<BrowseDirectoryForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let path = if form.path.trim().is_empty() {
    // Start at user's home or root
    std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
  } else {
    form.path.trim().to_string()
  };

  let path_buf = std::path::Path::new(&path);

  // Compute parent path
  let parent_path = path_buf.parent().map(|p| p.to_string_lossy().to_string());

  // Read directory entries
  let (entries, error) = match std::fs::read_dir(&path) {
    Ok(read_dir) => {
      let mut dirs: Vec<DirectoryEntry> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| {
          // Skip hidden directories (starting with .)
          e.file_name().to_string_lossy().chars().next() != Some('.')
        })
        .map(|e| {
          let entry_path = e.path();
          let has_packs = count_packs_in_directory(&entry_path) > 0;
          DirectoryEntry {
            name: e.file_name().to_string_lossy().to_string(),
            path: entry_path.to_string_lossy().to_string(),
            is_dir: true,
            has_packs,
          }
        })
        .collect();
      dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
      (dirs, None)
    }
    Err(e) => (Vec::new(), Some(format!("Cannot read directory: {}", e))),
  };

  let template = DirectoryBrowserTemplate {
    current_path: path,
    parent_path,
    entries,
    error,
  };

  if is_htmx_request(&headers) {
    Html(template.render().unwrap_or_default()).into_response()
  } else {
    Redirect::to("/settings").into_response()
  }
}

#[derive(Deserialize)]
pub struct BrowseDirectoryForm {
  #[serde(default)]
  pub path: String,
}

/// Form for registering a new pack path
#[derive(Deserialize)]
pub struct RegisterPackPathForm {
  pub path: String,
  pub name: Option<String>,
}

/// Display info for a registered pack path
#[derive(Clone)]
pub struct RegisteredPathDisplay {
  pub id: i64,
  pub path: String,
  pub name: Option<String>,
  pub registered_by: String,
  pub is_active: bool,
  pub pack_count: usize,
}

/// Template for registered paths list (HTMX partial)
#[derive(Template)]
#[template(path = "partials/settings_registered_paths.html")]
pub struct RegisteredPathsTemplate {
  pub paths: Vec<RegisteredPathDisplay>,
}

/// Helper to render registered paths partial
pub fn render_registered_paths(conn: &rusqlite::Connection) -> Option<String> {
  let db_paths = auth_db::get_all_registered_paths(conn).ok()?;
  let paths: Vec<RegisteredPathDisplay> = db_paths
    .into_iter()
    .map(|p| {
      let pack_count = count_packs_in_directory(std::path::Path::new(&p.path));
      RegisteredPathDisplay {
        id: p.id,
        path: p.path,
        name: p.name,
        registered_by: p.registered_by,
        is_active: p.is_active,
        pack_count,
      }
    })
    .collect();

  let template = RegisteredPathsTemplate { paths };
  template.render().ok()
}

/// Register a new external pack path (admin only)
pub async fn register_pack_path(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<RegisterPackPathForm>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let path = form.path.trim();
  if path.is_empty() {
    if is_htmx_request(&headers) {
      return Html(error_notification("Path is required")).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Validate path exists and is a directory
  let path_buf = std::path::Path::new(path);
  if !path_buf.exists() {
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Path does not exist: {}", path))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }
  if !path_buf.is_dir() {
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Path is not a directory: {}", path))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Check for valid packs
  let pack_count = count_packs_in_directory(path_buf);
  if pack_count == 0 {
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("No valid packs found in: {}", path))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  // Check if already registered
  if auth_db::is_path_registered(&auth_db, path).unwrap_or(false) {
    if is_htmx_request(&headers) {
      return Html(error_notification("Path is already registered")).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Register the path
  let name = form.name.as_deref().filter(|s| !s.trim().is_empty());
  if let Err(e) = auth_db::register_pack_path(&auth_db, path, name, &auth.username) {
    tracing::warn!("Failed to register pack path: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to register path: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_registered_paths(&auth_db) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

/// Unregister (delete) a pack path (admin only)
pub async fn unregister_pack_path(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<i64>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  if let Err(e) = auth_db::unregister_pack_path(&auth_db, id) {
    tracing::warn!("Failed to unregister pack path: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to remove path: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_registered_paths(&auth_db) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}

/// Toggle a pack path's active status (admin only)
pub async fn toggle_pack_path(
  auth: AuthContext,
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(id): Path<i64>,
) -> Response {
  if !auth.is_admin {
    return Redirect::to("/settings").into_response();
  }

  let auth_db = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => {
      if is_htmx_request(&headers) {
        return Html(error_notification("Database error")).into_response();
      }
      return Redirect::to("/settings").into_response();
    }
  };

  if let Err(e) = auth_db::toggle_pack_path_active(&auth_db, id) {
    tracing::warn!("Failed to toggle pack path: {}", e);
    if is_htmx_request(&headers) {
      return Html(error_notification(&format!("Failed to toggle path: {}", e))).into_response();
    }
    return Redirect::to("/settings").into_response();
  }

  // Return HTMX partial or redirect
  if is_htmx_request(&headers) {
    if let Some(html) = render_registered_paths(&auth_db) {
      return Html(html).into_response();
    }
  }
  Redirect::to("/settings").into_response()
}
