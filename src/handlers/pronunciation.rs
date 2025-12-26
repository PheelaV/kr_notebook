use askama::Template;
use axum::response::{Html, Redirect};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::settings::{has_lesson1, has_lesson2};

/// Check if scraped pronunciation content exists (either lesson)
pub fn has_scraped_content() -> bool {
    has_lesson1() || has_lesson2()
}

/// Get set of available syllable romanizations for a lesson
fn get_available_syllables(lesson: &str) -> HashSet<String> {
    let syllables_dir = format!("data/scraped/htsk/{}/syllables", lesson);
    let syllables_path = Path::new(&syllables_dir);

    if syllables_path.exists() {
        fs::read_dir(syllables_path)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let path = e.path();
                        if path.extension().map(|ext| ext == "mp3").unwrap_or(false) {
                            path.file_stem()
                                .and_then(|s| s.to_str())
                                .map(String::from)
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        HashSet::new()
    }
}

#[derive(Clone)]
pub struct VowelColumn {
    pub character: String,
    pub romanization: String,
}

#[derive(Clone)]
pub struct Syllable {
    pub character: String,
    pub romanization: String,
    pub has_audio: bool,
}

pub struct ConsonantRow {
    pub character: String,
    pub romanization: String,
    pub syllables: Vec<Syllable>,
    pub has_row_audio: bool,
}

/// Represents a pronunciation table (one per lesson)
pub struct PronunciationTable {
    pub lesson_name: String,
    pub lesson_id: String,
    pub vowels: Vec<VowelColumn>,
    pub consonants: Vec<ConsonantRow>,
    pub has_syllable_segments: bool,
}

#[derive(Template)]
#[template(path = "pronunciation.html")]
pub struct PronunciationTemplate {
    pub has_pronunciation: bool,
    pub tables: Vec<PronunciationTable>,
}

/// Build a pronunciation table from a manifest file
fn build_table_from_manifest(
    lesson_id: &str,
    lesson_name: &str,
    manifest_path: &Path,
) -> Option<PronunciationTable> {
    let manifest_content = fs::read_to_string(manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_content).ok()?;

    // Get available syllable audio files
    let available_syllables = get_available_syllables(lesson_id);
    let has_syllable_segments = !available_syllables.is_empty();

    // Extract vowels order
    let vowels_order: Vec<String> = manifest["vowels_order"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Extract consonants order
    let consonants_order: Vec<String> = manifest["consonants_order"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Build vowel columns (for lesson1 which has column audio)
    let columns = &manifest["columns"];
    let vowels: Vec<VowelColumn> = vowels_order
        .iter()
        .map(|v| {
            let rom = columns
                .get(v)
                .and_then(|col| col["romanization"].as_str())
                .unwrap_or("")
                .to_string();
            VowelColumn {
                character: v.clone(),
                romanization: if rom.is_empty() {
                    // Fallback romanization for vowels without column audio
                    match v.as_str() {
                        "ㅣ" => "i",
                        "ㅏ" => "a",
                        "ㅓ" => "eo",
                        "ㅡ" => "eu",
                        "ㅜ" => "u",
                        "ㅗ" => "o",
                        _ => "",
                    }
                    .to_string()
                } else {
                    rom
                },
            }
        })
        .collect();

    // Build consonant rows with syllables
    let rows = &manifest["rows"];
    let syllable_table = &manifest["syllable_table"];

    let consonants: Vec<ConsonantRow> = consonants_order
        .iter()
        .filter_map(|c| {
            let row = rows.get(c)?;
            let row_audio_file = row["file"].as_str().unwrap_or("");
            let has_row_audio = !row_audio_file.is_empty();

            // Get syllables for this row
            let syllables: Vec<Syllable> = row["syllables"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|s| {
                            let syllable_char = s.as_str()?;
                            let rom = syllable_table
                                .get(syllable_char)
                                .and_then(|st| st["romanization"].as_str())
                                .unwrap_or("")
                                .to_string();

                            // Check if this syllable has audio
                            let has_audio = available_syllables.contains(&rom);

                            Some(Syllable {
                                character: syllable_char.to_string(),
                                romanization: rom,
                                has_audio,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            Some(ConsonantRow {
                character: c.clone(),
                romanization: row["romanization"].as_str().unwrap_or("").to_string(),
                syllables,
                has_row_audio,
            })
        })
        .collect();

    Some(PronunciationTable {
        lesson_name: lesson_name.to_string(),
        lesson_id: lesson_id.to_string(),
        vowels,
        consonants,
        has_syllable_segments,
    })
}

pub async fn pronunciation_page() -> axum::response::Response {
    use axum::response::IntoResponse;

    if !has_scraped_content() {
        return Redirect::to("/").into_response();
    }

    let mut tables = Vec::new();

    // Load lesson1 if available
    if has_lesson1() {
        if let Some(table) = build_table_from_manifest(
            "lesson1",
            "Lesson 1: Basic Consonants & Vowels",
            Path::new("data/scraped/htsk/lesson1/manifest.json"),
        ) {
            tables.push(table);
        }
    }

    // Load lesson2 if available
    if has_lesson2() {
        if let Some(table) = build_table_from_manifest(
            "lesson2",
            "Lesson 2: Additional Consonants",
            Path::new("data/scraped/htsk/lesson2/manifest.json"),
        ) {
            tables.push(table);
        }
    }

    if tables.is_empty() {
        return Redirect::to("/").into_response();
    }

    let template = PronunciationTemplate {
        has_pronunciation: true,
        tables,
    };

    Html(template.render().unwrap_or_default()).into_response()
}
