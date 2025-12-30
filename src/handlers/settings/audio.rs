//! Audio data models and manifest parsing for lesson content.

use serde_json;
use std::collections::HashSet;
use std::fs;

use crate::paths;

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

/// Syllable info for preview (Korean char + romanization + has audio + timestamps)
pub struct SyllablePreview {
  pub korean: String,
  pub romanization: String,
  pub has_audio: bool,
  // Baseline timestamps from automatic segmentation
  pub baseline_start_ms: Option<i32>,
  pub baseline_end_ms: Option<i32>,
  // Manual override timestamps (if user adjusted)
  pub manual_start_ms: Option<i32>,
  pub manual_end_ms: Option<i32>,
}

/// Audio row info for preview
pub struct AudioRow {
  pub character: String,
  pub romanization: String,
  pub syllables: Vec<SyllablePreview>,   // All syllables with Korean + romanization
  pub available_count: usize,            // Count of syllables with audio
  pub segments_json: String,             // JSON array for JS (available segments only)
  pub params: SegmentParams,             // Current segmentation parameters
}

/// Lesson audio preview data
pub struct LessonAudio {
  pub lesson_id: String,
  pub lesson_name: String,
  pub rows: Vec<AudioRow>,
  pub has_columns: bool,  // Lesson 1 has column audio
}

/// Tier graduation status for UI
pub struct TierGraduationStatus {
  pub tier: u8,
  pub is_fully_graduated: bool,
  pub has_backup: bool,
}

/// Get audio preview data for a lesson
pub fn get_lesson_audio(lesson_id: &str, lesson_name: &str) -> Option<LessonAudio> {
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

  // Try consonants_order first (lesson1, lesson2), then vowels_order (lesson3)
  let row_keys: Vec<String> = manifest["consonants_order"]
    .as_array()
    .or_else(|| manifest["vowels_order"].as_array())
    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    .unwrap_or_default();

  let mut rows = Vec::new();
  for row_key in row_keys {
    if let Some(row) = rows_data.get(&row_key) {
      let romanization = row["romanization"].as_str().unwrap_or("").to_string();

      // Build syllables with Korean char, romanization, audio availability, and timestamps
      let syllables: Vec<SyllablePreview> = row["syllables"]
        .as_array()
        .map(|arr| {
          arr.iter()
            .filter_map(|s| {
              let korean = s.as_str()?.to_string();
              let syllable_info = syllable_table.get(&korean)?;
              let rom = syllable_info["romanization"].as_str().unwrap_or("").to_string();
              let has_audio = available_segments.contains(&rom);

              // Extract timestamps from segment field
              let segment = syllable_info.get("segment");
              let baseline = segment.and_then(|s| s.get("baseline"));
              let manual = segment.and_then(|s| s.get("manual"));

              let baseline_start_ms = baseline
                .and_then(|b| b.get("start_ms"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
              let baseline_end_ms = baseline
                .and_then(|b| b.get("end_ms"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
              let manual_start_ms = manual
                .and_then(|m| m.get("start_ms"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
              let manual_end_ms = manual
                .and_then(|m| m.get("end_ms"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);

              Some(SyllablePreview {
                korean,
                romanization: rom,
                has_audio,
                baseline_start_ms,
                baseline_end_ms,
                manual_start_ms,
                manual_end_ms,
              })
            })
            .collect()
        })
        .unwrap_or_default();

      // Build available segments list for JS playback
      let available: Vec<String> = syllables
        .iter()
        .filter(|s| s.has_audio)
        .map(|s| s.romanization.clone())
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

      let available_count = syllables.iter().filter(|s| s.has_audio).count();

      rows.push(AudioRow {
        character: row_key,
        romanization,
        syllables,
        available_count,
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

/// Get a single audio row from the manifest
pub fn get_audio_row(lesson_id: &str, row_romanization: &str) -> Option<AudioRow> {
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

    // Build syllables with Korean char, romanization, audio availability, and timestamps
    let syllables: Vec<SyllablePreview> = info["syllables"]
      .as_array()
      .map(|arr| {
        arr
          .iter()
          .filter_map(|s| {
            let korean = s.as_str()?.to_string();
            let syllable_info = syllable_table.get(&korean)?;
            let rom = syllable_info["romanization"].as_str().unwrap_or("").to_string();
            let has_audio = available_segments.contains(&rom);

            // Extract timestamps from segment field
            let segment = syllable_info.get("segment");
            let baseline = segment.and_then(|s| s.get("baseline"));
            let manual = segment.and_then(|s| s.get("manual"));

            let baseline_start_ms = baseline
              .and_then(|b| b.get("start_ms"))
              .and_then(|v| v.as_i64())
              .map(|v| v as i32);
            let baseline_end_ms = baseline
              .and_then(|b| b.get("end_ms"))
              .and_then(|v| v.as_i64())
              .map(|v| v as i32);
            let manual_start_ms = manual
              .and_then(|m| m.get("start_ms"))
              .and_then(|v| v.as_i64())
              .map(|v| v as i32);
            let manual_end_ms = manual
              .and_then(|m| m.get("end_ms"))
              .and_then(|v| v.as_i64())
              .map(|v| v as i32);

            Some(SyllablePreview {
              korean,
              romanization: rom,
              has_audio,
              baseline_start_ms,
              baseline_end_ms,
              manual_start_ms,
              manual_end_ms,
            })
          })
          .collect()
      })
      .unwrap_or_default();

    let available: Vec<String> = syllables
      .iter()
      .filter(|s| s.has_audio)
      .map(|s| s.romanization.clone())
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

    let available_count = syllables.iter().filter(|s| s.has_audio).count();

    return Some(AudioRow {
      character: char.clone(),
      romanization: romanization.to_string(),
      syllables,
      available_count,
      segments_json,
      params,
    });
  }

  None
}
