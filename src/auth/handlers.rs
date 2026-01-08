//! Authentication handlers for login, register, and logout.

use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;
use std::fs;

use super::db as auth_db;
use super::middleware::SESSION_COOKIE_NAME;
use super::password;

/// Username cookie (not HttpOnly, readable by JS for navbar display)
const USERNAME_COOKIE_NAME: &str = "kr_username";
use crate::db;
use crate::filters;
use crate::session::generate_session_id;
use crate::state::AppState;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// Session duration in hours (1 week)
const SESSION_DURATION_HOURS: i64 = 24 * 7;

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
    pub version: &'static str,
}

#[derive(Template)]
#[template(path = "auth/register.html")]
pub struct RegisterTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/guest.html")]
pub struct GuestTemplate {
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    /// Client-side SHA-256 hash of password+username (server never sees plaintext)
    pub password_hash: String,
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub username: String,
    /// Client-side SHA-256 hash of password+username (server never sees plaintext)
    pub password_hash: String,
}

#[derive(Deserialize)]
pub struct GuestForm {
    /// Optional nickname (if empty, random name generated)
    #[serde(default)]
    pub nickname: String,
}

/// GET /login - Show login page
pub async fn login_page() -> Html<String> {
    let template = LoginTemplate {
        error: None,
        version: env!("CARGO_PKG_VERSION"),
    };
    Html(template.render().unwrap_or_default())
}

/// POST /login - Process login
pub async fn login_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    // Validate input
    if form.username.is_empty() || form.password_hash.is_empty() {
        let template = LoginTemplate {
            error: Some("Username and password are required".to_string()),
            version: env!("CARGO_PKG_VERSION"),
        };
        return (jar, Html(template.render().unwrap_or_default())).into_response();
    }

    let auth_db = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            let template = LoginTemplate {
                error: Some("Database error".to_string()),
                version: env!("CARGO_PKG_VERSION"),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    // Look up user
    let (user_id, password_hash) = match auth_db::get_user_by_username(&auth_db, &form.username) {
        Ok(Some(user)) => user,
        Ok(None) => {
            let template = LoginTemplate {
                error: Some("Invalid username or password".to_string()),
                version: env!("CARGO_PKG_VERSION"),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
        Err(_) => {
            let template = LoginTemplate {
                error: Some("Database error".to_string()),
                version: env!("CARGO_PKG_VERSION"),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    // Verify password (client sent SHA-256 hash, stored is Argon2 of that hash)
    if !password::verify_password(&form.password_hash, &password_hash) {
        #[cfg(feature = "profiling")]
        crate::profile_log!(EventType::AuthLogin {
            username: form.username.clone(),
            success: false,
        });

        let template = LoginTemplate {
            error: Some("Invalid username or password".to_string()),
            version: env!("CARGO_PKG_VERSION"),
        };
        return (jar, Html(template.render().unwrap_or_default())).into_response();
    }

    // Update last login time (log but don't fail on error)
    if let Err(e) = auth_db::update_last_login(&auth_db, user_id) {
        tracing::warn!("Failed to update last login for user {}: {}", user_id, e);
    }

    // Create session
    let session_id = generate_session_id();
    if auth_db::create_session(&auth_db, user_id, &session_id, SESSION_DURATION_HOURS).is_err() {
        let template = LoginTemplate {
            error: Some("Failed to create session".to_string()),
            version: env!("CARGO_PKG_VERSION"),
        };
        return (jar, Html(template.render().unwrap_or_default())).into_response();
    }

    drop(auth_db);

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::AuthLogin {
        username: form.username.clone(),
        success: true,
    });

    // Set session cookie
    let session_cookie = Cookie::build((SESSION_COOKIE_NAME, session_id))
        .path("/")
        .http_only(true)
        .secure(false) // Set to true in production with HTTPS
        .max_age(time::Duration::hours(SESSION_DURATION_HOURS))
        .build();

    // Set username cookie (readable by JS for navbar display)
    let username_cookie = Cookie::build((USERNAME_COOKIE_NAME, form.username.clone()))
        .path("/")
        .http_only(false)
        .secure(false)
        .max_age(time::Duration::hours(SESSION_DURATION_HOURS))
        .build();

    (jar.add(session_cookie).add(username_cookie), Redirect::to("/")).into_response()
}

/// GET /register - Show registration page
pub async fn register_page() -> Html<String> {
    let template = RegisterTemplate { error: None };
    Html(template.render().unwrap_or_default())
}

/// POST /register - Process registration
pub async fn register_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> impl IntoResponse {
    // Check registration limits first
    {
        let auth_db = match state.auth_db.lock() {
            Ok(conn) => conn,
            Err(_) => {
                let template = RegisterTemplate {
                    error: Some("Database error".to_string()),
                };
                return (jar, Html(template.render().unwrap_or_default())).into_response();
            }
        };

        match auth_db::can_register_user(&auth_db) {
            Ok(false) => {
                let template = RegisterTemplate {
                    error: Some("Registration is currently disabled or at capacity".to_string()),
                };
                return (jar, Html(template.render().unwrap_or_default())).into_response();
            }
            Err(_) => {
                let template = RegisterTemplate {
                    error: Some("Database error".to_string()),
                };
                return (jar, Html(template.render().unwrap_or_default())).into_response();
            }
            Ok(true) => {}
        }
    }

    // Validate username
    if !is_valid_username(&form.username) {
        let template = RegisterTemplate {
            error: Some("Username must be 3-32 alphanumeric characters or underscores".to_string()),
        };
        return (jar, Html(template.render().unwrap_or_default())).into_response();
    }

    // Validate client hash is present (password validation done client-side)
    if form.password_hash.is_empty() || form.password_hash.len() != 64 {
        let template = RegisterTemplate {
            error: Some("Invalid password hash received".to_string()),
        };
        return (jar, Html(template.render().unwrap_or_default())).into_response();
    }

    // Hash the client's hash with Argon2 for storage
    let password_hash = match password::hash_password(&form.password_hash) {
        Ok(hash) => hash,
        Err(_) => {
            let template = RegisterTemplate {
                error: Some("Failed to process password".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    let auth_db = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            let template = RegisterTemplate {
                error: Some("Database error".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    // Check if username already exists
    match auth_db::username_exists(&auth_db, &form.username) {
        Ok(true) => {
            let template = RegisterTemplate {
                error: Some("Username already exists".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
        Err(_) => {
            let template = RegisterTemplate {
                error: Some("Database error".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
        Ok(false) => {}
    }

    // Create user
    let user_id = match auth_db::create_user(&auth_db, &form.username, &password_hash) {
        Ok(id) => id,
        Err(_) => {
            let template = RegisterTemplate {
                error: Some("Failed to create account".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    drop(auth_db); // Release lock before file operations

    // Create user's data directory
    let user_dir = state.user_dir(&form.username);
    if let Err(e) = fs::create_dir_all(&user_dir) {
        tracing::error!("Failed to create user directory: {}", e);
        let template = RegisterTemplate {
            error: Some("Failed to create user data directory".to_string()),
        };
        return (jar, Html(template.render().unwrap_or_default())).into_response();
    }

    // Initialize user's database with schema and seed data
    let user_db_path = state.user_db_path(&form.username);
    let user_db = match db::init_db(&user_db_path) {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to initialize user database: {}", e);
            // Clean up: remove user directory
            let _ = fs::remove_dir_all(&user_dir);
            let template = RegisterTemplate {
                error: Some("Failed to initialize user database".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    // Seed the database with hangul cards
    {
        let conn = match user_db.lock() {
            Ok(conn) => conn,
            Err(_) => {
                tracing::error!("User DB lock poisoned during registration");
                let _ = fs::remove_dir_all(&user_dir);
                let template = RegisterTemplate {
                    error: Some("Database error".to_string()),
                };
                return (jar, Html(template.render().unwrap_or_default())).into_response();
            }
        };
        if let Err(e) = db::seed_hangul_cards(&conn) {
            tracing::error!("Failed to seed user database: {}", e);
            drop(conn);
            // Clean up
            let _ = fs::remove_dir_all(&user_dir);
            let template = RegisterTemplate {
                error: Some("Failed to seed user database".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    }

    // Create session for auto-login
    let session_id = generate_session_id();
    let auth_db = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            tracing::error!("Auth DB lock poisoned during session creation");
            // User was created but session failed - they can still log in manually
            let template = RegisterTemplate {
                error: Some("Account created but session failed. Please log in.".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };
    if let Err(e) = auth_db::create_session(&auth_db, user_id, &session_id, SESSION_DURATION_HOURS)
    {
        tracing::error!("Failed to create session after registration: {}", e);
    }
    drop(auth_db);

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::AuthRegister {
        username: form.username.clone(),
    });

    // Set session cookie
    let session_cookie = Cookie::build((SESSION_COOKIE_NAME, session_id))
        .path("/")
        .http_only(true)
        .secure(false)
        .max_age(time::Duration::hours(SESSION_DURATION_HOURS))
        .build();

    // Set username cookie (readable by JS for navbar display)
    let username_cookie = Cookie::build((USERNAME_COOKIE_NAME, form.username.clone()))
        .path("/")
        .http_only(false)
        .secure(false)
        .max_age(time::Duration::hours(SESSION_DURATION_HOURS))
        .build();

    (jar.add(session_cookie).add(username_cookie), Redirect::to("/")).into_response()
}

/// POST /logout - Log out and clear session
pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    // Get session from cookie and delete it
    #[cfg(feature = "profiling")]
    let mut logged_out_username: Option<String> = None;

    if let Some(session_cookie) = jar.get(SESSION_COOKIE_NAME) {
        let session_id = session_cookie.value();
        if let Ok(auth_db) = state.auth_db.lock() {
            #[cfg(feature = "profiling")]
            {
                // Get username before deleting session for profiling
                if let Ok(Some((_, username))) = auth_db::get_session_user(&auth_db, session_id) {
                    logged_out_username = Some(username);
                }
            }
            if let Err(e) = auth_db::delete_session(&auth_db, session_id) {
                tracing::warn!("Failed to delete session during logout: {}", e);
            }
        }
    }

    #[cfg(feature = "profiling")]
    if let Some(username) = logged_out_username {
        crate::profile_log!(EventType::AuthLogout { username });
    }

    // Remove session cookie
    let session_cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    // Remove username cookie
    let username_cookie = Cookie::build((USERNAME_COOKIE_NAME, ""))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    (jar.remove(session_cookie).remove(username_cookie), Redirect::to("/login"))
}

/// GET /guest - Show guest login page
pub async fn guest_page() -> Html<String> {
    let template = GuestTemplate { error: None };
    Html(template.render().unwrap_or_default())
}

/// POST /guest - Create guest account and log in
pub async fn guest_login(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<GuestForm>,
) -> impl IntoResponse {
    let auth_db = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            let template = GuestTemplate {
                error: Some("Database error".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    // Clean up expired guests first
    let expiry_hours = auth_db::get_guest_expiry_hours(&auth_db).unwrap_or(24);
    if let Ok(expired_usernames) = auth_db::cleanup_expired_guests(&auth_db, expiry_hours) {
        // Clean up directories for expired guests
        for username in expired_usernames {
            let user_dir = state.user_dir(&username);
            let _ = fs::remove_dir_all(&user_dir);
            tracing::info!("Cleaned up expired guest: {}", username);
        }
    }

    // Check if guest creation is allowed
    match auth_db::can_create_guest(&auth_db) {
        Ok(false) => {
            let template = GuestTemplate {
                error: Some("Guest accounts are currently disabled or at capacity".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
        Err(_) => {
            let template = GuestTemplate {
                error: Some("Database error".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
        Ok(true) => {}
    }

    // Generate guest username
    let nickname = form.nickname.trim();
    let username = if nickname.is_empty() {
        // Generate random 8-char suffix
        let suffix: String = (0..8)
            .map(|_| {
                let idx = rand::random::<u8>() % 36;
                match idx {
                    0..=9 => (b'0' + idx) as char,
                    _ => (b'a' + idx - 10) as char,
                }
            })
            .collect();
        format!("_guest_{}", suffix)
    } else {
        // Sanitize nickname: only alphanumeric, max 20 chars
        let sanitized: String = nickname
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(20)
            .collect();
        if sanitized.is_empty() {
            let template = GuestTemplate {
                error: Some("Nickname must contain at least one letter or number".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
        format!("_guest_{}", sanitized.to_lowercase())
    };

    // Check if username already taken (unlikely but possible with custom nicknames)
    if auth_db::username_exists(&auth_db, &username).unwrap_or(false) {
        let template = GuestTemplate {
            error: Some("That nickname is already taken. Try another one or leave it empty.".to_string()),
        };
        return (jar, Html(template.render().unwrap_or_default())).into_response();
    }

    // Generate random password (user doesn't see or need it)
    let random_password: String = (0..32)
        .map(|_| {
            let idx = rand::random::<u8>() % 62;
            match idx {
                0..=9 => (b'0' + idx) as char,
                10..=35 => (b'a' + idx - 10) as char,
                _ => (b'A' + idx - 36) as char,
            }
        })
        .collect();

    // Hash password for storage
    let password_hash = match password::hash_password(&random_password) {
        Ok(hash) => hash,
        Err(_) => {
            let template = GuestTemplate {
                error: Some("Failed to create guest account".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    // Create guest user
    let user_id = match auth_db::create_guest_user(&auth_db, &username, &password_hash) {
        Ok(id) => id,
        Err(_) => {
            let template = GuestTemplate {
                error: Some("Failed to create guest account".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    drop(auth_db); // Release lock before file operations

    // Create user's data directory
    let user_dir = state.user_dir(&username);
    if let Err(e) = fs::create_dir_all(&user_dir) {
        tracing::error!("Failed to create guest directory: {}", e);
        let template = GuestTemplate {
            error: Some("Failed to create guest data".to_string()),
        };
        return (jar, Html(template.render().unwrap_or_default())).into_response();
    }

    // Initialize user's database with schema and seed data
    let user_db_path = state.user_db_path(&username);
    let user_db = match db::init_db(&user_db_path) {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to initialize guest database: {}", e);
            let _ = fs::remove_dir_all(&user_dir);
            let template = GuestTemplate {
                error: Some("Failed to initialize guest data".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };

    // Seed the database with hangul cards
    {
        let conn = match user_db.lock() {
            Ok(conn) => conn,
            Err(_) => {
                tracing::error!("User DB lock poisoned during guest creation");
                let _ = fs::remove_dir_all(&user_dir);
                let template = GuestTemplate {
                    error: Some("Database error".to_string()),
                };
                return (jar, Html(template.render().unwrap_or_default())).into_response();
            }
        };
        if let Err(e) = db::seed_hangul_cards(&conn) {
            tracing::error!("Failed to seed guest database: {}", e);
            drop(conn);
            let _ = fs::remove_dir_all(&user_dir);
            let template = GuestTemplate {
                error: Some("Failed to seed guest data".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    }

    // Create session
    let session_id = generate_session_id();
    let auth_db = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => {
            tracing::error!("Auth DB lock poisoned during guest session creation");
            let template = GuestTemplate {
                error: Some("Database error".to_string()),
            };
            return (jar, Html(template.render().unwrap_or_default())).into_response();
        }
    };
    if let Err(e) = auth_db::create_session(&auth_db, user_id, &session_id, SESSION_DURATION_HOURS) {
        tracing::error!("Failed to create session for guest: {}", e);
    }
    drop(auth_db);

    #[cfg(feature = "profiling")]
    crate::profile_log!(EventType::AuthRegister {
        username: username.clone(),
    });

    tracing::info!("Guest account created: {}", username);

    // Set session cookie
    let session_cookie = Cookie::build((SESSION_COOKIE_NAME, session_id))
        .path("/")
        .http_only(true)
        .secure(false)
        .max_age(time::Duration::hours(SESSION_DURATION_HOURS))
        .build();

    // Set username cookie (readable by JS for navbar display)
    let username_cookie = Cookie::build((USERNAME_COOKIE_NAME, username))
        .path("/")
        .http_only(false)
        .secure(false)
        .max_age(time::Duration::hours(SESSION_DURATION_HOURS))
        .build();

    (jar.add(session_cookie).add(username_cookie), Redirect::to("/")).into_response()
}

/// Validate username: 3-32 chars, alphanumeric or underscore
fn is_valid_username(username: &str) -> bool {
    username.len() >= 3
        && username.len() <= 32
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_usernames() {
        assert!(is_valid_username("abc"));
        assert!(is_valid_username("user123"));
        assert!(is_valid_username("my_user"));
        assert!(is_valid_username("User_Name_123"));
        assert!(is_valid_username("a".repeat(32).as_str()));
    }

    #[test]
    fn test_invalid_usernames() {
        assert!(!is_valid_username("ab")); // too short
        assert!(!is_valid_username(&"a".repeat(33))); // too long
        assert!(!is_valid_username("user name")); // space
        assert!(!is_valid_username("user-name")); // hyphen
        assert!(!is_valid_username("user@name")); // special char
        assert!(!is_valid_username("")); // empty
    }
}
