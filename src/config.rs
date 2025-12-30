//! Application configuration constants.
//!
//! This module centralizes all configurable values that were previously
//! hardcoded throughout the codebase.

use serde::Deserialize;
use std::path::PathBuf;

// ==================== Database Configuration ====================

/// Configuration file structure for config.toml
#[derive(Debug, Deserialize)]
struct AppConfig {
    database: Option<DatabaseConfig>,
}

#[derive(Debug, Deserialize)]
struct DatabaseConfig {
    path: Option<String>,
}

/// Load database path with priority: config.toml > .env > default
pub fn load_database_path() -> PathBuf {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Priority 1: config.toml
    if let Ok(contents) = std::fs::read_to_string("config.toml") {
        if let Ok(config) = toml::from_str::<AppConfig>(&contents) {
            if let Some(db) = config.database {
                if let Some(path) = db.path {
                    tracing::info!("Using database from config.toml: {}", path);
                    return PathBuf::from(path);
                }
            }
        }
    }

    // Priority 2: .env DATABASE_PATH
    if let Ok(path) = std::env::var("DATABASE_PATH") {
        tracing::info!("Using database from DATABASE_PATH env: {}", path);
        return PathBuf::from(path);
    }

    // Default
    let default = PathBuf::from("data/hangul.db");
    tracing::info!("Using default database path: {}", default.display());
    default
}

// ==================== Server Configuration ====================

/// Server address to bind to
pub const SERVER_ADDR: &str = "0.0.0.0";

/// Server port
pub const SERVER_PORT: u16 = 3000;

/// Get the full server bind address
pub fn server_bind_addr() -> String {
    format!("{}:{}", SERVER_ADDR, SERVER_PORT)
}

// ==================== Session Configuration ====================

/// Session expiration time in hours
pub const SESSION_EXPIRY_HOURS: i64 = 1;

/// Probability threshold for session cleanup (0-255, lower = more frequent)
/// Value of 25 means ~10% chance (25/256) on each session access
pub const SESSION_CLEANUP_THRESHOLD: u8 = 25;

// ==================== Tier Configuration ====================

/// Tier information struct
pub struct TierInfo {
    pub tier: u8,
    pub name: &'static str,
    pub short_name: &'static str,
    pub lesson_id: &'static str,
}

/// All tier definitions
pub const TIERS: [TierInfo; 4] = [
    TierInfo {
        tier: 1,
        name: "Lesson 1: Basic Consonants",
        short_name: "Basic Consonants & Vowels",
        lesson_id: "lesson1",
    },
    TierInfo {
        tier: 2,
        name: "Lesson 2: Y-Vowels & Special",
        short_name: "Y-Vowels & Special",
        lesson_id: "lesson2",
    },
    TierInfo {
        tier: 3,
        name: "Lesson 3: Diphthongs & Combined Vowels",
        short_name: "Diphthongs & Combined Vowels",
        lesson_id: "lesson3",
    },
    TierInfo {
        tier: 4,
        name: "Tier 4: Compound Vowels",
        short_name: "Compound Vowels",
        lesson_id: "lesson4",
    },
];

/// Get tier info by tier number
pub fn get_tier_info(tier: u8) -> Option<&'static TierInfo> {
    TIERS.iter().find(|t| t.tier == tier)
}

/// Get tier name by tier number
pub fn get_tier_name(tier: u8) -> String {
    get_tier_info(tier)
        .map(|t| t.short_name.to_string())
        .unwrap_or_else(|| format!("Tier {}", tier))
}

/// Get tier lesson ID and display name for listen mode
pub fn get_listen_tier_info(tier: u8) -> Option<(&'static str, &'static str)> {
    get_tier_info(tier).map(|t| (t.lesson_id, t.name))
}

// ==================== Study Configuration ====================

/// Number of distractor choices in multiple choice mode
pub const DISTRACTOR_COUNT: usize = 3;

// ==================== Query Limits ====================

/// Default limit for card queries
pub const DEFAULT_CARD_LIMIT: i64 = 50;

/// Limit for problem cards display
pub const PROBLEM_CARDS_LIMIT: i64 = 5;

/// Limit for confusion entries per card
pub const CONFUSIONS_LIMIT: i64 = 3;

// ==================== SRS Learning Steps ====================

/// Normal learning steps in minutes: 1min → 10min → 1hr → 4hr (~5 hours to graduate)
pub const LEARNING_STEPS_NORMAL: [i64; 4] = [1, 10, 60, 240];

/// Focus mode learning steps in minutes: 1min → 5min → 15min → 30min (~50 minutes to graduate)
pub const LEARNING_STEPS_FOCUS: [i64; 4] = [1, 5, 15, 30];

/// Get learning steps based on focus mode
pub fn get_learning_steps(focus_mode: bool) -> &'static [i64; 4] {
    if focus_mode {
        &LEARNING_STEPS_FOCUS
    } else {
        &LEARNING_STEPS_NORMAL
    }
}
