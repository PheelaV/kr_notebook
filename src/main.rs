use axum::{routing::get, routing::post, Router};
use std::path::Path;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kr_notebook::{config, db, handlers, paths, profiling};

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

  let db_path = Path::new(paths::DB_PATH);
  let pool = db::init_db(db_path).expect("Failed to initialize database");

  {
    let conn = pool.lock().expect("Database lock failed during startup");
    db::seed_hangul_cards(&conn).expect("Failed to seed cards");

    // Refresh character stats decay windows (7D/1D) on startup
    if let Err(e) = db::refresh_character_stats_decay(&conn) {
      tracing::warn!("Failed to refresh character stats decay: {}", e);
    }
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
    .route("/practice-validate", post(handlers::practice_validate))
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
    .route("/pronunciation", get(handlers::pronunciation_page))
    .route("/listen", get(handlers::listen_index))
    .route("/listen/start", get(handlers::listen_start))
    .route("/listen/answer", post(handlers::listen_answer))
    .route("/listen/answer-htmx", post(handlers::listen_answer_htmx))
    .route("/listen/skip", get(handlers::listen_skip))
    .route("/settings", get(handlers::settings_page).post(handlers::update_settings))
    .route("/settings/scrape", post(handlers::trigger_scrape))
    .route("/settings/scrape/{lesson}", post(handlers::trigger_scrape_lesson))
    .route("/settings/delete-scraped", post(handlers::delete_scraped))
    .route("/settings/delete-scraped/{lesson}", post(handlers::delete_scraped_lesson))
    .route("/settings/segment", post(handlers::trigger_segment))
    .route("/settings/segment-row", post(handlers::trigger_row_segment))
    .route("/settings/make-all-due", post(handlers::make_all_due))
    .route("/diagnostic", post(handlers::log_diagnostic))
    .nest_service("/audio/scraped", ServeDir::new(paths::SCRAPED_DIR))
    .nest_service("/static", ServeDir::new("static"))
    .with_state(pool);

  let bind_addr = config::server_bind_addr();
  let listener = tokio::net::TcpListener::bind(&bind_addr)
    .await
    .unwrap_or_else(|_| panic!("Failed to bind to {}", bind_addr));

  tracing::info!("Server running on http://localhost:{}", config::SERVER_PORT);

  axum::serve(listener, app)
    .await
    .expect("Server failed to start");
}
