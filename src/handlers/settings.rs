use askama::Template;
use axum::{
  extract::{Path, State},
  response::{Html, Redirect},
  Form,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path as StdPath;
use std::process::Command;

use crate::db::{self, try_lock, DbPool, LogOnError};
use crate::paths;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// Check if lesson1 content exists
pub fn has_lesson1() -> bool {
  StdPath::new(&paths::manifest_path("lesson1")).exists()
}

/// Check if lesson2 content exists
pub fn has_lesson2() -> bool {
  StdPath::new(&paths::manifest_path("lesson2")).exists()
}

/// Count segmented syllables for a lesson
fn count_syllables(lesson: &str) -> usize {
  let path = paths::syllables_dir(lesson);
  std::fs::read_dir(path)
    .map(|entries| {
      entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "mp3").unwrap_or(false))
        .count()
    })
    .unwrap_or(0)
}

/// Segmentation parameters for a row
pub struct SegmentParams {
  pub min_silence: i32,
  pub threshold: i32,
  pub padding: i32,
  pub skip_first: i32,
  pub skip_last: i32,
}

impl Default for SegmentParams {
  fn default() -> Self {
    Self {
      min_silence: 200,
      threshold: -40,
      padding: 75,
      skip_first: 0,
      skip_last: 0,
    }
  }
}

/// Audio row info for preview
pub struct AudioRow {
  pub character: String,
  pub romanization: String,
  pub syllables: Vec<String>,            // Syllable romanizations
  pub available_segments: Vec<String>,   // Which segments have audio files
  pub segments_json: String,             // JSON array for JS
  pub params: SegmentParams,             // Current segmentation parameters
}

/// Lesson audio preview data
pub struct LessonAudio {
  pub lesson_id: String,
  pub lesson_name: String,
  pub rows: Vec<AudioRow>,
  pub has_columns: bool,  // Lesson 1 has column audio
}

/// Get audio preview data for a lesson
fn get_lesson_audio(lesson_id: &str, lesson_name: &str) -> Option<LessonAudio> {
  let manifest_path = paths::manifest_path(lesson_id);
  let manifest_content = fs::read_to_string(&manifest_path).ok()?;
  let manifest: serde_json::Value = serde_json::from_str(&manifest_content).ok()?;

  // Get available syllable files
  let syllables_dir = paths::syllables_dir(lesson_id);
  let available_segments: HashSet<String> = fs::read_dir(&syllables_dir)
    .map(|entries| {
      entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
          let path = e.path();
          if path.extension().map(|ext| ext == "mp3").unwrap_or(false) {
            path.file_stem().and_then(|s| s.to_str()).map(String::from)
          } else {
            None
          }
        })
        .collect()
    })
    .unwrap_or_default();

  let rows_data = manifest.get("rows")?;
  let syllable_table = manifest.get("syllable_table")?;
  let consonants_order: Vec<String> = manifest["consonants_order"]
    .as_array()
    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    .unwrap_or_default();

  let mut rows = Vec::new();
  for consonant in consonants_order {
    if let Some(row) = rows_data.get(&consonant) {
      let romanization = row["romanization"].as_str().unwrap_or("").to_string();
      let syllables: Vec<String> = row["syllables"]
        .as_array()
        .map(|arr| {
          arr.iter()
            .filter_map(|s| {
              let syllable_char = s.as_str()?;
              syllable_table
                .get(syllable_char)
                .and_then(|st| st["romanization"].as_str())
                .map(String::from)
            })
            .collect()
        })
        .unwrap_or_default();

      let available: Vec<String> = syllables
        .iter()
        .filter(|s| available_segments.contains(*s))
        .cloned()
        .collect();

      let segments_json = serde_json::to_string(&available).unwrap_or_else(|_| "[]".to_string());

      // Read segment_params from manifest
      let segment_params = row.get("segment_params");
      let params = SegmentParams {
        min_silence: segment_params
          .and_then(|p| p.get("min_silence"))
          .and_then(|v| v.as_i64())
          .unwrap_or(200) as i32,
        threshold: segment_params
          .and_then(|p| p.get("threshold"))
          .and_then(|v| v.as_i64())
          .unwrap_or(-40) as i32,
        padding: segment_params
          .and_then(|p| p.get("padding"))
          .and_then(|v| v.as_i64())
          .unwrap_or(75) as i32,
        skip_first: segment_params
          .and_then(|p| p.get("skip_first"))
          .and_then(|v| v.as_i64())
          .unwrap_or(0) as i32,
        skip_last: segment_params
          .and_then(|p| p.get("skip_last"))
          .and_then(|v| v.as_i64())
          .unwrap_or(0) as i32,
      };

      rows.push(AudioRow {
        character: consonant,
        romanization,
        syllables,
        available_segments: available,
        segments_json,
        params,
      });
    }
  }

  let has_columns = manifest.get("columns").map(|c| !c.is_null()).unwrap_or(false);

  Some(LessonAudio {
    lesson_id: lesson_id.to_string(),
    lesson_name: lesson_name.to_string(),
    rows,
    has_columns,
  })
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
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
  pub lesson1_syllables: usize,
  pub lesson2_syllables: usize,
  // Audio preview data
  pub lesson_audio: Vec<LessonAudio>,
}

/// Error HTML for database unavailable
const DB_ERROR_HTML: &str = r#"<!DOCTYPE html><html><head><title>Error</title></head><body><h1>Database Error</h1><p>Please refresh the page.</p></body></html>"#;

pub async fn settings_page(State(pool): State<DbPool>) -> Html<String> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings".into(),
    method: "GET".into(),
  });

  let conn = match try_lock(&pool) {
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
  let scraped_content_available = has_l1 || has_l2;

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

  let template = SettingsTemplate {
    all_tiers_unlocked,
    enabled_tiers,
    desired_retention,
    focus_tier,
    max_unlocked_tier,
    has_scraped_content: scraped_content_available,
    has_pronunciation: scraped_content_available,
    has_lesson1: has_l1,
    has_lesson2: has_l2,
    lesson1_syllables: if has_l1 { count_syllables("lesson1") } else { 0 },
    lesson2_syllables: if has_l2 { count_syllables("lesson2") } else { 0 },
    lesson_audio,
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
  State(pool): State<DbPool>,
  Form(form): Form<SettingsForm>,
) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings".into(),
    method: "POST".into(),
  });

  let conn = match try_lock(&pool) {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/settings"),
  };

  // Update all_tiers_unlocked
  let all_tiers_unlocked = form.all_tiers_unlocked.is_some();
  let _ = db::set_all_tiers_unlocked(&conn, all_tiers_unlocked);

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::SettingsUpdate {
    setting: "all_tiers_unlocked".into(),
    value: all_tiers_unlocked.to_string(),
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

  let _ = db::set_enabled_tiers(&conn, &enabled_tiers);

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::SettingsUpdate {
    setting: "enabled_tiers".into(),
    value: format!("{:?}", enabled_tiers),
  });

  // Update desired retention if provided
  if let Some(retention) = form.desired_retention {
    // Validate and clamp to valid range
    let retention_pct = retention.clamp(80, 95);
    let retention_f64 = f64::from(retention_pct) / 100.0;
    let _ = db::set_setting(&conn, "desired_retention", &retention_f64.to_string());

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::SettingsUpdate {
      setting: "desired_retention".into(),
      value: retention_f64.to_string(),
    });
  }

  // Update focus tier if provided
  if let Some(focus_str) = form.focus_tier {
    let focus_tier = if focus_str == "none" || focus_str.is_empty() {
      None
    } else {
      focus_str.parse::<u8>().ok()
    };
    let _ = db::set_focus_tier(&conn, focus_tier);

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::SettingsUpdate {
      setting: "focus_tier".into(),
      value: focus_tier.map(|t| t.to_string()).unwrap_or_else(|| "none".to_string()),
    });
  }

  Redirect::to("/settings")
}

/// Scrape all lessons
pub async fn trigger_scrape() -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/scrape".into(),
    method: "POST".into(),
  });

  // Run the scraper commands for all lessons
  let cmd = format!(
    "cd {} && uv run kr-scraper lesson1 && uv run kr-scraper lesson2 && uv run kr-scraper segment --padding 75",
    paths::PY_SCRIPTS_DIR
  );
  let _ = Command::new("sh").args(["-c", &cmd]).output();

  Redirect::to("/settings")
}

/// Scrape a specific lesson
pub async fn trigger_scrape_lesson(Path(lesson): Path<String>) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: format!("/settings/scrape/{}", lesson).into(),
    method: "POST".into(),
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
    _ => return Redirect::to("/settings"),
  };

  let _ = Command::new("sh").args(["-c", &cmd]).output();

  Redirect::to("/settings")
}

/// Delete all scraped content
pub async fn delete_scraped() -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/delete-scraped".into(),
    method: "POST".into(),
  });

  // Run the clean command
  let cmd = format!("cd {} && uv run kr-scraper clean --yes", paths::PY_SCRIPTS_DIR);
  let _ = Command::new("sh").args(["-c", &cmd]).output();

  Redirect::to("/settings")
}

/// Delete a specific lesson's content
pub async fn delete_scraped_lesson(Path(lesson): Path<String>) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: format!("/settings/delete-scraped/{}", lesson).into(),
    method: "POST".into(),
  });

  let path = match lesson.as_str() {
    "1" => paths::lesson_dir("lesson1"),
    "2" => paths::lesson_dir("lesson2"),
    _ => return Redirect::to("/settings"),
  };

  let _ = std::fs::remove_dir_all(path);

  Redirect::to("/settings")
}

/// Re-segment syllables with custom padding
#[derive(Deserialize)]
pub struct SegmentForm {
  #[serde(default = "default_segment_padding")]
  pub padding: u32,
}

fn default_segment_padding() -> u32 {
  75
}

pub async fn trigger_segment(Form(form): Form<SegmentForm>) -> Html<String> {
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

      // Add out-of-band swaps for all rows in both lessons
      for lesson_id in ["lesson1", "lesson2"] {
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

pub async fn trigger_row_segment(Form(form): Form<RowSegmentForm>) -> Html<String> {
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
      available_segments: vec![],
      segments_json: "[]".to_string(),
      params: SegmentParams::default(),
    }),
    show_params: true, // Keep params visible after re-segment
    status_message,
    status_success,
  };

  Html(template.render().unwrap_or_default())
}

/// Get a single audio row from the manifest
fn get_audio_row(lesson_id: &str, row_romanization: &str) -> Option<AudioRow> {
  let manifest_path = paths::manifest_path(lesson_id);
  let manifest_content = fs::read_to_string(&manifest_path).ok()?;
  let manifest: serde_json::Value = serde_json::from_str(&manifest_content).ok()?;

  // Get available syllable files
  let syllables_dir = paths::syllables_dir(lesson_id);
  let available_segments: HashSet<String> = fs::read_dir(&syllables_dir)
    .map(|entries| {
      entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
          let path = e.path();
          if path.extension().map(|ext| ext == "mp3").unwrap_or(false) {
            path.file_stem().and_then(|s| s.to_str()).map(String::from)
          } else {
            None
          }
        })
        .collect()
    })
    .unwrap_or_default();

  let rows = manifest.get("rows")?;
  let syllable_table = manifest.get("syllable_table")?;

  // Find the row by romanization
  for (char, info) in rows.as_object()?.iter() {
    let romanization = info["romanization"].as_str().unwrap_or("");
    if romanization != row_romanization {
      continue;
    }

    let syllables: Vec<String> = info["syllables"]
      .as_array()
      .map(|arr| {
        arr
          .iter()
          .filter_map(|s| {
            let syllable_char = s.as_str()?;
            syllable_table
              .get(syllable_char)
              .and_then(|st| st["romanization"].as_str())
              .map(String::from)
          })
          .collect()
      })
      .unwrap_or_default();

    let available: Vec<String> = syllables
      .iter()
      .filter(|s| available_segments.contains(*s))
      .cloned()
      .collect();

    let segments_json = serde_json::to_string(&available).unwrap_or_else(|_| "[]".to_string());

    let segment_params = info.get("segment_params");
    let params = SegmentParams {
      min_silence: segment_params
        .and_then(|p| p.get("min_silence"))
        .and_then(|v| v.as_i64())
        .unwrap_or(200) as i32,
      threshold: segment_params
        .and_then(|p| p.get("threshold"))
        .and_then(|v| v.as_i64())
        .unwrap_or(-40) as i32,
      padding: segment_params
        .and_then(|p| p.get("padding"))
        .and_then(|v| v.as_i64())
        .unwrap_or(75) as i32,
      skip_first: segment_params
        .and_then(|p| p.get("skip_first"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32,
      skip_last: segment_params
        .and_then(|p| p.get("skip_last"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32,
    };

    return Some(AudioRow {
      character: char.clone(),
      romanization: romanization.to_string(),
      syllables,
      available_segments: available,
      segments_json,
      params,
    });
  }

  None
}

/// Make all cards due now for accelerated learning/testing
pub async fn make_all_due(State(pool): State<DbPool>) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/settings/make-all-due".into(),
    method: "POST".into(),
  });

  let conn = match try_lock(&pool) {
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
