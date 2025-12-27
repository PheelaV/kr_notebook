use askama::Template;
use axum::response::{Html, Redirect};
use std::path::Path;

use super::settings::{has_lesson1, has_lesson2};
use crate::audio::{
    get_available_syllables, get_row_romanization, get_row_syllables, load_manifest,
    row_has_audio, vowel_romanization,
};
use crate::paths;

/// Check if scraped pronunciation content exists (either lesson)
pub fn has_scraped_content() -> bool {
    has_lesson1() || has_lesson2()
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

/// Build a pronunciation table from a manifest file using shared utilities
fn build_table_from_manifest(
    lesson_id: &str,
    lesson_name: &str,
    _manifest_path: &Path,
) -> Option<PronunciationTable> {
    let manifest = load_manifest(lesson_id)?;

    // Get available syllable audio files
    let available_syllables = get_available_syllables(lesson_id);
    let has_syllable_segments = !available_syllables.is_empty();

    // Build vowel columns with fallback romanization
    let vowels: Vec<VowelColumn> = manifest
        .vowels_order
        .iter()
        .map(|v| {
            let rom = manifest
                .columns
                .get(v)
                .and_then(|col| col["romanization"].as_str())
                .unwrap_or("")
                .to_string();
            VowelColumn {
                character: v.clone(),
                romanization: if rom.is_empty() {
                    vowel_romanization(v).to_string()
                } else {
                    rom
                },
            }
        })
        .collect();

    // Build consonant rows with syllables
    let consonants: Vec<ConsonantRow> = manifest
        .consonants_order
        .iter()
        .filter_map(|c| {
            let syllable_infos = get_row_syllables(&manifest, c);

            let syllables: Vec<Syllable> = syllable_infos
                .into_iter()
                .map(|s| {
                    let has_audio = available_syllables.contains(&s.romanization);
                    Syllable {
                        character: s.character,
                        romanization: s.romanization,
                        has_audio,
                    }
                })
                .collect();

            Some(ConsonantRow {
                character: c.clone(),
                romanization: get_row_romanization(&manifest, c),
                syllables,
                has_row_audio: row_has_audio(&manifest, c),
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
        let manifest_path = paths::manifest_path("lesson1");
        if let Some(table) = build_table_from_manifest(
            "lesson1",
            "Lesson 1: Basic Consonants & Vowels",
            Path::new(&manifest_path),
        ) {
            tables.push(table);
        }
    }

    // Load lesson2 if available
    if has_lesson2() {
        let manifest_path = paths::manifest_path("lesson2");
        if let Some(table) = build_table_from_manifest(
            "lesson2",
            "Lesson 2: Additional Consonants",
            Path::new(&manifest_path),
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
