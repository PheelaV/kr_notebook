use axum::{routing::get, routing::post, Router};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kr_notebook::{auth, config, handlers, paths, profiling, state::AppState};

/// Path to the shared auth database
const AUTH_DB_PATH: &str = "data/app.db";

/// Path to the users data directory
const USERS_DATA_DIR: &str = "data/users";

/// Path to the old single-user database (for migration)
const OLD_DB_PATH: &str = "data/hangul.db";

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

    // Ensure data directories exist
    std::fs::create_dir_all("data").expect("Failed to create data directory");
    std::fs::create_dir_all(USERS_DATA_DIR).expect("Failed to create users directory");

    // Initialize auth database
    let auth_db_path = Path::new(AUTH_DB_PATH);

    // Backup auth database before migrations if it exists
    if auth_db_path.exists() {
        let backup_path = Path::new("data/app.db.backup");
        if let Err(e) = std::fs::copy(auth_db_path, backup_path) {
            tracing::warn!("Could not create auth database backup: {}", e);
        } else {
            tracing::debug!("Auth database backed up to {:?}", backup_path);
        }
    }

    let auth_conn = Connection::open(auth_db_path).expect("Failed to open auth database");
    auth::db::init_auth_schema(&auth_conn).expect("Failed to initialize auth schema");
    let auth_db = Arc::new(Mutex::new(auth_conn));

    // Check for migration: old single-user database exists, no users yet
    if Path::new(OLD_DB_PATH).exists() {
        let should_migrate = {
            let conn = auth_db.lock().expect("Auth DB lock failed");
            auth::db::get_user_count(&conn).unwrap_or(0) == 0
        };

        if should_migrate {
            migrate_existing_database(&auth_db);
        }
    }

    // Create app state
    let state = AppState::new(auth_db, PathBuf::from(USERS_DATA_DIR));

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/login", get(auth::login_page).post(auth::login_submit))
        .route(
            "/register",
            get(auth::register_page).post(auth::register_submit),
        )
        .route("/logout", post(auth::logout))
        .route("/guest", get(auth::guest_page).post(auth::guest_login))
        // Public reference/guide pages
        .route("/guide", get(handlers::guide))
        .route("/reference", get(handlers::reference_index))
        .route("/reference/basics", get(handlers::reference_basics))
        .route("/reference/tier1", get(handlers::reference_tier1))
        .route("/reference/tier2", get(handlers::reference_tier2))
        .route("/reference/tier3", get(handlers::reference_tier3))
        .route("/reference/tier4", get(handlers::reference_tier4))
        .route("/pronunciation", get(handlers::pronunciation_page));

    // Protected routes (auth required - AuthContext extractor handles this)
    let protected_routes = Router::new()
        .route("/", get(handlers::index))
        .route("/study", get(handlers::study_start_interactive))
        .route("/study-classic", get(handlers::study_start))
        .route("/review", post(handlers::submit_review_interactive))
        .route("/review-classic", post(handlers::submit_review))
        .route("/validate-answer", post(handlers::validate_answer_handler))
        .route("/next-card", post(handlers::next_card_interactive))
        .route("/practice", get(handlers::practice_start))
        .route("/practice-next", post(handlers::practice_next))
        .route("/practice-validate", post(handlers::practice_validate))
        .route("/progress", get(handlers::progress))
        .route("/unlock-tier", post(handlers::unlock_tier))
        .route("/library", get(handlers::library_index))
        .route("/library/characters", get(handlers::library_characters))
        .route("/library/vocabulary", get(handlers::vocabulary_library))
        .route("/listen", get(handlers::listen_index))
        .route("/listen/start", get(handlers::listen_start))
        .route("/listen/answer", post(handlers::listen_answer))
        .route("/listen/answer-htmx", post(handlers::listen_answer_htmx))
        .route("/listen/skip", get(handlers::listen_skip))
        .route(
            "/settings",
            get(handlers::settings_page).post(handlers::update_settings),
        )
        .route("/settings/scrape", post(handlers::trigger_scrape))
        .route("/settings/scrape/{lesson}", post(handlers::trigger_scrape_lesson))
        .route("/settings/delete-scraped", post(handlers::delete_scraped))
        .route(
            "/settings/delete-scraped/{lesson}",
            post(handlers::delete_scraped_lesson),
        )
        .route("/settings/segment", post(handlers::trigger_segment))
        .route("/settings/segment-row", post(handlers::trigger_row_segment))
        .route(
            "/settings/segment-manual",
            post(handlers::trigger_manual_segment),
        )
        .route(
            "/settings/segment-reset",
            post(handlers::trigger_reset_segment),
        )
        .route("/settings/make-all-due", post(handlers::make_all_due))
        .route(
            "/settings/graduate-tier/{tier}",
            post(handlers::graduate_tier),
        )
        .route(
            "/settings/restore-tier/{tier}",
            post(handlers::restore_tier),
        )
        .route("/settings/export", get(handlers::export_data))
        .route("/settings/import", post(handlers::import_data))
        .route("/settings/pack/{pack_id}/enable", post(handlers::enable_pack))
        .route("/settings/pack/{pack_id}/disable", post(handlers::disable_pack))
        .route("/settings/cleanup-guests", post(handlers::cleanup_guests))
        .route(
            "/settings/delete-all-guests",
            post(handlers::delete_all_guests),
        )
        .route("/diagnostic", post(handlers::log_diagnostic));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .nest_service("/audio/scraped", ServeDir::new(paths::SCRAPED_DIR))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let bind_addr = config::server_bind_addr();
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to {}", bind_addr));

    tracing::info!("Server running on http://localhost:{}", config::SERVER_PORT);

    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}

/// Migrate existing single-user database to multi-user setup
fn migrate_existing_database(auth_db: &Arc<Mutex<Connection>>) {
    use auth::password::hash_password;
    use sha2::{Digest, Sha256};

    tracing::info!("Migrating existing database to multi-user setup...");

    // Generate a random password for the admin user
    let password: String = (0..16)
        .map(|_| {
            let idx = rand::random::<u8>() % 62;
            match idx {
                0..=9 => (b'0' + idx) as char,
                10..=35 => (b'a' + idx - 10) as char,
                _ => (b'A' + idx - 36) as char,
            }
        })
        .collect();

    // Compute client-side hash: SHA-256(password + ':' + username)
    // This matches what the browser JavaScript does
    let client_input = format!("{}:admin", password);
    let mut hasher = Sha256::new();
    hasher.update(client_input.as_bytes());
    let client_hash = hex::encode(hasher.finalize());

    // Hash the client hash with Argon2 for storage
    let password_hash = hash_password(&client_hash).expect("Failed to hash password");

    // Create admin user
    {
        let conn = auth_db.lock().expect("Auth DB lock failed");
        auth::db::create_user(&conn, "admin", &password_hash).expect("Failed to create admin user");
    }

    // Create admin's data directory
    let admin_dir = Path::new(USERS_DATA_DIR).join("admin");
    std::fs::create_dir_all(&admin_dir).expect("Failed to create admin directory");

    // Move old database to admin's folder
    let new_db_path = admin_dir.join("learning.db");
    std::fs::rename(OLD_DB_PATH, &new_db_path).expect("Failed to move database");

    // Also move backup if it exists
    let old_backup = Path::new("data/hangul.db.backup");
    if old_backup.exists() {
        let new_backup = admin_dir.join("learning.db.backup");
        let _ = std::fs::rename(old_backup, new_backup);
    }

    tracing::info!("=======================================================");
    tracing::info!("MIGRATION COMPLETE");
    tracing::info!("=======================================================");
    tracing::info!("Your existing data has been migrated to user 'admin'");
    tracing::info!("Generated password: {}", password);
    tracing::info!("IMPORTANT: Save this password! It will not be shown again.");
    tracing::info!("You can change it by editing the database directly.");
    tracing::info!("=======================================================");
}
