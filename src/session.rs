//! Simple in-memory session storage for study sessions.
//!
//! Stores StudySession state keyed by session ID (from cookie).
//! Sessions auto-expire after a configurable duration of inactivity.

use crate::config;
use crate::srs::StudySession;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Session entry with last access time for expiration
struct SessionEntry {
  session: StudySession,
  last_access: DateTime<Utc>,
}

/// Global session store
static SESSIONS: LazyLock<Mutex<HashMap<String, SessionEntry>>> =
  LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get or create a session for the given ID
pub fn get_session(session_id: &str) -> StudySession {
  let mut sessions = SESSIONS.lock().expect("Session store lock poisoned");

  // Clean up expired sessions occasionally (~10% chance)
  if rand::random::<u8>() < config::SESSION_CLEANUP_THRESHOLD {
    cleanup_expired(&mut sessions);
  }

  // Get existing or create new
  if let Some(entry) = sessions.get_mut(session_id) {
    entry.last_access = Utc::now();
    entry.session.clone()
  } else {
    let session = StudySession::new();
    sessions.insert(
      session_id.to_string(),
      SessionEntry {
        session: session.clone(),
        last_access: Utc::now(),
      },
    );
    session
  }
}

/// Update a session
pub fn update_session(session_id: &str, session: StudySession) {
  let mut sessions = SESSIONS.lock().expect("Session store lock poisoned");
  sessions.insert(
    session_id.to_string(),
    SessionEntry {
      session,
      last_access: Utc::now(),
    },
  );
}

/// Clean up expired sessions
fn cleanup_expired(sessions: &mut HashMap<String, SessionEntry>) {
  let expiry = Utc::now() - Duration::hours(config::SESSION_EXPIRY_HOURS);
  sessions.retain(|_, entry| entry.last_access > expiry);
}

/// Generate a new session ID
pub fn generate_session_id() -> String {
  use rand::Rng;
  let mut rng = rand::rng();
  (0..32)
    .map(|_| {
      let idx = rng.random_range(0..36);
      if idx < 10 {
        (b'0' + idx) as char
      } else {
        (b'a' + idx - 10) as char
      }
    })
    .collect()
}
