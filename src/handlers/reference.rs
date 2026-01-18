use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};

use super::NavContext;
use crate::auth::{AuthContext, OptionalAuth};
use crate::content::reference::{ReferenceSection, SectionType};
use crate::content::{load_reference, ReferenceLesson};
use crate::filters;
use crate::services::pack_manager;
use crate::state::AppState;

/// Summary of a grammar pack for the index page.
pub struct GrammarPackSummary {
    pub pack_id: String,
    pub pack_name: String,
    pub description: Option<String>,
    pub lesson_count: usize,
}

/// TOC item for navigation (reuses same structure as guide, settings)
pub struct TocItem {
    pub id: String,
    pub short_label: String,
    pub full_label: String,
}

#[derive(Template)]
#[template(path = "reference/index.html")]
pub struct ReferenceIndexTemplate {
  pub nav: NavContext,
  pub grammar_packs: Vec<GrammarPackSummary>,
}

#[derive(Template)]
#[template(path = "reference/basics.html")]
pub struct ReferenceBasicsTemplate {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/tier1.html")]
pub struct ReferenceTier1Template {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/tier2.html")]
pub struct ReferenceTier2Template {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/tier3.html")]
pub struct ReferenceTier3Template {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/tier4.html")]
pub struct ReferenceTier4Template {
  pub nav: NavContext,
}

pub async fn reference_index(
    State(state): State<AppState>,
    OptionalAuth(auth): OptionalAuth,
) -> Html<String> {
    let nav = auth.as_ref().map(|a| NavContext::from_auth(a)).unwrap_or_default();

    // Get grammar packs if user is logged in
    let grammar_packs = if let Some(ref auth_ctx) = auth {
        if let Ok(app_conn) = state.auth_db.lock() {
            get_grammar_packs(&app_conn, auth_ctx.user_id)
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let template = ReferenceIndexTemplate { nav, grammar_packs };
    Html(template.render().unwrap_or_default())
}

/// Get accessible grammar packs for a user.
fn get_grammar_packs(
    app_conn: &rusqlite::Connection,
    user_id: i64,
) -> Vec<GrammarPackSummary> {
    let accessible_packs = pack_manager::get_accessible_packs(app_conn, user_id, None);

    accessible_packs
        .into_iter()
        .filter_map(|pack| {
            // Only include packs that have reference config
            let ref_config = pack.manifest.reference.as_ref()?;

            // Try to load reference content to get lesson count
            let data = load_reference(&pack.path, ref_config).ok()?;

            Some(GrammarPackSummary {
                pack_id: pack.manifest.id,
                pack_name: pack.manifest.name,
                description: pack.manifest.description,
                lesson_count: data.lessons.len(),
            })
        })
        .collect()
}

pub async fn reference_basics(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceBasicsTemplate { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier1(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceTier1Template { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier2(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceTier2Template { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier3(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceTier3Template { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier4(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceTier4Template { nav };
  Html(template.render().unwrap_or_default())
}

// ============================================================================
// Pack-based Grammar Reference Handlers
// ============================================================================

/// Summary of a lesson for the pack overview page.
pub struct LessonSummary {
    pub number: u8,
    pub title: String,
    pub description: Option<String>,
    pub section_count: usize,
}

/// Template for pack overview showing all lessons.
#[derive(Template)]
#[template(path = "reference/pack_overview.html")]
pub struct ReferencePackOverviewTemplate {
    pub nav: NavContext,
    pub pack_id: String,
    pub pack_name: String,
    pub description: Option<String>,
    pub lessons: Vec<LessonSummary>,
}

/// Template for full lesson view.
#[derive(Template)]
#[template(path = "reference/lesson.html")]
pub struct ReferenceLessonTemplate {
    pub nav: NavContext,
    pub pack_id: String,
    pub pack_name: String,
    pub lesson: ReferenceLesson,
    pub prev_lesson: Option<u8>,
    pub next_lesson: Option<u8>,
    pub toc_items: Vec<TocItem>,
    pub toc_title: String,
}

/// A quick reference item extracted from a lesson.
pub struct QuickReferenceItem {
    pub lesson_number: u8,
    pub lesson_title: String,
    pub section: ReferenceSection,
}

/// Template for compiled quick reference page (all QuickReference sections from all lessons).
#[derive(Template)]
#[template(path = "reference/quick_reference.html")]
pub struct QuickReferenceTemplate {
    pub nav: NavContext,
    pub pack_id: String,
    pub pack_name: String,
    pub items: Vec<QuickReferenceItem>,
    pub toc_items: Vec<TocItem>,
    pub toc_title: String,
}

/// Pack overview handler - list lessons in a grammar pack.
pub async fn reference_pack_overview(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(pack_id): Path<String>,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => return Html(super::DB_ERROR_HTML.to_string()).into_response(),
    };

    // Find pack
    let accessible_packs = pack_manager::get_accessible_packs(&app_conn, auth.user_id, None);
    let pack = match accessible_packs.iter().find(|p| p.manifest.id == pack_id) {
        Some(p) => p,
        None => return Redirect::to("/reference").into_response(),
    };

    // Check for reference config
    let ref_config = match pack.manifest.reference.as_ref() {
        Some(c) => c,
        None => return Redirect::to("/reference").into_response(),
    };

    // Load reference content
    let data = match load_reference(&pack.path, ref_config) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to load reference pack {}: {}", pack_id, e);
            return Html("<h1>Error loading reference content</h1>".to_string()).into_response();
        }
    };

    let lessons: Vec<LessonSummary> = data
        .lessons
        .iter()
        .map(|l| LessonSummary {
            number: l.number,
            title: l.title.clone(),
            description: l.description.clone(),
            section_count: l.sections.len(),
        })
        .collect();

    let template = ReferencePackOverviewTemplate {
        nav: NavContext::from_auth(&auth),
        pack_id: pack.manifest.id.clone(),
        pack_name: pack.manifest.name.clone(),
        description: pack.manifest.description.clone(),
        lessons,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

/// Lesson view handler - show full lesson content.
pub async fn reference_lesson(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((pack_id, lesson_num)): Path<(String, u8)>,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => return Html(super::DB_ERROR_HTML.to_string()).into_response(),
    };

    // Find pack
    let accessible_packs = pack_manager::get_accessible_packs(&app_conn, auth.user_id, None);
    let pack = match accessible_packs.iter().find(|p| p.manifest.id == pack_id) {
        Some(p) => p,
        None => return Redirect::to("/reference").into_response(),
    };

    // Check for reference config
    let ref_config = match pack.manifest.reference.as_ref() {
        Some(c) => c,
        None => return Redirect::to("/reference").into_response(),
    };

    // Load reference content
    let data = match load_reference(&pack.path, ref_config) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to load reference pack {}: {}", pack_id, e);
            return Html("<h1>Error loading reference content</h1>".to_string()).into_response();
        }
    };

    // Find the requested lesson
    let lesson = match data.lessons.iter().find(|l| l.number == lesson_num) {
        Some(l) => l.clone(),
        None => {
            return Redirect::to(&format!("/reference/pack/{}", pack_id)).into_response();
        }
    };

    // Determine prev/next lessons
    let lesson_nums: Vec<u8> = data.lessons.iter().map(|l| l.number).collect();
    let current_idx = lesson_nums.iter().position(|&n| n == lesson_num);
    let prev_lesson =
        current_idx.and_then(|i| if i > 0 { lesson_nums.get(i - 1).copied() } else { None });
    let next_lesson = current_idx.and_then(|i| lesson_nums.get(i + 1).copied());

    // Build TOC items from lesson sections
    let toc_items: Vec<TocItem> = lesson
        .sections
        .iter()
        .map(|s| {
            // Truncate title for short_label (mobile chips)
            let short_label = if s.title.chars().count() > 15 {
                format!("{}…", s.title.chars().take(14).collect::<String>())
            } else {
                s.title.clone()
            };
            TocItem {
                id: s.id.clone(),
                short_label,
                full_label: s.title.clone(),
            }
        })
        .collect();

    let template = ReferenceLessonTemplate {
        nav: NavContext::from_auth(&auth),
        pack_id: pack.manifest.id.clone(),
        pack_name: pack.manifest.name.clone(),
        lesson,
        prev_lesson,
        next_lesson,
        toc_items,
        toc_title: "Sections".to_string(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}

/// Quick reference compilation handler - shows all QuickReference sections from all lessons.
/// Only available for packs with more than one lesson.
pub async fn quick_reference(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(pack_id): Path<String>,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => return Html(super::DB_ERROR_HTML.to_string()).into_response(),
    };

    // Find pack
    let accessible_packs = pack_manager::get_accessible_packs(&app_conn, auth.user_id, None);
    let pack = match accessible_packs.iter().find(|p| p.manifest.id == pack_id) {
        Some(p) => p,
        None => return Redirect::to("/reference").into_response(),
    };

    // Check for reference config
    let ref_config = match pack.manifest.reference.as_ref() {
        Some(c) => c,
        None => return Redirect::to("/reference").into_response(),
    };

    // Load reference content
    let data = match load_reference(&pack.path, ref_config) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to load reference pack {}: {}", pack_id, e);
            return Html("<h1>Error loading reference content</h1>".to_string()).into_response();
        }
    };

    // Redirect to pack overview if only 1 lesson (quick reference not useful)
    if data.lessons.len() <= 1 {
        return Redirect::to(&format!("/reference/pack/{}", pack_id)).into_response();
    }

    // Extract all QuickReference sections from all lessons
    let mut items: Vec<QuickReferenceItem> = Vec::new();
    for lesson in &data.lessons {
        for section in &lesson.sections {
            if section.section_type == SectionType::QuickReference {
                items.push(QuickReferenceItem {
                    lesson_number: lesson.number,
                    lesson_title: lesson.title.clone(),
                    section: section.clone(),
                });
            }
        }
    }

    // Build TOC items from quick reference items
    let toc_items: Vec<TocItem> = items
        .iter()
        .map(|item| {
            let label = format!("L{}: {}", item.lesson_number, item.section.title);
            let short_label = if label.chars().count() > 15 {
                format!("{}…", label.chars().take(14).collect::<String>())
            } else {
                label.clone()
            };
            TocItem {
                id: format!("lesson-{}-{}", item.lesson_number, item.section.id),
                short_label,
                full_label: label,
            }
        })
        .collect();

    let template = QuickReferenceTemplate {
        nav: NavContext::from_auth(&auth),
        pack_id: pack.manifest.id.clone(),
        pack_name: pack.manifest.name.clone(),
        items,
        toc_items,
        toc_title: "Quick Reference".to_string(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}

// ============================================================================
// Precache URLs API
// ============================================================================

/// Returns a JSON array of URLs to precache for the service worker.
/// Includes static reference pages plus dynamic pack/lesson URLs for the user.
pub async fn precache_urls(
    State(state): State<AppState>,
    auth: AuthContext,
) -> axum::Json<Vec<String>> {
    let mut urls = vec![
        // Static reference pages
        "/reference".to_string(),
        "/reference/basics".to_string(),
        "/reference/tier1".to_string(),
        "/reference/tier2".to_string(),
        "/reference/tier3".to_string(),
        "/reference/tier4".to_string(),
        // Library pages
        "/library".to_string(),
        "/library/characters".to_string(),
        "/library/vocabulary".to_string(),
        // Guide
        "/guide".to_string(),
    ];

    // Add dynamic pack/lesson URLs
    if let Ok(app_conn) = state.auth_db.lock() {
        let accessible_packs = pack_manager::get_accessible_packs(&app_conn, auth.user_id, None);

        for pack in accessible_packs {
            // Only include packs that have reference config
            if let Some(ref ref_config) = pack.manifest.reference {
                let pack_id = &pack.manifest.id;

                // Add pack overview URL
                urls.push(format!("/reference/pack/{}", pack_id));

                // Try to load reference content to get lesson numbers
                if let Ok(data) = load_reference(&pack.path, ref_config) {
                    // Add quick reference URL if pack has more than 1 lesson
                    if data.lessons.len() > 1 {
                        urls.push(format!("/reference/pack/{}/quick-reference", pack_id));
                    }

                    for lesson in &data.lessons {
                        urls.push(format!("/reference/pack/{}/lesson/{}", pack_id, lesson.number));
                    }
                }
            }
        }
    }

    axum::Json(urls)
}
