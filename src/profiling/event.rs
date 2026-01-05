//! Event types for profiling.

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::Serialize;

/// A profiling event with timestamp and optional duration.
#[derive(Serialize)]
pub struct ProfileEvent {
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// The type of event
    pub event_type: EventType,
    /// Duration in microseconds (for timed events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_us: Option<u64>,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl ProfileEvent {
    /// Create a new event with the current timestamp.
    pub fn new(event_type: EventType) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            duration_us: None,
            metadata: None,
        }
    }

    /// Create a new event with duration.
    pub fn with_duration(event_type: EventType, duration: std::time::Duration) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            duration_us: Some(duration.as_micros() as u64),
            metadata: None,
        }
    }

    /// Create a new event with metadata.
    pub fn with_metadata(event_type: EventType, metadata: serde_json::Value) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            duration_us: None,
            metadata: Some(metadata),
        }
    }
}

/// Types of events that can be logged.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventType {
    // === Session lifecycle ===
    /// Profiling session started
    SessionStart {
        /// Session identifier
        session_id: String,
    },
    /// Profiling session ended
    SessionEnd {
        /// Total events logged
        total_events: u64,
    },

    // === Handler lifecycle ===
    /// HTTP handler started processing
    HandlerStart {
        /// Route path (e.g., "/study")
        route: String,
        /// HTTP method
        method: String,
        /// Username (None for public routes)
        username: Option<String>,
    },
    /// HTTP handler finished processing
    HandlerEnd {
        /// Route path
        route: String,
        /// HTTP status code
        status: u16,
        /// Duration in milliseconds
        duration_ms: u64,
        /// Username (None for public routes)
        username: Option<String>,
    },

    // === Auth events ===
    /// User login attempt
    AuthLogin {
        /// Username attempting login
        username: String,
        /// Whether login succeeded
        success: bool,
    },
    /// User registration
    AuthRegister {
        /// Newly registered username
        username: String,
    },
    /// User logout
    AuthLogout {
        /// Username logging out
        username: String,
    },

    // === Database operations ===
    /// Database query started
    DbQuery {
        /// Operation type (select, insert, update, delete)
        operation: String,
        /// Table name
        table: String,
    },
    /// Database query completed
    DbQueryComplete {
        /// Function name that executed the query
        operation: String,
        /// Number of rows affected/returned
        rows: i64,
        /// Duration in milliseconds
        duration_ms: u64,
    },
    /// Database batch operation (tier graduation, bulk updates)
    DbBatchOp {
        /// Operation type
        operation: String,
        /// Table name
        table: String,
        /// Number of rows affected
        rows_affected: i64,
        /// Username performing the operation
        username: String,
    },

    // === SRS calculations ===
    /// Spaced repetition calculation
    SrsCalculation {
        /// Algorithm used (fsrs, sm2)
        algorithm: String,
        /// Card being calculated
        card_id: i64,
        /// User's rating
        rating: u8,
        /// Username
        username: String,
    },

    // === Card selection ===
    /// Card selection logic executed
    CardSelection {
        /// Selection mode (due, unreviewed, practice)
        mode: String,
        /// Sibling card excluded (if any)
        excluded_sibling: Option<i64>,
        /// Number of cards considered
        cards_available: Option<i64>,
        /// Username
        username: String,
    },

    // === Answer validation ===
    /// Answer validation performed
    AnswerValidation {
        /// Card being validated
        card_id: i64,
        /// Whether the answer was correct
        is_correct: bool,
        /// Hint level used (0 = none, 1-3 = hints)
        hints_used: Option<u8>,
        /// Username
        username: String,
    },

    // === Settings ===
    /// Settings were updated
    SettingsUpdate {
        /// Setting name
        setting: String,
        /// New value
        value: String,
        /// Username
        username: String,
    },

    // === Tier management ===
    /// Tier was unlocked
    TierUnlock {
        /// Tier number
        tier: u8,
        /// Username
        username: String,
    },

    // === Listen mode ===
    /// Answer submitted in listen mode
    ListenAnswer {
        /// Tier being practiced
        tier: u8,
        /// Syllable character
        syllable: String,
        /// Whether answer was correct
        is_correct: bool,
        /// Username
        username: String,
    },
    /// Listen session completed or exited
    ListenSession {
        /// Tier being practiced
        tier: u8,
        /// Correct answers
        correct: u8,
        /// Total questions
        total: u8,
        /// Hard mode enabled
        hard_mode: bool,
        /// Username
        username: String,
    },

    // === Audio operations ===
    /// Audio manifest loaded
    AudioManifestLoad {
        /// Lesson identifier
        lesson_id: String,
        /// Number of syllables in manifest
        syllable_count: usize,
        /// Load duration in milliseconds
        duration_ms: u64,
    },
    /// Audio file access attempt
    AudioFileAccess {
        /// Lesson identifier
        lesson_id: String,
        /// Syllable romanization
        syllable: String,
        /// Whether file was found
        found: bool,
    },

    // === Progress tracking ===
    /// Tier progress calculated
    TierProgressCalc {
        /// Tier number
        tier: u8,
        /// Total cards in tier
        total_cards: i64,
        /// Percentage learned
        learned_pct: f64,
        /// Username
        username: String,
    },
    /// Character stats updated
    CharacterStatsUpdate {
        /// Korean character
        character: String,
        /// Whether answer was correct
        correct: bool,
        /// Username
        username: String,
    },

    // === Content pack operations ===
    /// Card skipped during pack enable (duplicate detected)
    PackCardSkipped {
        /// Pack being enabled
        pack_id: String,
        /// Card front text
        front: String,
        /// Card answer
        main_answer: String,
        /// Card type
        card_type: String,
        /// Reason for skip
        reason: String,
    },

    // === Errors ===
    /// Error occurred
    Error {
        /// Context where error occurred
        context: String,
        /// Error type/category
        error_type: String,
        /// Error message
        message: String,
        /// Username (None if not authenticated)
        username: Option<String>,
    },

    // === Timed scope ===
    /// A timed code block completed
    TimedScope {
        /// Name of the scope
        name: String,
        /// Duration in milliseconds
        duration_ms: u64,
    },

    // === Custom events ===
    /// Generic custom event
    Custom {
        /// Event name
        name: String,
        /// Custom data
        data: serde_json::Value,
    },
}
