//! Compile-time conditional profiling system.
//!
//! When the `profiling` feature is enabled, this module provides JSONL-based
//! event logging for performance analysis. When disabled, all functions are
//! no-ops with zero runtime cost.
//!
//! # Usage
//!
//! ```rust
//! use kr_notebook::profile_log;
//! use kr_notebook::profiling::EventType;
//!
//! // Log an event
//! profile_log!(EventType::HandlerStart {
//!     route: "/study".into(),
//!     method: "GET".into()
//! });
//! ```

#[cfg(feature = "profiling")]
mod event;
#[cfg(feature = "profiling")]
mod logger;

#[cfg(feature = "profiling")]
pub use event::*;
#[cfg(feature = "profiling")]
pub use logger::*;

#[cfg(not(feature = "profiling"))]
mod noop;
#[cfg(not(feature = "profiling"))]
pub use noop::*;

// Macros are defined here to be available at crate root

/// Log a profiling event.
///
/// When the `profiling` feature is disabled, this macro expands to nothing.
///
/// # Examples
///
/// ```rust
/// profile_log!(EventType::DbQuery {
///     operation: "select".into(),
///     table: "cards".into()
/// });
/// ```
#[cfg(feature = "profiling")]
#[macro_export]
macro_rules! profile_log {
    ($event_type:expr) => {
        $crate::profiling::log_event($event_type)
    };
    ($event_type:expr, $meta:expr) => {
        $crate::profiling::log_event_with_meta($event_type, $meta)
    };
}

#[cfg(not(feature = "profiling"))]
#[macro_export]
macro_rules! profile_log {
    ($($args:tt)*) => {};
}

/// Execute a block and log its duration.
///
/// When the `profiling` feature is disabled, this macro just executes the block.
///
/// # Examples
///
/// ```rust
/// let result = profile_scope!("database_query", {
///     db::get_due_cards(&conn, 10, None)
/// });
/// ```
#[cfg(feature = "profiling")]
#[macro_export]
macro_rules! profile_scope {
    ($name:expr, $body:block) => {{
        let _start = std::time::Instant::now();
        let result = $body;
        $crate::profiling::log_timed($name, _start.elapsed());
        result
    }};
}

#[cfg(not(feature = "profiling"))]
#[macro_export]
macro_rules! profile_scope {
    ($name:expr, $body:block) => {
        $body
    };
}
