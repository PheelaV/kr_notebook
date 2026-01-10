//! Project path functions - single source of truth for all file paths.
//!
//! This module centralizes path definitions to avoid hardcoded strings
//! scattered throughout the codebase.
//!
//! ## Environment Variables
//!
//! - `DATA_DIR`: Override the base data directory (default: "data")
//! - `PORT`: Override the server port (see config.rs)
//!
//! This allows running multiple isolated server instances for E2E testing:
//! ```bash
//! DATA_DIR=data/test/auth PORT=3001 cargo run
//! DATA_DIR=data/test/study PORT=3002 cargo run
//! ```

use std::env;
use std::sync::OnceLock;

/// Lazily initialized data directory from DATA_DIR env var
static DATA_DIR_VALUE: OnceLock<String> = OnceLock::new();

/// Get the base data directory (from DATA_DIR env var or default "data")
pub fn data_dir() -> &'static str {
    DATA_DIR_VALUE.get_or_init(|| env::var("DATA_DIR").unwrap_or_else(|_| "data".to_string()))
}

/// SQLite database path (legacy single-user, kept for compatibility)
pub fn db_path() -> String {
    format!("{}/hangul.db", data_dir())
}

/// Auth database path (shared, multi-user)
pub fn auth_db_path() -> String {
    format!("{}/app.db", data_dir())
}

/// Users directory (contains per-user learning.db and content)
pub fn users_dir() -> String {
    format!("{}/users", data_dir())
}

/// Base directory for scraped content (legacy location)
pub fn scraped_dir() -> String {
    format!("{}/scraped", data_dir())
}

/// HTSK (How To Study Korean) scraped content directory (legacy)
pub fn htsk_dir() -> String {
    format!("{}/scraped/htsk", data_dir())
}

/// Python scripts directory (kr-scraper, etc.) - not under DATA_DIR
pub const PY_SCRIPTS_DIR: &str = "py_scripts";

/// Diagnostics log directory
pub fn diagnostics_dir() -> String {
    format!("{}/diagnostics", data_dir())
}

// ==================== Content Pack Paths ====================

/// Shared content directory (admin-installed packs and generated content)
pub fn content_dir() -> String {
    format!("{}/content", data_dir())
}

/// Shared packs directory
pub fn shared_packs_dir() -> String {
    format!("{}/content/packs", data_dir())
}

/// Shared generated content directory (scraper output)
pub fn shared_generated_dir() -> String {
    format!("{}/content/generated", data_dir())
}

// ==================== User-specific Paths ====================

/// Get user directory path
pub fn user_dir(username: &str) -> String {
    format!("{}/{username}", users_dir())
}

/// Get user learning database path
pub fn user_db_path(username: &str) -> String {
    format!("{}/{username}/learning.db", users_dir())
}

/// Get user content directory path
pub fn user_content_dir(username: &str) -> String {
    format!("{}/{username}/content", users_dir())
}

/// Get user packs directory path
pub fn user_packs_dir(username: &str) -> String {
    format!("{}/{username}/content/packs", users_dir())
}

/// Get user generated content directory path
pub fn user_generated_dir(username: &str) -> String {
    format!("{}/{username}/content/generated", users_dir())
}

// ==================== Lesson Paths ====================

/// Get the lesson directory path
pub fn lesson_dir(lesson: &str) -> String {
    format!("{}/{lesson}", htsk_dir())
}

/// Get the manifest.json path for a lesson
pub fn manifest_path(lesson: &str) -> String {
    format!("{}/{lesson}/manifest.json", htsk_dir())
}

/// Get the syllables directory path for a lesson
pub fn syllables_dir(lesson: &str) -> String {
    format!("{}/{lesson}/syllables", htsk_dir())
}

/// Get the rows directory path for a lesson
pub fn rows_dir(lesson: &str) -> String {
    format!("{}/{lesson}/rows", htsk_dir())
}

/// Get the columns directory path for a lesson
pub fn columns_dir(lesson: &str) -> String {
    format!("{}/{lesson}/columns", htsk_dir())
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    // Note: We can't easily test env var override because OnceLock
    // initializes once. These tests verify the default behavior.

    #[test]
    fn test_data_dir_default() {
        // Can't test env override due to OnceLock, but verify it returns a value
        let dir = data_dir();
        assert!(!dir.is_empty());
    }

    #[test]
    fn test_auth_db_path_format() {
        let path = auth_db_path();
        assert!(path.ends_with("/app.db"));
    }

    #[test]
    fn test_users_dir_format() {
        let path = users_dir();
        assert!(path.ends_with("/users"));
    }

    #[test]
    fn test_user_db_path() {
        let path = user_db_path("alice");
        assert!(path.contains("/alice/learning.db"));
    }

    #[test]
    fn test_shared_packs_dir_format() {
        let path = shared_packs_dir();
        assert!(path.ends_with("/content/packs"));
    }

    #[test]
    fn test_lesson_paths() {
        let dir = lesson_dir("lesson1");
        assert!(dir.ends_with("/lesson1"));

        let manifest = manifest_path("lesson1");
        assert!(manifest.ends_with("/lesson1/manifest.json"));
    }
}
