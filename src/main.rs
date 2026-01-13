use axum::{routing::delete, routing::get, routing::post, Router};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kr_notebook::{auth, config, handlers, paths, profiling, state::AppState};

/// Check if --init-db flag is present (initialize database and exit)
fn is_init_db_only() -> bool {
    std::env::args().any(|arg| arg == "--init-db" || arg == "--init-db-only")
}

/// Check for --init-user-db <username> flag (initialize user learning.db and exit)
fn get_init_user_db() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg == "--init-user-db" {
            return args.get(i + 1).cloned();
        }
    }
    None
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kr_notebook=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Check for --init-db flag
    let init_db_only = is_init_db_only();
    if init_db_only {
        tracing::info!("Running in --init-db mode (database initialization only)");
    }

    // Initialize profiling (no-op if feature disabled)
    profiling::init();

    // Log data directory (useful for debugging E2E test isolation)
    tracing::info!("Using data directory: {}", paths::data_dir());

    // Ensure data directories exist
    std::fs::create_dir_all(paths::data_dir()).expect("Failed to create data directory");
    std::fs::create_dir_all(paths::users_dir()).expect("Failed to create users directory");

    // Initialize auth database
    let auth_db_path_str = paths::auth_db_path();
    let auth_db_path = Path::new(&auth_db_path_str);

    // Backup auth database before migrations if it exists (skip for init-db mode)
    if auth_db_path.exists() && !init_db_only {
        let backup_path_str = format!("{}/app.db.backup", paths::data_dir());
        let backup_path = Path::new(&backup_path_str);
        if let Err(e) = std::fs::copy(auth_db_path, backup_path) {
            tracing::warn!("Could not create auth database backup: {}", e);
        } else {
            tracing::debug!("Auth database backed up to {:?}", backup_path);
        }
    }

    let auth_conn = Connection::open(auth_db_path).expect("Failed to open auth database");
    auth::db::init_auth_schema(&auth_conn).expect("Failed to initialize auth schema");

    // If --init-db flag, exit after initializing database
    if init_db_only {
        tracing::info!("Database initialized successfully at: {}", auth_db_path_str);
        tracing::info!("Schema version: {}", auth::db::get_schema_version(&auth_conn).unwrap_or(0));
        return;
    }

    // Check for --init-user-db <username> flag
    if let Some(username) = get_init_user_db() {
        let user_db_path = paths::user_db_path(&username);
        tracing::info!("Initializing user learning.db for: {}", username);

        // Create user directory
        let user_dir = Path::new(&user_db_path).parent().unwrap();
        std::fs::create_dir_all(user_dir).expect("Failed to create user directory");

        // Initialize learning.db with schema
        let _pool = kr_notebook::db::init_db_with_app_db(
            Path::new(&user_db_path),
            Some(auth_db_path),
        )
        .expect("Failed to initialize user learning.db");

        tracing::info!("User learning.db initialized at: {}", user_db_path);
        return;
    }

    let auth_db = Arc::new(Mutex::new(auth_conn));

    // Check for migration: old single-user database exists, no users yet
    let old_db_path_str = paths::db_path();
    if Path::new(&old_db_path_str).exists() {
        let should_migrate = {
            let conn = match auth_db.lock() {
                Ok(conn) => conn,
                Err(_) => {
                    tracing::error!("Auth DB lock poisoned during migration check");
                    panic!("Fatal: Auth database lock poisoned at startup");
                }
            };
            auth::db::get_user_count(&conn).unwrap_or(0) == 0
        };

        if should_migrate {
            migrate_existing_database(&auth_db);
        }
    }

    // Create app state
    let state = AppState::new(auth_db, PathBuf::from(paths::users_dir()));

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
        .route("/pronunciation", get(handlers::pronunciation_page))
        // Offline / Service Worker routes
        .route("/offline", get(handlers::offline_page))
        .route("/offline-study", get(handlers::offline_study_page))
        .route("/sw.js", get(handlers::service_worker));

    // Protected routes (auth required - AuthContext extractor handles this)
    let protected_routes = Router::new()
        .route("/", get(handlers::index))
        .route("/study", get(handlers::study_start_interactive))
        .route("/study-classic", get(handlers::study_start))
        .route("/review", post(handlers::submit_review_interactive))
        .route("/review-classic", post(handlers::submit_review))
        .route("/validate-answer", post(handlers::validate_answer_handler))
        .route("/next-card", post(handlers::next_card_interactive))
        .route("/study/filter", post(handlers::set_study_filter))
        .route("/study/focus", post(handlers::toggle_focus_mode))
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
        // Reference pack routes (dynamic grammar content)
        .route("/reference/pack/{pack_id}", get(handlers::reference_pack_overview))
        .route("/reference/pack/{pack_id}/lesson/{lesson}", get(handlers::reference_lesson))
        // API endpoint for service worker to get dynamic precache URLs
        .route("/api/precache-urls", get(handlers::precache_urls))
        // Offline study mode API
        .route("/api/study/download-session", post(handlers::study::download_session))
        .route("/api/study/sync-offline", post(handlers::study::sync_session))
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
        // User/group management (admin)
        .route("/settings/user/role", post(handlers::set_user_role))
        .route("/settings/group/create", post(handlers::create_group))
        .route("/settings/group/{group_id}", delete(handlers::delete_group))
        .route("/settings/group/add-member", post(handlers::add_to_group))
        .route("/settings/group/remove-member", post(handlers::remove_from_group))
        // Pack permissions (admin) - groups
        .route("/settings/pack/permission/add", post(handlers::restrict_pack_to_group))
        .route("/settings/pack/permission/remove", post(handlers::remove_pack_restriction))
        .route("/settings/pack/{pack_id}/make-public", post(handlers::make_pack_public))
        // Pack permissions (admin) - users
        .route("/settings/pack/user-permission/add", post(handlers::restrict_pack_to_user))
        .route("/settings/pack/user-permission/remove", post(handlers::remove_pack_user_restriction))
        // External pack paths (admin)
        .route("/settings/pack-paths/register", post(handlers::register_pack_path))
        .route("/settings/pack-paths/{id}", delete(handlers::unregister_pack_path))
        .route("/settings/pack-paths/{id}/toggle", post(handlers::toggle_pack_path))
        .route("/settings/pack-paths/browse", post(handlers::browse_directories))
        .route("/diagnostic", post(handlers::log_diagnostic));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .nest_service("/audio/scraped", ServeDir::new(paths::scraped_dir()))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let bind_addr = config::server_bind_addr();
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to {}", bind_addr));

    tracing::info!("Server running on http://localhost:{}", config::server_port());

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
        let conn = match auth_db.lock() {
            Ok(conn) => conn,
            Err(_) => {
                tracing::error!("Auth DB lock poisoned during admin creation");
                panic!("Fatal: Auth database lock poisoned during migration");
            }
        };
        auth::db::create_user(&conn, "admin", &password_hash).expect("Failed to create admin user");
    }

    // Create admin's data directory
    let users_dir = paths::users_dir();
    let admin_dir = Path::new(&users_dir).join("admin");
    std::fs::create_dir_all(&admin_dir).expect("Failed to create admin directory");

    // Move old database to admin's folder
    let new_db_path = admin_dir.join("learning.db");
    let old_db_path = paths::db_path();
    std::fs::rename(&old_db_path, &new_db_path).expect("Failed to move database");

    // Also move backup if it exists
    let old_backup_path = format!("{}.backup", old_db_path);
    let old_backup = Path::new(&old_backup_path);
    if old_backup.exists() {
        let new_backup = admin_dir.join("learning.db.backup");
        let _ = std::fs::rename(old_backup, new_backup);
    }

    // Don't log password - security risk if logs are captured
    let _ = password; // Password is set but not displayed
    tracing::info!("=======================================================");
    tracing::info!("MIGRATION COMPLETE");
    tracing::info!("=======================================================");
    tracing::info!("Your existing data has been migrated to user 'admin'");
    tracing::info!("Admin user created with a random password.");
    tracing::info!("To set the admin password, run:");
    tracing::info!("  uv run scripts/reset_pwd.py admin <new_password>");
    tracing::info!("=======================================================");
}
