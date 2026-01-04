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
use crate::paths::AUTH_DB_PATH;
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
        let app_db_path = Path::new(AUTH_DB_PATH);
        run_migrations_with_app_db(&conn, Some(app_db_path)).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to run database migrations",
            )
                .into_response()
        })?;

        // Attach app.db for cross-database queries (card_definitions)
        conn.execute(
            &format!("ATTACH DATABASE '{}' AS app", AUTH_DB_PATH),
            [],
        )
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to attach app database",
            )
                .into_response()
        })?;

        let is_admin = username.eq_ignore_ascii_case("admin");

        Ok(AuthContext {
            user_id,
            username,
            is_admin,
            user_db: Arc::new(Mutex::new(conn)),
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
