use askama::Template;
use axum::response::{Html, Redirect};
use std::path::Path;

use super::settings::{has_lesson1, has_lesson2, has_lesson3};
use crate::auth::AuthContext;
use crate::filters;
use crate::audio::{
    get_available_syllables, get_row_romanization, get_row_syllables, load_manifest,
    row_has_audio, vowel_romanization,
};
use crate::paths;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// Check if scraped pronunciation content exists (any lesson)
pub fn has_scraped_content() -> bool {
    has_lesson1() || has_lesson2() || has_lesson3()
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
    pub vowel_rows: Vec<VowelRow>, // For lesson3 (non-matrix layout)
    pub has_syllable_segments: bool,
    pub is_matrix: bool, // true for lesson1/2 (matrix), false for lesson3 (list)
}

/// Vowel row for Lesson 3 (vowel + consonant combinations, not a matrix)
pub struct VowelRow {
    pub character: String,
    pub romanization: String,
    pub syllables: Vec<Syllable>,
    pub has_row_audio: bool,
    pub syllable_count: usize,
    pub available_count: usize,
    pub is_complete: bool, // all expected syllables have audio
}

/// TOC item for navigation
pub struct TocItem {
    pub id: String,
    pub short_label: String,
    pub full_label: String,
}

#[derive(Template)]
#[template(path = "pronunciation.html")]
pub struct PronunciationTemplate {
    pub has_pronunciation: bool,
    pub tables: Vec<PronunciationTable>,
    pub toc_items: Vec<TocItem>,
    pub toc_title: String,
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

    // Lesson 3 has rows keyed by vowels (no consonants_order)
    // Lesson 1/2 have rows keyed by consonants (consonants_order exists)
    let is_matrix = !manifest.consonants_order.is_empty();

    if is_matrix {
        // Lesson 1/2: Matrix layout (consonant rows x vowel columns)
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
            vowel_rows: Vec::new(),
            has_syllable_segments,
            is_matrix: true,
        })
    } else {
        // Lesson 3: List layout (vowel rows with varying consonants)
        let vowel_rows: Vec<VowelRow> = manifest
            .vowels_order
            .iter()
            .filter_map(|v| {
                // For lesson3, rows are keyed by vowels
                let syllable_infos = get_row_syllables(&manifest, v);
                let syllable_count = syllable_infos.len();

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

                let available_count = syllables.iter().filter(|s| s.has_audio).count();
                let is_complete = syllable_count > 0 && available_count == syllable_count;

                Some(VowelRow {
                    character: v.clone(),
                    romanization: vowel_romanization(v).to_string(),
                    syllables,
                    has_row_audio: row_has_audio(&manifest, v),
                    syllable_count,
                    available_count,
                    is_complete,
                })
            })
            .collect();

        Some(PronunciationTable {
            lesson_name: lesson_name.to_string(),
            lesson_id: lesson_id.to_string(),
            vowels: Vec::new(),
            consonants: Vec::new(),
            vowel_rows,
            has_syllable_segments,
            is_matrix: false,
        })
    }
}

pub async fn pronunciation_page(auth: AuthContext) -> axum::response::Response {
    use axum::response::IntoResponse;

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::HandlerStart {
        route: "/pronunciation".into(),
        method: "GET".into(),
        username: Some(auth.username.clone()),
    });

    // Silence unused variable warning when profiling is disabled
    let _ = &auth;

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

    // Load lesson3 if available
    if has_lesson3() {
        let manifest_path = paths::manifest_path("lesson3");
        if let Some(table) = build_table_from_manifest(
            "lesson3",
            "Lesson 3: Diphthongs & Combined Vowels",
            Path::new(&manifest_path),
        ) {
            tables.push(table);
        }
    }

    if tables.is_empty() {
        return Redirect::to("/").into_response();
    }

    // Build TOC items from tables
    let toc_items: Vec<TocItem> = tables
        .iter()
        .map(|t| {
            let short_label = match t.lesson_id.as_str() {
                "lesson1" => "Lesson 1",
                "lesson2" => "Lesson 2",
                "lesson3" => "Lesson 3",
                _ => &t.lesson_id,
            }
            .to_string();
            TocItem {
                id: t.lesson_id.clone(),
                short_label,
                full_label: t.lesson_name.clone(),
            }
        })
        .collect();

    let template = PronunciationTemplate {
        has_pronunciation: true,
        tables,
        toc_items,
        toc_title: "Lessons".to_string(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}
