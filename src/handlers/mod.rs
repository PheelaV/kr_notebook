pub mod diagnostic;
pub mod guide;
pub mod library;
pub mod listen;
pub mod progress;
pub mod pronunciation;
pub mod reference;
pub mod settings;
pub mod study;

use askama::Template;
use axum::response::Html;
use chrono::{DateTime, Utc};

use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::filters;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// Error HTML returned when database is unavailable
pub const DB_ERROR_HTML: &str = r#"<!DOCTYPE html><html><head><title>Error</title></head><body><h1>Database Error</h1><p>Please refresh the page. If the problem persists, restart the application.</p></body></html>"#;

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

pub async fn index(auth: AuthContext) -> Html<String> {
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
  let due_count = db::get_due_count(&conn).log_warn_default("Failed to get due count");
  let unreviewed_count = if accelerated_mode {
    db::get_unreviewed_today_count(&conn).log_warn_default("Failed to get unreviewed count")
  } else {
    0
  };
  let (total_cards, _, cards_learned) = db::get_total_stats(&conn).log_warn_default("Failed to get total stats");

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
  };

  Html(template.render().unwrap_or_default())
}

pub use diagnostic::log_diagnostic;
pub use guide::guide;
pub use library::library;
pub use progress::{progress, unlock_tier};
pub use reference::{
  reference_basics, reference_index, reference_tier1, reference_tier2, reference_tier3,
  reference_tier4,
};
pub use listen::{listen_index, listen_start, listen_answer, listen_answer_htmx, listen_skip};
pub use pronunciation::{has_scraped_content, pronunciation_page};
pub use settings::{
  cleanup_guests, delete_all_guests, delete_scraped, delete_scraped_lesson, export_data,
  graduate_tier, import_data, make_all_due, restore_tier, settings_page, trigger_scrape,
  trigger_scrape_lesson, trigger_segment, trigger_row_segment, trigger_manual_segment,
  trigger_reset_segment, update_settings,
};
pub use study::{
  next_card_interactive, practice_next, practice_start, practice_validate, study_start,
  submit_review, study_start_interactive, submit_review_interactive, validate_answer_handler,
};
