use axum::{
  extract::State,
  response::{Html, IntoResponse},
  Form,
};
use chrono::Utc;
use serde::Deserialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::db::{self, try_lock, DbPool};

#[derive(Deserialize)]
pub struct DiagnosticForm {
  pub card_id: i64,
  pub displayed_front: String,
  pub displayed_answer: String,
}

pub async fn log_diagnostic(
  State(pool): State<DbPool>,
  Form(form): Form<DiagnosticForm>,
) -> impl IntoResponse {
  let conn = match try_lock(&pool) {
    Ok(conn) => conn,
    Err(_) => return Html("<p>Database error - diagnostic not logged.</p>".to_string()),
  };
  let timestamp = Utc::now();

  // Ensure diagnostics directory exists
  let diag_dir = Path::new("data/diagnostics");
  fs::create_dir_all(diag_dir).ok();

  // Build diagnostic report
  let mut report = String::new();
  report.push_str(&format!("=== Diagnostic Report ===\n"));
  report.push_str(&format!("Timestamp: {}\n", timestamp.to_rfc3339()));
  report.push_str(&format!("\n--- What User Saw ---\n"));
  report.push_str(&format!("Card ID: {}\n", form.card_id));
  report.push_str(&format!("Displayed Front: {}\n", form.displayed_front));
  report.push_str(&format!("Displayed Answer: {}\n", form.displayed_answer));

  // Get actual database state
  report.push_str(&format!("\n--- Database State ---\n"));
  match db::get_card_by_id(&conn, form.card_id) {
    Ok(Some(card)) => {
      report.push_str(&format!("DB ID: {}\n", card.id));
      report.push_str(&format!("DB Front: {}\n", card.front));
      report.push_str(&format!("DB Main Answer: {}\n", card.main_answer));
      report.push_str(&format!("DB Description: {:?}\n", card.description));
      report.push_str(&format!("DB Tier: {}\n", card.tier));
      report.push_str(&format!("DB Card Type: {:?}\n", card.card_type));
      report.push_str(&format!("DB Ease Factor: {}\n", card.ease_factor));
      report.push_str(&format!("DB Interval Days: {}\n", card.interval_days));
      report.push_str(&format!("DB Repetitions: {}\n", card.repetitions));
      report.push_str(&format!("DB Next Review: {}\n", card.next_review.to_rfc3339()));
      report.push_str(&format!("DB Total Reviews: {}\n", card.total_reviews));
      report.push_str(&format!("DB Correct Reviews: {}\n", card.correct_reviews));

      // Check for potential issues
      report.push_str(&format!("\n--- Analysis ---\n"));
      if card.front == card.main_answer {
        report.push_str("WARNING: Front and main_answer are identical!\n");
      }
      if form.displayed_front != card.front {
        report.push_str(&format!(
          "MISMATCH: Displayed front '{}' != DB front '{}'\n",
          form.displayed_front, card.front
        ));
      }
      if form.displayed_answer != card.main_answer {
        report.push_str(&format!(
          "MISMATCH: Displayed answer '{}' != DB main_answer '{}'\n",
          form.displayed_answer, card.main_answer
        ));
      }
      if form.displayed_front == card.front && form.displayed_answer == card.main_answer {
        report.push_str("OK: Displayed values match database values.\n");
      }
    }
    Ok(None) => {
      report.push_str(&format!("ERROR: Card with ID {} not found in database!\n", form.card_id));
    }
    Err(e) => {
      report.push_str(&format!("ERROR: Database query failed: {}\n", e));
    }
  }

  // Get some context - nearby cards
  report.push_str(&format!("\n--- Nearby Cards (for context) ---\n"));
  let nearby_ids = [form.card_id - 2, form.card_id - 1, form.card_id + 1, form.card_id + 2];
  for id in nearby_ids {
    if id > 0 {
      if let Ok(Some(card)) = db::get_card_by_id(&conn, id) {
        report.push_str(&format!(
          "Card {}: '{}' -> '{}'\n",
          card.id, card.front, card.main_answer
        ));
      }
    }
  }

  report.push_str(&format!("\n=== End Report ===\n\n"));

  // Write to log file
  let log_file = diag_dir.join("diagnostic.log");
  let write_result = OpenOptions::new()
    .create(true)
    .append(true)
    .open(&log_file)
    .and_then(|mut file| file.write_all(report.as_bytes()));

  // Also log to console
  tracing::warn!("Diagnostic captured:\n{}", report);

  // Return confirmation HTML
  let response = if write_result.is_ok() {
    format!(
      r#"<div class="fixed top-4 right-4 bg-green-500 text-white px-4 py-2 rounded-lg shadow-lg z-50"
           x-data="{{ show: true }}"
           x-init="setTimeout(() => $el.remove(), 3000)">
        Diagnostic logged to data/diagnostics/diagnostic.log
      </div>"#
    )
  } else {
    format!(
      r#"<div class="fixed top-4 right-4 bg-red-500 text-white px-4 py-2 rounded-lg shadow-lg z-50"
           x-data="{{ show: true }}"
           x-init="setTimeout(() => $el.remove(), 3000)">
        Failed to write diagnostic log
      </div>"#
    )
  };

  Html(response)
}
