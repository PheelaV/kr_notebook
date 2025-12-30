//! Auth database operations (users, sessions tables).

use chrono::{Duration, Utc};
use rusqlite::{params, Connection, Result};

/// Initialize the auth database schema
pub fn init_auth_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE COLLATE NOCASE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_login_at TEXT
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            last_access_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
    "#,
    )?;
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
