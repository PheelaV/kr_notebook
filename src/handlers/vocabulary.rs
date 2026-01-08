//! Vocabulary library handler for passive vocabulary browsing.
//!
//! Displays vocabulary content organized by lesson with collapsible entries
//! showing rich metadata (common usages, notes, examples).

use askama::Template;
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::auth::AuthContext;
use crate::content::{any_pack_provides, find_packs_providing, is_pack_enabled};
use crate::filters;
use crate::paths;

/// Vocabulary entry with full metadata from vocabulary.json
#[derive(Debug, Clone, Deserialize)]
pub struct VocabularyEntry {
    pub term: String,
    pub romanization: String,
    pub translation: String,
    pub word_type: String,
    #[serde(default)]
    pub lesson: u8,
    #[serde(default)]
    pub page: u8,
    #[serde(default)]
    pub common_usages: Vec<Usage>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub examples: Vec<Example>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub korean: String,
    pub english: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub korean: String,
    pub english: String,
}

/// TOC item for lesson navigation
pub struct LessonTocItem {
    pub id: String,
    pub short_label: String,
    pub full_label: String,
    pub count: usize,
}

/// Vocabulary entries grouped by lesson
pub struct LessonGroup {
    pub lesson: u8,
    pub entries: Vec<VocabularyEntry>,
}

#[derive(Template)]
#[template(path = "library/vocabulary.html")]
pub struct VocabularyTemplate {
    pub pack_enabled: bool,
    pub lessons: Vec<LessonGroup>,
    pub toc_items: Vec<LessonTocItem>,
    pub total_count: usize,
}

/// Check if any vocabulary pack is available on disk
fn is_vocabulary_available() -> bool {
    any_pack_provides(Path::new(paths::SHARED_PACKS_DIR), "vocabulary")
}

/// Get pack IDs that provide vocabulary content
fn get_vocabulary_pack_ids() -> Vec<String> {
    find_packs_providing(Path::new(paths::SHARED_PACKS_DIR), "vocabulary")
}

/// Load vocabulary from an enabled pack's vocabulary.json
fn load_vocabulary(pack_id: &str) -> Option<Vec<VocabularyEntry>> {
    let vocab_path = Path::new(paths::SHARED_PACKS_DIR)
        .join(pack_id)
        .join("vocabulary.json");

    let content = fs::read_to_string(&vocab_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Vocabulary library page handler
pub async fn vocabulary_library(auth: AuthContext) -> Response {
    // First check if any vocabulary pack is available
    if !is_vocabulary_available() {
        // No vocabulary packs installed - redirect to library
        return Redirect::to("/library").into_response();
    }

    let conn = match auth.user_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
                .into_response()
        }
    };

    // Find vocabulary packs and check if any are enabled
    let vocab_pack_ids = get_vocabulary_pack_ids();
    let enabled_pack = vocab_pack_ids
        .iter()
        .find(|id| is_pack_enabled(&conn, id));

    let Some(pack_id) = enabled_pack else {
        // Packs available but none enabled
        let template = VocabularyTemplate {
            pack_enabled: false,
            lessons: vec![],
            toc_items: vec![],
            total_count: 0,
        };
        return Html(template.render().unwrap_or_default()).into_response();
    };

    // Load and group vocabulary
    let vocabulary = load_vocabulary(pack_id).unwrap_or_default();
    let total_count = vocabulary.len();

    // Group by lesson number
    let mut lesson_map: BTreeMap<u8, Vec<VocabularyEntry>> = BTreeMap::new();
    for entry in vocabulary {
        lesson_map.entry(entry.lesson).or_default().push(entry);
    }

    // Convert to Vec<LessonGroup>
    let lessons: Vec<LessonGroup> = lesson_map
        .into_iter()
        .map(|(lesson, entries)| LessonGroup { lesson, entries })
        .collect();

    // Build TOC items
    let toc_items: Vec<LessonTocItem> = lessons
        .iter()
        .map(|g| LessonTocItem {
            id: format!("lesson-{}", g.lesson),
            short_label: format!("L{}", g.lesson),
            full_label: format!("Lesson {} ({})", g.lesson, g.entries.len()),
            count: g.entries.len(),
        })
        .collect();

    let template = VocabularyTemplate {
        pack_enabled: true,
        lessons,
        toc_items,
        total_count,
    };

    Html(template.render().unwrap_or_default()).into_response()
}
