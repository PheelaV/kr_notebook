//! Project path constants - single source of truth for all file paths.
//!
//! This module centralizes path definitions to avoid hardcoded strings
//! scattered throughout the codebase.

/// Directory for all data files
pub const DATA_DIR: &str = "data";

/// SQLite database path (legacy single-user, kept for compatibility)
pub const DB_PATH: &str = "data/hangul.db";

/// Auth database path (shared, multi-user)
pub const AUTH_DB_PATH: &str = "data/app.db";

/// Users directory (contains per-user learning.db and content)
pub const USERS_DIR: &str = "data/users";

/// Base directory for scraped content (legacy location)
pub const SCRAPED_DIR: &str = "data/scraped";

/// HTSK (How To Study Korean) scraped content directory (legacy)
pub const HTSK_DIR: &str = "data/scraped/htsk";

/// Python scripts directory (kr-scraper, etc.)
pub const PY_SCRIPTS_DIR: &str = "py_scripts";

/// Diagnostics log directory
pub const DIAGNOSTICS_DIR: &str = "data/diagnostics";

// ==================== Content Pack Paths ====================

/// Shared content directory (admin-installed packs and generated content)
pub const CONTENT_DIR: &str = "data/content";

/// Shared packs directory
pub const SHARED_PACKS_DIR: &str = "data/content/packs";

/// Shared generated content directory (scraper output)
pub const SHARED_GENERATED_DIR: &str = "data/content/generated";

/// Get user directory path
pub fn user_dir(username: &str) -> String {
    format!("{USERS_DIR}/{username}")
}

/// Get user learning database path
pub fn user_db_path(username: &str) -> String {
    format!("{USERS_DIR}/{username}/learning.db")
}

/// Get user content directory path
pub fn user_content_dir(username: &str) -> String {
    format!("{USERS_DIR}/{username}/content")
}

/// Get user packs directory path
pub fn user_packs_dir(username: &str) -> String {
    format!("{USERS_DIR}/{username}/content/packs")
}

/// Get user generated content directory path
pub fn user_generated_dir(username: &str) -> String {
    format!("{USERS_DIR}/{username}/content/generated")
}

/// Get the lesson directory path
pub fn lesson_dir(lesson: &str) -> String {
    format!("{HTSK_DIR}/{lesson}")
}

/// Get the manifest.json path for a lesson
pub fn manifest_path(lesson: &str) -> String {
    format!("{HTSK_DIR}/{lesson}/manifest.json")
}

/// Get the syllables directory path for a lesson
pub fn syllables_dir(lesson: &str) -> String {
    format!("{HTSK_DIR}/{lesson}/syllables")
}

/// Get the rows directory path for a lesson
pub fn rows_dir(lesson: &str) -> String {
    format!("{HTSK_DIR}/{lesson}/rows")
}

/// Get the columns directory path for a lesson
pub fn columns_dir(lesson: &str) -> String {
    format!("{HTSK_DIR}/{lesson}/columns")
}
