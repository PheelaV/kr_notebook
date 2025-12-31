//! Admin-only operations: scraper, segmentation, tier graduation, guest management.

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, Redirect};
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
