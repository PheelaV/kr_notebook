pub mod diagnostic;
pub mod guide;
pub mod library;
pub mod listen;
pub mod progress;
pub mod pronunciation;
pub mod reference;
pub mod settings;
pub mod study;
pub mod vocabulary;

use askama::Template;
use axum::response::Html;
use chrono::{DateTime, Utc};

use axum::extract::State;

use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::filters;
use crate::state::AppState;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// Error HTML returned when database is unavailable
pub const DB_ERROR_HTML: &str = r#"<!DOCTYPE html><html><head><title>Error</title></head><body><h1>Database Error</h1><p>Please refresh the page. If the problem persists, restart the application.</p></body></html>"#;

/// Shared navigation context for all templates.
/// Contains data needed by the navbar component in base.html.
#[derive(Clone, Default)]
pub struct NavContext {
    pub has_vocab_access: bool,
    pub has_grammar_access: bool,
}

impl NavContext {
    /// Create NavContext from authenticated user context
    pub fn from_auth(auth: &AuthContext) -> Self {
        Self {
            has_vocab_access: auth.has_vocab_access,
            has_grammar_access: auth.has_grammar_access,
        }
    }

    /// Create empty NavContext for public pages (no auth)
    pub fn public() -> Self {
        Self::default()
    }
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
  pub due_count: i64,
  pub unreviewed_count: i64, // Cards not reviewed today (accelerated mode)
  pub total_cards: i64,
  pub cards_learned: i64,
  pub next_review: Option<String>,
  pub next_review_timestamp: Option<i64>, // Unix timestamp in seconds for live countdown
  pub accelerated_mode: bool,
  pub unlocked_tier: Option<u8>, // Tier that was just auto-unlocked
  pub testing_mode: bool,
  pub nav: NavContext,
}

fn format_relative_time(dt: DateTime<Utc>) -> String {
  let now = Utc::now();
  let duration = dt.signed_duration_since(now);

  let minutes = duration.num_minutes();
  let hours = duration.num_hours();
  let days = duration.num_days();

  if minutes < 1 {
    "now".to_string()
  } else if minutes < 60 {
    format!("in {} minute{}", minutes, if minutes == 1 { "" } else { "s" })
  } else if hours < 24 {
    format!("in {} hour{}", hours, if hours == 1 { "" } else { "s" })
  } else if days == 1 {
    "tomorrow".to_string()
  } else {
    format!("in {} days", days)
  }
}

pub async fn index(State(state): State<AppState>, auth: AuthContext) -> Html<String> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/".into(),
    method: "GET".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Html(DB_ERROR_HTML.to_string()),
  };

  let app_conn = match state.auth_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Html(DB_ERROR_HTML.to_string()),
  };

  // Check for auto tier unlock
  let unlocked_tier = db::try_auto_unlock_tier(&conn).log_warn("Auto tier unlock failed").flatten();

  #[cfg(feature = "profiling")]
  if let Some(tier) = unlocked_tier {
    crate::profile_log!(EventType::TierUnlock {
      tier,
      username: auth.username.clone(),
    });
  }

  let accelerated_mode = db::get_all_tiers_unlocked(&conn).log_warn_default("Failed to get all_tiers_unlocked");

  // Use filtered counts to include vocabulary pack cards (with permission check)
  let filter = db::StudyFilterMode::All;
  let due_count = db::get_due_count_filtered(&conn, &app_conn, auth.user_id, &filter).log_warn_default("Failed to get due count");
  let unreviewed_count = if accelerated_mode {
    db::get_unreviewed_today_count_filtered(&conn, &app_conn, auth.user_id, &filter).log_warn_default("Failed to get unreviewed count")
  } else {
    0
  };
  // Get accessible card counts (only cards user can actually study)
  let (total_cards, cards_learned) = db::get_accessible_card_count(&conn, &app_conn, auth.user_id)
    .log_warn_default("Failed to get accessible card count");

  // Always fetch next upcoming review time (for cards not yet due)
  // This allows the UI to show a countdown even when there are cards currently due
  let next_review_time = db::get_next_upcoming_review_time(&conn)
    .log_warn("Failed to get next review time")
    .flatten();

  let next_review = next_review_time.map(format_relative_time);
  let next_review_timestamp = next_review_time.map(|dt| dt.timestamp());

  let template = IndexTemplate {
    due_count,
    unreviewed_count,
    total_cards,
    cards_learned,
    next_review,
    next_review_timestamp,
    accelerated_mode,
    unlocked_tier,
    #[cfg(feature = "testing")]
    testing_mode: true,
    #[cfg(not(feature = "testing"))]
    testing_mode: false,
    nav: NavContext::from_auth(&auth),
  };

  Html(template.render().unwrap_or_default())
}

pub use diagnostic::log_diagnostic;
pub use guide::guide;
pub use library::{library_characters, library_index};
pub use progress::{progress, unlock_tier};
pub use reference::{
  precache_urls, reference_basics, reference_index, reference_lesson, reference_pack_overview,
  reference_tier1, reference_tier2, reference_tier3, reference_tier4,
};
pub use listen::{listen_index, listen_start, listen_answer, listen_answer_htmx, listen_skip};
pub use pronunciation::{has_scraped_content, pronunciation_page};
pub use settings::{
  cleanup_guests, delete_all_guests, delete_scraped, delete_scraped_lesson, disable_pack,
  enable_pack, export_data, graduate_tier, import_data, make_all_due, restore_tier, settings_page,
  trigger_manual_segment, trigger_reset_segment, trigger_row_segment, trigger_scrape,
  trigger_scrape_lesson, trigger_segment, update_settings,
  // User/group management
  set_user_role, create_group, delete_group, add_to_group, remove_from_group,
  // Pack permissions (groups and users)
  restrict_pack_to_group, remove_pack_restriction, make_pack_public, make_pack_private,
  restrict_pack_to_user, remove_pack_user_restriction,
  // External pack paths (admin)
  register_pack_path, unregister_pack_path, toggle_pack_path, browse_directories,
};
pub use study::{
  next_card_interactive, override_ruling_handler, practice_next, practice_start, practice_validate,
  set_study_filter, study_start, submit_review, study_start_interactive, submit_review_interactive,
  toggle_focus_mode, validate_answer_handler,
  download_session, sync_session,
};
pub use vocabulary::vocabulary_library;

// ============================================================================
// Offline / Service Worker Handlers
// ============================================================================

/// Template for the offline fallback page
#[derive(Template)]
#[template(path = "offline.html")]
pub struct OfflineTemplate {
    pub css_url: String,
}

/// Handler for the offline fallback page
pub async fn offline_page() -> Html<String> {
    let css_url = format!(
        "/static/css/styles.css?v={}",
        crate::filters::STYLES_CSS_HASH
    );
    let template = OfflineTemplate { css_url };
    Html(template.render().unwrap_or_default())
}

/// Template for the offline study page
#[derive(Template)]
#[template(path = "offline_study.html")]
pub struct OfflineStudyTemplate {
    pub css_url: String,
    pub nav: NavContext,
}

/// Handler for the offline study page
pub async fn offline_study_page(
    crate::auth::OptionalAuth(auth): crate::auth::OptionalAuth,
) -> Html<String> {
    let css_url = format!(
        "/static/css/styles.css?v={}",
        crate::filters::STYLES_CSS_HASH
    );
    let nav = auth.as_ref().map(|a| NavContext::from_auth(a)).unwrap_or_default();
    let template = OfflineStudyTemplate { css_url, nav };
    Html(template.render().unwrap_or_default())
}

/// Handler to serve the service worker from the root path
/// Service workers must be served from the root to have full scope
pub async fn service_worker() -> impl axum::response::IntoResponse {
    use axum::http::{header, StatusCode};
    use axum::response::Response;

    // Read the service worker file
    let sw_content = match std::fs::read_to_string("static/sw.js") {
        Ok(content) => content,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(axum::body::Body::from("Service worker not found"))
                .unwrap();
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        // Allow SW to control the entire site
        .header("Service-Worker-Allowed", "/")
        // Prevent caching of SW itself (browser handles SW updates)
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .body(axum::body::Body::from(sw_content))
        .unwrap()
}
