//! Project path constants - single source of truth for all file paths.
//!
//! This module centralizes path definitions to avoid hardcoded strings
//! scattered throughout the codebase.

/// Directory for all data files
pub const DATA_DIR: &str = "data";

/// SQLite database path
pub const DB_PATH: &str = "data/hangul.db";

/// Base directory for scraped content
pub const SCRAPED_DIR: &str = "data/scraped";

/// HTSK (How To Study Korean) scraped content directory
pub const HTSK_DIR: &str = "data/scraped/htsk";

/// Python scripts directory (kr-scraper, etc.)
pub const PY_SCRIPTS_DIR: &str = "py_scripts";

/// Diagnostics log directory
pub const DIAGNOSTICS_DIR: &str = "data/diagnostics";

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
