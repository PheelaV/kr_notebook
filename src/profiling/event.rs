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
    },
    /// HTTP handler finished processing
    HandlerEnd {
        /// Route path
        route: String,
        /// HTTP status code
        status: u16,
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
    },

    // === Settings ===
    /// Settings were updated
    SettingsUpdate {
        /// Setting name
        setting: String,
        /// New value
        value: String,
    },

    // === Timed scope ===
    /// A timed code block completed
    TimedScope {
        /// Name of the scope
        name: String,
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
