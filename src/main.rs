use axum::{routing::get, routing::post, Router};
use std::path::Path;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kr_notebook::{db, handlers, profiling};

#[tokio::main]
async fn main() {
  tracing_subscriber::registry()
    .with(
      tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "kr_notebook=debug,tower_http=debug".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

  // Initialize profiling (no-op if feature disabled)
  profiling::init();

  let db_path = Path::new("data/hangul.db");
  let pool = db::init_db(db_path).expect("Failed to initialize database");

  {
    let conn = pool.lock().unwrap();
    db::seed_hangul_cards(&conn).expect("Failed to seed cards");
  }

  let app = Router::new()
    .route("/", get(handlers::index))
    .route("/study", get(handlers::study_start_interactive)) // Use interactive mode by default
    .route("/study-classic", get(handlers::study_start))     // Classic reveal-and-rate mode
    .route("/review", post(handlers::submit_review_interactive))
    .route("/review-classic", post(handlers::submit_review))
    .route("/validate-answer", post(handlers::validate_answer_handler))
    .route("/practice", get(handlers::practice_start))
    .route("/practice-next", post(handlers::practice_next))
    .route("/progress", get(handlers::progress))
    .route("/unlock-tier", post(handlers::unlock_tier))
    .route("/guide", get(handlers::guide))
    .route("/reference", get(handlers::reference_index))
    .route("/reference/basics", get(handlers::reference_basics))
    .route("/reference/tier1", get(handlers::reference_tier1))
    .route("/reference/tier2", get(handlers::reference_tier2))
    .route("/reference/tier3", get(handlers::reference_tier3))
    .route("/reference/tier4", get(handlers::reference_tier4))
    .route("/library", get(handlers::library))
    .route("/settings", get(handlers::settings_page).post(handlers::update_settings))
    .route("/diagnostic", post(handlers::log_diagnostic))
    .with_state(pool);

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
    .await
    .expect("Failed to bind to port 3000");

  tracing::info!("Server running on http://localhost:3000");

  axum::serve(listener, app)
    .await
    .expect("Server failed to start");
}
