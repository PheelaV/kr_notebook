pub mod diagnostic;
pub mod guide;
pub mod progress;
pub mod reference;
pub mod study;

use askama::Template;
use axum::{extract::State, response::Html};
use chrono::{DateTime, Utc};

use crate::db::{self, DbPool};

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
  pub due_count: i64,
  pub total_cards: i64,
  pub cards_learned: i64,
  pub next_review: Option<String>,
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
  let conn = pool.lock().unwrap();

  let due_count = db::get_due_count(&conn).unwrap_or(0);
  let (total_cards, _, cards_learned) = db::get_total_stats(&conn).unwrap_or((0, 0, 0));

  let next_review = if due_count == 0 {
    db::get_next_review_time(&conn)
      .ok()
      .flatten()
      .map(format_relative_time)
  } else {
    None
  };

  let template = IndexTemplate {
    due_count,
    total_cards,
    cards_learned,
    next_review,
  };

  Html(template.render().unwrap_or_default())
}

pub use diagnostic::log_diagnostic;
pub use guide::guide;
pub use progress::{progress, unlock_tier};
pub use reference::{
  reference_basics, reference_index, reference_tier1, reference_tier2, reference_tier3,
  reference_tier4,
};
pub use study::{practice_next, practice_start, study_start, submit_review};
