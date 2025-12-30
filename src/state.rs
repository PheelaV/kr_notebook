//! Application state and authentication context types.

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Auth database connection (shared across all users)
pub type AuthDb = Arc<Mutex<Connection>>;

/// User's database connection
pub type UserDb = Arc<Mutex<Connection>>;

/// Application state passed to all handlers
#[derive(Clone)]
pub struct AppState {
    /// Shared auth database (users, sessions)
    pub auth_db: AuthDb,

    /// Base path for user data directories (data/users/)
    pub users_data_dir: PathBuf,
}

impl AppState {
    pub fn new(auth_db: AuthDb, users_data_dir: PathBuf) -> Self {
        Self {
            auth_db,
            users_data_dir,
        }
    }

    /// Get path to a user's learning database
    pub fn user_db_path(&self, username: &str) -> PathBuf {
        self.users_data_dir.join(username).join("learning.db")
    }

    /// Get path to a user's data directory
    pub fn user_dir(&self, username: &str) -> PathBuf {
        self.users_data_dir.join(username)
    }
}

/// Authenticated request context (extracted by middleware)
#[derive(Clone)]
pub struct AuthContext {
    pub user_id: i64,
    pub username: String,
    pub user_db: UserDb,
}
