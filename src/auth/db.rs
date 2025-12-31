//! Auth database operations (users, sessions, app_settings tables).

use chrono::{Duration, Utc};
use rusqlite::{params, Connection, Result};

/// Initialize the auth database schema
pub fn init_auth_schema(conn: &Connection) -> Result<()> {
    // Create base tables first
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE COLLATE NOCASE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_login_at TEXT,
            is_guest INTEGER DEFAULT 0,
            last_activity_at TEXT
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            last_access_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
    "#,
    )?;

    // Migrations for existing databases (must run before index on is_guest)
    add_column_if_missing(conn, "users", "is_guest", "INTEGER DEFAULT 0")?;
    add_column_if_missing(conn, "users", "last_activity_at", "TEXT")?;

    // Create index on is_guest after migration ensures column exists
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_users_is_guest ON users(is_guest);

        -- Default app settings
        INSERT OR IGNORE INTO app_settings (key, value) VALUES ('max_users', NULL);
        INSERT OR IGNORE INTO app_settings (key, value) VALUES ('max_guests', NULL);
        INSERT OR IGNORE INTO app_settings (key, value) VALUES ('guest_expiry_hours', '24');
    "#,
    )?;

    Ok(())
}

/// Check if a column exists in a table
fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn
        .prepare(&format!("SELECT {} FROM {} LIMIT 1", column, table))
        .is_ok()
}

/// Add a column if it doesn't already exist
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    column_def: &str,
) -> Result<()> {
    if !column_exists(conn, table, column) {
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, column_def),
            [],
        )?;
    }
    Ok(())
}

/// Create a new user, returns the user ID
pub fn create_user(conn: &Connection, username: &str, password_hash: &str) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
        params![username, password_hash, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get user by username, returns (user_id, password_hash)
pub fn get_user_by_username(conn: &Connection, username: &str) -> Result<Option<(i64, String)>> {
    let mut stmt = conn.prepare("SELECT id, password_hash FROM users WHERE username = ?1")?;
    let result = stmt.query_row(params![username], |row| Ok((row.get(0)?, row.get(1)?)));
    match result {
        Ok(user) => Ok(Some(user)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Check if a username already exists
pub fn username_exists(conn: &Connection, username: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM users WHERE username = ?1",
        params![username],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Create a new session
pub fn create_session(
    conn: &Connection,
    user_id: i64,
    session_id: &str,
    duration_hours: i64,
) -> Result<()> {
    let now = Utc::now();
    let expires = now + Duration::hours(duration_hours);
    conn.execute(
        "INSERT INTO sessions (id, user_id, created_at, expires_at, last_access_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            session_id,
            user_id,
            now.to_rfc3339(),
            expires.to_rfc3339(),
            now.to_rfc3339()
        ],
    )?;
    Ok(())
}

/// Validate session and get user info, returns (user_id, username)
pub fn get_session_user(conn: &Connection, session_id: &str) -> Result<Option<(i64, String)>> {
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        r#"
        SELECT u.id, u.username
        FROM sessions s
        JOIN users u ON s.user_id = u.id
        WHERE s.id = ?1 AND s.expires_at > ?2
    "#,
    )?;
    let result = stmt.query_row(params![session_id, now], |row| Ok((row.get(0)?, row.get(1)?)));
    match result {
        Ok((user_id, username)) => {
            // Update last access time
            let _ = conn.execute(
                "UPDATE sessions SET last_access_at = ?1 WHERE id = ?2",
                params![now, session_id],
            );
            Ok(Some((user_id, username)))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Delete a session (logout)
pub fn delete_session(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;
    Ok(())
}

/// Delete all sessions for a user
pub fn delete_user_sessions(conn: &Connection, user_id: i64) -> Result<usize> {
    let count = conn.execute("DELETE FROM sessions WHERE user_id = ?1", params![user_id])?;
    Ok(count)
}

/// Cleanup expired sessions, returns count of deleted sessions
pub fn cleanup_expired_sessions(conn: &Connection) -> Result<usize> {
    let now = Utc::now().to_rfc3339();
    let count = conn.execute("DELETE FROM sessions WHERE expires_at < ?1", params![now])?;
    Ok(count)
}

/// Update user's last login timestamp
pub fn update_last_login(conn: &Connection, user_id: i64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE users SET last_login_at = ?1 WHERE id = ?2",
        params![now, user_id],
    )?;
    Ok(())
}

/// Get user count (for migration check)
pub fn get_user_count(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
}

// ==================== Guest Operations ====================

/// Create a guest user, returns the user ID
pub fn create_guest_user(conn: &Connection, username: &str, password_hash: &str) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO users (username, password_hash, created_at, is_guest, last_activity_at) VALUES (?1, ?2, ?3, 1, ?4)",
        params![username, password_hash, now, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get count of regular (non-guest) users
pub fn get_regular_user_count(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM users WHERE is_guest = 0",
        [],
        |row| row.get(0),
    )
}

/// Get count of guest users
pub fn get_guest_count(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM users WHERE is_guest = 1",
        [],
        |row| row.get(0),
    )
}

/// Check if a user is a guest
pub fn is_guest_user(conn: &Connection, user_id: i64) -> Result<bool> {
    let is_guest: i64 = conn.query_row(
        "SELECT COALESCE(is_guest, 0) FROM users WHERE id = ?1",
        params![user_id],
        |row| row.get(0),
    )?;
    Ok(is_guest == 1)
}

/// Update user's last activity timestamp
pub fn update_last_activity(conn: &Connection, user_id: i64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE users SET last_activity_at = ?1 WHERE id = ?2",
        params![now, user_id],
    )?;
    Ok(())
}

/// Cleanup expired guest accounts, returns count of deleted users
/// Also deletes their sessions and returns list of usernames for directory cleanup
pub fn cleanup_expired_guests(conn: &Connection, expiry_hours: i64) -> Result<Vec<String>> {
    let cutoff = (Utc::now() - Duration::hours(expiry_hours)).to_rfc3339();

    // Get usernames of guests to delete (for directory cleanup)
    let mut stmt = conn.prepare(
        "SELECT username FROM users WHERE is_guest = 1 AND last_activity_at < ?1",
    )?;
    let usernames: Vec<String> = stmt
        .query_map(params![cutoff], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // Delete expired guests (sessions cascade delete)
    conn.execute(
        "DELETE FROM users WHERE is_guest = 1 AND last_activity_at < ?1",
        params![cutoff],
    )?;

    Ok(usernames)
}

/// Delete all guest accounts, returns list of usernames for directory cleanup
pub fn delete_all_guests(conn: &Connection) -> Result<Vec<String>> {
    // Get usernames first
    let mut stmt = conn.prepare("SELECT username FROM users WHERE is_guest = 1")?;
    let usernames: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // Delete all guests
    conn.execute("DELETE FROM users WHERE is_guest = 1", [])?;

    Ok(usernames)
}

/// Delete a specific user by ID, returns username for directory cleanup
pub fn delete_user(conn: &Connection, user_id: i64) -> Result<Option<String>> {
    // Get username first
    let username: Option<String> = conn
        .query_row(
            "SELECT username FROM users WHERE id = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .ok();

    if username.is_some() {
        conn.execute("DELETE FROM users WHERE id = ?1", params![user_id])?;
    }

    Ok(username)
}

// ==================== App Settings ====================

/// Get an app setting value
pub fn get_app_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let result = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    );
    match result {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Set an app setting value
pub fn set_app_setting(conn: &Connection, key: &str, value: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

/// Get max users limit (None = infinite, Some(0) = disabled, Some(n) = limit)
pub fn get_max_users(conn: &Connection) -> Result<Option<i64>> {
    get_app_setting(conn, "max_users")?
        .map(|s| s.parse::<i64>())
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)
}

/// Get max guests limit (None = infinite, Some(0) = disabled, Some(n) = limit)
pub fn get_max_guests(conn: &Connection) -> Result<Option<i64>> {
    get_app_setting(conn, "max_guests")?
        .map(|s| s.parse::<i64>())
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)
}

/// Get guest expiry hours (default 24)
pub fn get_guest_expiry_hours(conn: &Connection) -> Result<i64> {
    get_app_setting(conn, "guest_expiry_hours")?
        .and_then(|s| s.parse::<i64>().ok())
        .map(Ok)
        .unwrap_or(Ok(24))
}

/// Check if registration is allowed (under max_users limit)
pub fn can_register_user(conn: &Connection) -> Result<bool> {
    match get_max_users(conn)? {
        None => Ok(true),         // No limit
        Some(0) => Ok(false),     // Registration disabled
        Some(max) => {
            let count = get_regular_user_count(conn)?;
            Ok(count < max)
        }
    }
}

/// Check if guest login is allowed (under max_guests limit)
pub fn can_create_guest(conn: &Connection) -> Result<bool> {
    match get_max_guests(conn)? {
        None => Ok(true),         // No limit
        Some(0) => Ok(false),     // Guests disabled
        Some(max) => {
            let count = get_guest_count(conn)?;
            Ok(count < max)
        }
    }
}
