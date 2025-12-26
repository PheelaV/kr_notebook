pub mod diagnostic;
pub mod guide;
pub mod library;
pub mod progress;
pub mod pronunciation;
pub mod reference;
pub mod settings;
pub mod study;

use askama::Template;
use axum::{extract::State, response::Html};
use chrono::{DateTime, Utc};

use crate::db::{self, DbPool};
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

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

pub async fn index(State(pool): State<DbPool>) -> Html<String> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/".into(),
    method: "GET".into(),
  });

  let conn = pool.lock().unwrap();

  let accelerated_mode = db::get_all_tiers_unlocked(&conn).unwrap_or(false);
  let due_count = db::get_due_count(&conn).unwrap_or(0);
  let unreviewed_count = if accelerated_mode {
    db::get_unreviewed_today_count(&conn).unwrap_or(0)
  } else {
    0
  };
  let (total_cards, _, cards_learned) = db::get_total_stats(&conn).unwrap_or((0, 0, 0));

  // In accelerated mode, show next review only if both due and unreviewed are 0
  // In normal mode, show next review only if due is 0
  let cards_available = if accelerated_mode {
    due_count + unreviewed_count
  } else {
    due_count
  };

  let next_review_time = if cards_available == 0 {
    db::get_next_review_time(&conn).ok().flatten()
  } else {
    None
  };

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
pub use pronunciation::{has_scraped_content, pronunciation_page};
pub use settings::{
  delete_scraped, delete_scraped_lesson, settings_page, trigger_scrape, trigger_scrape_lesson,
  trigger_segment, trigger_row_segment, update_settings,
};
pub use study::{
  practice_next, practice_start, study_start, submit_review,
  study_start_interactive, submit_review_interactive, validate_answer_handler,
};
