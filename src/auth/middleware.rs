//! Authentication middleware and extractors.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use super::db as auth_db;
use crate::db::run_migrations_with_app_db;
use crate::paths;
use crate::state::AppState;
use std::path::Path;

pub const SESSION_COOKIE_NAME: &str = "kr_session";

/// Authenticated request context.
/// Add this as a handler parameter to require authentication.
/// Redirects to /login if not authenticated.
#[derive(Clone)]
pub struct AuthContext {
    pub user_id: i64,
    pub username: String,
    pub is_admin: bool,
    pub user_db: Arc<Mutex<Connection>>,
    /// Whether user has access to vocabulary content (for nav dropdown)
    pub has_vocab_access: bool,
}

impl FromRequestParts<AppState> for AuthContext {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract cookies
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| Redirect::to("/login").into_response())?;

        // Get session cookie
        let session_id = jar
            .get(SESSION_COOKIE_NAME)
            .map(|c| c.value().to_string())
            .ok_or_else(|| Redirect::to("/login").into_response())?;

        // Validate session
        let auth_db = state
            .auth_db
            .lock()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response())?;

        let (user_id, username) = auth_db::get_session_user(&auth_db, &session_id)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response())?
            .ok_or_else(|| Redirect::to("/login").into_response())?;

        drop(auth_db); // Release lock before opening user db

        // Open user's database and run migrations
        let user_db_path = state.user_db_path(&username);
        let conn = Connection::open(&user_db_path).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to open user database",
            )
                .into_response()
        })?;

        // Ensure schema is up to date (adds new columns if missing)
        // Pass app.db path for legacy cards → card_progress migration
        let app_db_path_str = paths::auth_db_path();
        let app_db_path = Path::new(&app_db_path_str);
        run_migrations_with_app_db(&conn, Some(app_db_path)).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to run database migrations",
            )
                .into_response()
        })?;

        // Attach app.db for cross-database queries (card_definitions)
        conn.execute(
            &format!("ATTACH DATABASE '{}' AS app", app_db_path_str),
            [],
        )
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to attach app database",
            )
                .into_response()
        })?;

        // Check admin status and vocab access
        let (is_admin, has_vocab_access) = match state.auth_db.lock() {
            Ok(db) => {
                let admin = auth_db::is_user_admin(&db, user_id)
                    .unwrap_or_else(|_| username.eq_ignore_ascii_case("admin"));
                // Check if user has access to any vocabulary-providing packs
                let vocab = crate::services::pack_manager::any_accessible_pack_provides(
                    &db, user_id, "vocabulary"
                );
                (admin, vocab)
            }
            Err(_) => (username.eq_ignore_ascii_case("admin"), false),
        };

        Ok(AuthContext {
            user_id,
            username,
            is_admin,
            user_db: Arc::new(Mutex::new(conn)),
            has_vocab_access,
        })
    }
}

/// Optional authentication extractor.
/// Returns Some(AuthContext) if authenticated, None otherwise.
/// Use for pages that work both with and without authentication.
pub struct OptionalAuth(pub Option<AuthContext>);

impl FromRequestParts<AppState> for OptionalAuth {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match AuthContext::from_request_parts(parts, state).await {
            Ok(auth) => Ok(OptionalAuth(Some(auth))),
            Err(_) => Ok(OptionalAuth(None)),
        }
    }
}
