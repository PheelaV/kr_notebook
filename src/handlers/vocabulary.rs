//! Vocabulary library handler for passive vocabulary browsing.
//!
//! Displays vocabulary content organized by lesson with collapsible entries
//! showing rich metadata (common usages, notes, examples).

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

use crate::auth::AuthContext;
use crate::filters;
use crate::handlers::NavContext;
use crate::services::pack_manager::{self, PackFilter};
use crate::state::AppState;

/// Vocabulary entry with full metadata from vocabulary.json
#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Usage {
    pub korean: String,
    pub english: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Example {
    pub korean: String,
    pub english: String,
}

/// Vocabulary entries grouped by lesson
pub struct LessonGroup {
    pub lesson: u8,
    pub entries: Vec<VocabularyEntry>,
}

/// Flattened, searchable entry for Fuse.js (includes all text fields)
#[derive(Debug, Clone, Serialize)]
pub struct SearchableEntry {
    pub id: String,
    pub term: String,
    pub romanization: String,
    pub translation: String,
    pub notes: String,
    pub word_type: String,
    pub lesson: u8,
    pub pack_id: String,
    pub usages_text: String,
    pub examples_text: String,
}

/// A pack with its vocabulary content grouped by lesson
pub struct PackGroup {
    pub pack_id: String,
    pub pack_name: String,
    pub pack_description: Option<String>,
    pub lessons: Vec<LessonGroup>,
    pub word_count: usize,
}

/// TOC item for pack navigation
pub struct PackTocItem {
    pub id: String,
    pub name: String,
    pub word_count: usize,
    pub lessons: Vec<LessonTocItem>,
}

/// TOC item for lesson navigation within a pack
pub struct LessonTocItem {
    pub id: String,
    pub short_label: String,
    pub full_label: String,
    pub count: usize,
}

#[derive(Template)]
#[template(path = "library/vocabulary.html")]
pub struct VocabularyTemplate {
    pub pack_enabled: bool,
    pub packs: Vec<PackGroup>,
    pub toc_items: Vec<PackTocItem>,
    pub total_count: usize,
    pub nav: NavContext,
    /// JSON array of searchable entries for client-side Fuse.js search
    pub vocabulary_json: String,
    /// Use local Fuse.js instead of CDN (set via USE_LOCAL_FUSE env var for tests)
    pub use_local_fuse: bool,
}

/// Load vocabulary from a pack's vocabulary source.
///
/// Supports two formats:
/// 1. Single `vocabulary.json` file at pack root (legacy format)
/// 2. `vocabulary/` directory with `lesson_*.json` files (directory format)
fn load_vocabulary_from_path(pack_path: &Path) -> Option<Vec<VocabularyEntry>> {
    // Try single vocabulary.json first (backward compatible)
    let vocab_file = pack_path.join("vocabulary.json");
    if vocab_file.exists() {
        let content = fs::read_to_string(&vocab_file).ok()?;
        return serde_json::from_str(&content).ok();
    }

    // Try vocabulary directory with lesson_*.json files
    let vocab_dir = pack_path.join("vocabulary");
    if vocab_dir.is_dir() {
        let mut all_entries = Vec::new();

        if let Ok(dir_entries) = fs::read_dir(&vocab_dir) {
            for entry in dir_entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                        if name.starts_with("lesson_") {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(entries) =
                                    serde_json::from_str::<Vec<VocabularyEntry>>(&content)
                                {
                                    all_entries.extend(entries);
                                }
                            }
                        }
                    }
                }
            }
        }

        if !all_entries.is_empty() {
            return Some(all_entries);
        }
    }

    None
}

/// Build searchable entries from pack groups for Fuse.js client-side search
fn build_searchable_entries(packs: &[PackGroup]) -> Vec<SearchableEntry> {
    let mut entries = Vec::new();

    for pack in packs {
        for lesson_group in &pack.lessons {
            for (entry_idx, entry) in lesson_group.entries.iter().enumerate() {
                // Flatten common_usages to searchable text
                let usages_text: String = entry
                    .common_usages
                    .iter()
                    .map(|u| format!("{} {}", u.korean, u.english))
                    .collect::<Vec<_>>()
                    .join(" ");

                // Flatten examples to searchable text
                let examples_text: String = entry
                    .examples
                    .iter()
                    .map(|e| format!("{} {}", e.korean, e.english))
                    .collect::<Vec<_>>()
                    .join(" ");

                // ID format matches template: pack_id-lesson-entry_index (0-based within lesson)
                entries.push(SearchableEntry {
                    id: format!("{}-{}-{}", pack.pack_id, lesson_group.lesson, entry_idx),
                    term: entry.term.clone(),
                    romanization: entry.romanization.clone(),
                    translation: entry.translation.clone(),
                    notes: entry.notes.clone().unwrap_or_default(),
                    word_type: entry.word_type.clone(),
                    lesson: lesson_group.lesson,
                    pack_id: pack.pack_id.clone(),
                    usages_text,
                    examples_text,
                });
            }
        }
    }

    entries
}

/// Vocabulary library page handler
pub async fn vocabulary_library(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string())
                .into_response()
        }
    };

    // Check if any vocabulary packs exist at all (for redirect vs "not enabled")
    if !pack_manager::any_accessible_pack_provides(&app_conn, auth.user_id, "vocabulary") {
        // Check if vocabulary packs exist but user can't access them
        let all_packs = pack_manager::discover_all_packs(&app_conn);
        let vocab_exists = all_packs
            .iter()
            .any(|p| p.manifest.provides.iter().any(|t| t == "vocabulary"));

        if !vocab_exists {
            // No vocabulary packs installed at all - redirect to library
            return Redirect::to("/library").into_response();
        }

        // Vocabulary packs exist but user can't access any
        let template = VocabularyTemplate {
            pack_enabled: false,
            packs: vec![],
            toc_items: vec![],
            total_count: 0,
            nav: NavContext::from_auth(&auth),
            vocabulary_json: "[]".to_string(),
            use_local_fuse: env::var("USE_LOCAL_FUSE").is_ok(),
        };
        return Html(template.render().unwrap_or_default()).into_response();
    }

    // Get accessible vocabulary packs using PackManager
    let accessible_packs = pack_manager::get_accessible_packs(
        &app_conn,
        auth.user_id,
        Some(PackFilter::provides("vocabulary")),
    );

    // Build pack groups with vocabulary content
    let mut pack_groups: Vec<PackGroup> = Vec::new();
    let mut toc_items: Vec<PackTocItem> = Vec::new();
    let mut total_count = 0;

    for pack in &accessible_packs {
        if let Some(vocab_entries) = load_vocabulary_from_path(&pack.path) {
            if vocab_entries.is_empty() {
                continue;
            }

            let word_count = vocab_entries.len();
            total_count += word_count;

            // Group entries by lesson within this pack
            let mut lesson_map: BTreeMap<u8, Vec<VocabularyEntry>> = BTreeMap::new();
            for entry in vocab_entries {
                lesson_map.entry(entry.lesson).or_default().push(entry);
            }

            // Convert to Vec<LessonGroup>
            let lessons: Vec<LessonGroup> = lesson_map
                .into_iter()
                .map(|(lesson, entries)| LessonGroup { lesson, entries })
                .collect();

            // Build lesson TOC items for this pack
            let lesson_toc_items: Vec<LessonTocItem> = lessons
                .iter()
                .map(|g| LessonTocItem {
                    id: format!("{}-lesson-{}", pack.manifest.id, g.lesson),
                    short_label: format!("L{}", g.lesson),
                    full_label: format!("Lesson {} ({})", g.lesson, g.entries.len()),
                    count: g.entries.len(),
                })
                .collect();

            toc_items.push(PackTocItem {
                id: pack.manifest.id.clone(),
                name: pack.manifest.name.clone(),
                word_count,
                lessons: lesson_toc_items,
            });

            pack_groups.push(PackGroup {
                pack_id: pack.manifest.id.clone(),
                pack_name: pack.manifest.name.clone(),
                pack_description: pack.manifest.description.clone(),
                lessons,
                word_count,
            });
        }
    }

    // Build searchable entries for client-side search
    let searchable_entries = build_searchable_entries(&pack_groups);
    let vocabulary_json = serde_json::to_string(&searchable_entries).unwrap_or_else(|_| "[]".to_string());

    let template = VocabularyTemplate {
        pack_enabled: !pack_groups.is_empty(),
        packs: pack_groups,
        toc_items,
        total_count,
        nav: NavContext::from_auth(&auth),
        vocabulary_json,
        use_local_fuse: env::var("USE_LOCAL_FUSE").is_ok(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}
