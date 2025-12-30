//! JSONL file logger for profiling events.

#![allow(dead_code)]

use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;

use super::event::{EventType, ProfileEvent};

/// Global logger instance - must be initialized via init().
static LOGGER: Mutex<Option<ProfileLogger>> = Mutex::new(None);

/// Event counter for session statistics.
static EVENT_COUNT: AtomicU64 = AtomicU64::new(0);

/// The profile logger that writes events to a JSONL file.
pub struct ProfileLogger {
    writer: BufWriter<File>,
    session_id: String,
}

impl ProfileLogger {
    /// Create a new logger with a timestamped filename.
    fn new() -> std::io::Result<Self> {
        let now = Utc::now();
        let session_id = now.format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("data/profile_{}.jsonl", session_id);

        // Ensure data directory exists
        create_dir_all("data")?;

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filename)?;

        tracing::info!("Profiling enabled: writing to {}", filename);

        Ok(Self {
            writer: BufWriter::new(file),
            session_id,
        })
    }

    /// Write an event to the log file and console.
    fn log(&mut self, event: ProfileEvent) {
        if let Ok(json) = serde_json::to_string(&event) {
            // Write to file
            let _ = writeln!(self.writer, "{}", json);
            // Flush periodically for durability (every 100 events)
            if EVENT_COUNT.load(Ordering::Relaxed) % 100 == 0 {
                let _ = self.writer.flush();
            }
            // Also write to console
            println!("[PROFILE] {}", json);
        }
        EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// Flush all buffered data.
    fn flush(&mut self) {
        let _ = self.writer.flush();
    }

    /// Get the session ID.
    fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Initialize the profiler. Call this from main() before any logging.
///
/// Creates a new log file with a timestamped name in the data/ directory.
pub fn init() {
    let mut guard = LOGGER.lock().expect("Profiler lock poisoned");
    if guard.is_some() {
        tracing::warn!("Profiler already initialized");
        return;
    }

    match ProfileLogger::new() {
        Ok(logger) => {
            let session_id = logger.session_id().to_string();
            *guard = Some(logger);

            // Log session start
            drop(guard); // Release lock before logging
            log_event(EventType::SessionStart { session_id });
        }
        Err(e) => {
            tracing::error!("Failed to initialize profiler: {}", e);
        }
    }
}

/// Shutdown the profiler and flush remaining events.
///
/// Call this before application exit to ensure all events are written.
pub fn shutdown() {
    let total_events = EVENT_COUNT.load(Ordering::Relaxed);

    // Log session end before shutting down
    log_event(EventType::SessionEnd { total_events });

    let mut guard = LOGGER.lock().expect("Profiler lock poisoned");
    if let Some(ref mut logger) = *guard {
        logger.flush();
        tracing::info!(
            "Profiling session ended: {} events logged",
            total_events
        );
    }
    *guard = None;
}

/// Log a profiling event.
pub fn log_event(event_type: EventType) {
    let event = ProfileEvent::new(event_type);
    if let Ok(mut guard) = LOGGER.lock() {
        if let Some(ref mut logger) = *guard {
            logger.log(event);
        }
    }
}

/// Log a profiling event with additional metadata.
pub fn log_event_with_meta(event_type: EventType, metadata: serde_json::Value) {
    let event = ProfileEvent::with_metadata(event_type, metadata);
    if let Ok(mut guard) = LOGGER.lock() {
        if let Some(ref mut logger) = *guard {
            logger.log(event);
        }
    }
}

/// Log a timed scope completion.
pub fn log_timed(name: &str, duration: Duration) {
    let event = ProfileEvent::with_duration(
        EventType::TimedScope {
            name: name.to_string(),
            duration_ms: duration.as_millis() as u64,
        },
        duration,
    );
    if let Ok(mut guard) = LOGGER.lock() {
        if let Some(ref mut logger) = *guard {
            logger.log(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = ProfileEvent::new(EventType::HandlerStart {
            route: "/study".into(),
            method: "GET".into(),
        });
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("handler_start"));
        assert!(json.contains("/study"));
    }
}
