//! Application configuration constants.
//!
//! This module centralizes all configurable values that were previously
//! hardcoded throughout the codebase.

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
        name: "Tier 3: Aspirated & Tense",
        short_name: "Aspirated & Tense Consonants",
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

// ==================== Query Limits ====================

/// Default limit for card queries
pub const DEFAULT_CARD_LIMIT: i64 = 50;

/// Limit for problem cards display
pub const PROBLEM_CARDS_LIMIT: i64 = 5;

/// Limit for confusion entries per card
pub const CONFUSIONS_LIMIT: i64 = 3;
