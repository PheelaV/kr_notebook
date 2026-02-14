//! Offline study mode handlers.
//!
//! Provides endpoints for downloading study sessions for offline use
//! and syncing results back after returning online.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::auth::AuthContext;
use crate::db::{self, LogOnError};
use crate::domain::{Card, CardType, FsrsState};
use crate::srs::fsrs_scheduler::calculate_fsrs_review_at;
use crate::state::AppState;

use super::{generate_choices, is_korean, parse_filter_mode};

/// Cards per minute estimate for session duration calculation
const CARDS_PER_MINUTE: f64 = 1.5;

// ============================================================================
// Download Session
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct DownloadSessionRequest {
    /// Desired session duration in minutes. If omitted or 0, uses saved user preference.
    #[serde(default)]
    pub duration_minutes: Option<u32>,
    /// Filter mode: "all", "hangul", "pack:X", "pack:X:lesson:N"
    #[serde(default = "default_filter")]
    pub filter_mode: String,
}

fn default_filter() -> String {
    "all".to_string()
}

/// Card data for offline study (includes SRS state for client-side scheduling)
#[derive(Debug, Serialize)]
pub struct OfflineCard {
    pub card_id: i64,
    pub front: String,
    pub back: String,
    pub description: Option<String>,
    pub card_type: String,
    pub tier: u8,
    pub is_reverse: bool,
    /// Pre-generated multiple choice options (if answer is Korean)
    pub choices: Option<Vec<String>>,
    // Current SRS state (for WASM to calculate next state)
    pub learning_step: i64,
    pub fsrs_stability: Option<f64>,
    pub fsrs_difficulty: Option<f64>,
    pub repetitions: i64,
    /// ISO8601 timestamp
    pub next_review: String,
    /// Audio URL for pronunciation (if available and audio enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DownloadSessionResponse {
    pub session_id: String,
    pub created_at: String,
    pub desired_retention: f64,
    pub focus_mode: bool,
    pub cards: Vec<OfflineCard>,
    /// Audio URLs to precache (if audio enabled)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub audio_urls: Vec<String>,
}

/// Download a study session for offline use.
///
/// POST /api/study/download-session
pub async fn download_session(
    auth: AuthContext,
    State(state): State<AppState>,
    Json(request): Json<DownloadSessionRequest>,
) -> impl IntoResponse {
    let conn = auth.user_db.lock().unwrap();
    let app_conn = state.auth_db.lock().unwrap();

    // Check if offline mode is enabled
    let offline_enabled = db::get_setting(&conn, "offline_mode_enabled")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);

    if !offline_enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Offline mode is not enabled. Enable it in Settings first."
            })),
        )
            .into_response();
    }

    // Get user settings
    let desired_retention = db::get_desired_retention(&conn)
        .log_warn_default("Failed to get desired_retention");
    let focus_mode = db::is_focus_mode_enabled(&conn)
        .log_warn_default("Failed to get focus_mode");

    // Calculate target card count, falling back to saved preference if not specified
    let duration_minutes = request
        .duration_minutes
        .filter(|&d| d > 0)
        .unwrap_or_else(|| {
            db::get_setting(&conn, "offline_session_duration")
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30u32)
        });
    let target_cards = (duration_minutes as f64 * CARDS_PER_MINUTE).ceil() as usize;

    // Parse filter mode
    let filter = parse_filter_mode(&request.filter_mode);

    // Get available cards (no sibling exclusion for batch download)
    let all_cards = super::get_available_study_cards_filtered(
        &conn,
        &app_conn,
        auth.user_id,
        &filter,
        None, // No last_card_id for batch download
        target_cards,
    );

    if all_cards.is_empty() {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "error": "No cards available for study with current filter."
            })),
        )
            .into_response();
    }

    // Select cards for the session (with some randomization)
    let mut selected_cards = all_cards.clone();
    let mut rng = rand::rng();
    selected_cards.shuffle(&mut rng);
    selected_cards.truncate(target_cards.max(10)); // Minimum 10 cards

    // Generate session ID
    let session_id = generate_session_id();
    let now = Utc::now();

    // Convert cards to offline format
    let offline_cards: Vec<OfflineCard> = selected_cards
        .iter()
        .map(|card| {
            let choices = if is_korean(&card.main_answer) {
                Some(generate_choices(card, &all_cards))
            } else {
                None
            };

            OfflineCard {
                card_id: card.id,
                front: card.front.clone(),
                back: card.main_answer.clone(),
                description: card.description.clone(),
                card_type: card.card_type.as_str().to_string(),
                tier: card.tier,
                is_reverse: card.is_reverse,
                choices,
                learning_step: card.learning_step,
                fsrs_stability: card.fsrs_stability,
                fsrs_difficulty: card.fsrs_difficulty,
                repetitions: card.repetitions as i64,
                next_review: card.next_review.to_rfc3339(),
                // TODO: Resolve audio URL when audio integration is added
                audio_url: None,
            }
        })
        .collect();

    // Record session in database
    let _ = conn.execute(
        "INSERT INTO offline_sessions (session_id, created_at, card_count, filter_mode, synced)
         VALUES (?1, ?2, ?3, ?4, 0)",
        rusqlite::params![
            &session_id,
            now.to_rfc3339(),
            offline_cards.len() as i32,
            &request.filter_mode
        ],
    );

    // TODO: Collect audio URLs when audio integration is added
    let audio_urls: Vec<String> = Vec::new();

    let response = DownloadSessionResponse {
        session_id,
        created_at: now.to_rfc3339(),
        desired_retention,
        focus_mode,
        cards: offline_cards,
        audio_urls,
    };

    (StatusCode::OK, Json(response)).into_response()
}

// ============================================================================
// Sync Session
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SyncReview {
    pub card_id: i64,
    pub quality: u8,
    pub is_correct: bool,
    pub hints_used: u8,
    /// ISO8601 timestamp when review occurred
    pub timestamp: String,
    // Final SRS state after this review (calculated by WASM)
    pub learning_step: i64,
    pub fsrs_stability: Option<f64>,
    pub fsrs_difficulty: Option<f64>,
    /// ISO8601 timestamp
    pub next_review: String,
    // Override fields (optional)
    #[serde(default)]
    pub is_override: bool,
    pub user_answer: Option<String>,
    pub original_result: Option<String>,
    pub suggested_answer: Option<String>,
    // Pre-state for override restoration
    pub pre_learning_step: Option<i64>,
    pub pre_fsrs_stability: Option<f64>,
    pub pre_fsrs_difficulty: Option<f64>,
    pub pre_next_review: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SyncSessionRequest {
    pub session_id: String,
    pub reviews: Vec<SyncReview>,
}

#[derive(Debug, Serialize)]
pub struct SyncSessionResponse {
    pub synced_count: usize,
    pub skipped_count: usize,
    pub skipped_cards: Vec<SkippedCard>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SkippedCard {
    pub card_id: i64,
    pub reason: String,
}

/// Sync offline study session results back to the server.
///
/// POST /api/study/sync-offline
pub async fn sync_session(
    auth: AuthContext,
    State(state): State<AppState>,
    Json(request): Json<SyncSessionRequest>,
) -> impl IntoResponse {
    let conn = auth.user_db.lock().unwrap();
    let app_conn = state.auth_db.lock().unwrap();

    // Verify session exists and belongs to user (by being in their DB)
    let session_exists: bool = conn
        .query_row(
            "SELECT 1 FROM offline_sessions WHERE session_id = ?1 AND synced = 0",
            [&request.session_id],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !session_exists {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Session not found or already synced"
            })),
        )
            .into_response();
    }

    // Get user settings for FSRS calculation
    let desired_retention = db::get_desired_retention(&conn)
        .log_warn_default("Failed to get desired_retention for sync");
    let focus_mode = db::is_focus_mode_enabled(&conn)
        .log_warn_default("Failed to get focus_mode for sync");

    let mut synced_count = 0;
    let mut skipped_cards = Vec::new();
    let mut errors = Vec::new();

    // Get offline session download time for conflict detection
    let session_download_time: Option<DateTime<Utc>> = conn
        .query_row(
            "SELECT created_at FROM offline_sessions WHERE session_id = ?1",
            [&request.session_id],
            |row| {
                let ts: String = row.get(0)?;
                Ok(chrono::DateTime::parse_from_rfc3339(&ts)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok())
            },
        )
        .ok()
        .flatten();

    // Sort reviews by timestamp for correct ordering
    let mut reviews = request.reviews;
    reviews.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    // Begin transaction for atomic sync
    if let Err(e) = conn.execute("BEGIN IMMEDIATE", []) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to begin transaction: {}", e)
            })),
        )
            .into_response();
    }

    // Process each review with server-side FSRS calculation
    for review in &reviews {
        // Parse review timestamp - reject invalid timestamps instead of using current time
        let review_time: DateTime<Utc> = match chrono::DateTime::parse_from_rfc3339(&review.timestamp) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(e) => {
                errors.push(format!("Card {}: invalid timestamp '{}': {}", review.card_id, review.timestamp, e));
                continue;
            }
        };

        // Validate timestamp is within reasonable bounds
        // Allow tolerance for clock skew and offline study (reviews can be timestamped
        // slightly ahead if synced shortly after the review was made)
        let now = Utc::now();
        let tolerance = chrono::Duration::minutes(5);
        if let Some(download_time) = session_download_time {
            if review_time < download_time - tolerance {
                errors.push(format!(
                    "Card {}: review timestamp {} is before session download {}",
                    review.card_id, review_time, download_time
                ));
                continue;
            }
        }
        if review_time > now + tolerance {
            errors.push(format!(
                "Card {}: review timestamp {} is in the future",
                review.card_id, review_time
            ));
            continue;
        }

        // CONFLICT DETECTION: Check if card was reviewed online after session download
        let last_online_review = get_last_review_time(&conn, review.card_id);
        if let (Some(download_time), Some(last_review)) = (session_download_time, last_online_review) {
            if last_review > download_time {
                // Card was reviewed online after this offline session was downloaded
                // Skip to avoid resetting progress
                skipped_cards.push(SkippedCard {
                    card_id: review.card_id,
                    reason: format!(
                        "Card reviewed online at {} (after session download at {})",
                        last_review.format("%H:%M:%S"),
                        download_time.format("%H:%M:%S")
                    ),
                });
                continue;
            }
        }

        // Get current card progress (for FSRS input state)
        let card_state = get_card_progress_for_sync(&conn, review.card_id);

        // Build a minimal Card struct for FSRS calculation
        let card = Card {
            id: review.card_id,
            front: String::new(),
            main_answer: String::new(),
            description: None,
            card_type: CardType::Consonant, // Doesn't affect FSRS
            tier: 1,
            audio_hint: None,
            is_reverse: false,
            pack_id: None,
            lesson: None,
            ease_factor: card_state.ease_factor,
            interval_days: card_state.interval_days,
            repetitions: card_state.repetitions,
            next_review: card_state.next_review,
            learning_step: card_state.learning_step,
            fsrs_stability: card_state.fsrs_stability,
            fsrs_difficulty: card_state.fsrs_difficulty,
            fsrs_state: card_state.fsrs_state,
            total_reviews: card_state.total_reviews,
            correct_reviews: card_state.correct_reviews,
        };

        // Calculate next review using server-side FSRS at the offline review time
        let fsrs_result = calculate_fsrs_review_at(
            &card,
            review.quality,
            desired_retention,
            focus_mode,
            review_time,
        );

        // Use server-calculated quality for is_correct to ensure consistency
        let is_correct_from_quality = review.quality >= 2;

        // Update card_progress with server-calculated SRS state
        let update_result = conn.execute(
            r#"
            INSERT INTO card_progress (
                card_id, ease_factor, interval_days, repetitions, next_review,
                total_reviews, correct_reviews, learning_step,
                fsrs_stability, fsrs_difficulty, fsrs_state
            ) VALUES (
                ?1, 2.5, 0, ?2, ?3,
                0, 0, ?4,
                ?5, ?6, ?7
            )
            ON CONFLICT(card_id) DO UPDATE SET
                repetitions = ?2,
                next_review = ?3,
                total_reviews = total_reviews + 1,
                correct_reviews = correct_reviews + ?8,
                learning_step = ?4,
                fsrs_stability = ?5,
                fsrs_difficulty = ?6,
                fsrs_state = ?7
            "#,
            rusqlite::params![
                review.card_id,
                fsrs_result.repetitions,
                fsrs_result.next_review.to_rfc3339(),
                fsrs_result.learning_step,
                fsrs_result.stability,
                fsrs_result.difficulty,
                fsrs_result.state.as_str(),
                if is_correct_from_quality { 1 } else { 0 },
            ],
        );

        if let Err(e) = update_result {
            errors.push(format!("Card {}: {}", review.card_id, e));
            continue;
        }

        // Determine study mode (Offline or Override)
        let study_mode = if review.is_override { "Override" } else { "Offline" };

        // Insert review log
        let log_result = conn.execute(
            r#"
            INSERT INTO review_logs (
                card_id, quality, reviewed_at, is_correct, study_mode, hints_used
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            rusqlite::params![
                review.card_id,
                review.quality,
                review_time.to_rfc3339(),
                if review.is_correct { 1 } else { 0 },
                study_mode,
                review.hints_used,
            ],
        );

        if let Err(e) = log_result {
            errors.push(format!("Review log for card {}: {}", review.card_id, e));
        }

        // For overrides, insert validation suggestion for admin review
        if review.is_override {
            if let Some(ref suggested) = review.suggested_answer {
                // Get card info from app.db for the suggestion
                let card_info: Option<(String, String)> = app_conn
                    .query_row(
                        r#"
                        SELECT front, main_answer
                        FROM card_definitions
                        WHERE id = ?1
                        "#,
                        [review.card_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .ok();

                if let Some((card_front, expected_answer)) = card_info {
                    let _ = app_conn.execute(
                        r#"
                        INSERT INTO validation_suggestions (
                            card_id, card_front, expected_answer, user_answer,
                            suggested_answer, user_quality, username, created_at,
                            original_result
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                        "#,
                        rusqlite::params![
                            review.card_id,
                            card_front,
                            expected_answer,
                            review.user_answer.as_deref().unwrap_or(""),
                            suggested,
                            review.quality,
                            &auth.username,
                            review_time.to_rfc3339(),
                            review.original_result.as_deref().unwrap_or(""),
                        ],
                    );
                }
            }
        }

        synced_count += 1;
    }

    // Mark session as synced
    let _ = conn.execute(
        "UPDATE offline_sessions SET synced = 1, synced_at = ?1 WHERE session_id = ?2",
        rusqlite::params![Utc::now().to_rfc3339(), &request.session_id],
    );

    // Commit transaction
    if let Err(e) = conn.execute("COMMIT", []) {
        // Try to rollback on commit failure
        let _ = conn.execute("ROLLBACK", []);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to commit transaction: {}", e)
            })),
        )
            .into_response();
    }

    // Check for tier and pack lesson unlocks after syncing reviews
    let _ = db::try_auto_unlock_tier(&conn).log_warn("Auto tier unlock failed");
    let _ = db::try_auto_unlock_all_pack_lessons(&conn, &app_conn)
        .log_warn("Auto lesson unlock failed");

    let skipped_count = skipped_cards.len();
    let response = SyncSessionResponse {
        synced_count,
        skipped_count,
        skipped_cards,
        errors,
    };

    (StatusCode::OK, Json(response)).into_response()
}

// ============================================================================
// Helpers
// ============================================================================

fn generate_session_id() -> String {
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

/// Card progress state for FSRS calculation during sync
struct CardProgressState {
    ease_factor: f64,
    interval_days: i64,
    repetitions: i64,
    next_review: DateTime<Utc>,
    learning_step: i64,
    fsrs_stability: Option<f64>,
    fsrs_difficulty: Option<f64>,
    fsrs_state: Option<FsrsState>,
    total_reviews: i64,
    correct_reviews: i64,
}

impl Default for CardProgressState {
    fn default() -> Self {
        Self {
            ease_factor: 2.5,
            interval_days: 0,
            repetitions: 0,
            next_review: Utc::now(),
            learning_step: 0,
            fsrs_stability: None,
            fsrs_difficulty: None,
            fsrs_state: None,
            total_reviews: 0,
            correct_reviews: 0,
        }
    }
}

/// Get current card progress from database for FSRS calculation
fn get_card_progress_for_sync(conn: &rusqlite::Connection, card_id: i64) -> CardProgressState {
    conn.query_row(
        r#"
        SELECT ease_factor, interval_days, repetitions, next_review,
               learning_step, fsrs_stability, fsrs_difficulty, fsrs_state,
               total_reviews, correct_reviews
        FROM card_progress WHERE card_id = ?1
        "#,
        [card_id],
        |row| {
            let next_review_str: String = row.get(3)?;
            let next_review = chrono::DateTime::parse_from_rfc3339(&next_review_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let fsrs_state_str: Option<String> = row.get(7)?;
            let fsrs_state = fsrs_state_str.and_then(|s| match s.as_str() {
                "New" => Some(FsrsState::New),
                "Learning" => Some(FsrsState::Learning),
                "Review" => Some(FsrsState::Review),
                "Relearning" => Some(FsrsState::Relearning),
                _ => None,
            });

            Ok(CardProgressState {
                ease_factor: row.get(0)?,
                interval_days: row.get(1)?,
                repetitions: row.get(2)?,
                next_review,
                learning_step: row.get(4)?,
                fsrs_stability: row.get(5)?,
                fsrs_difficulty: row.get(6)?,
                fsrs_state,
                total_reviews: row.get(8)?,
                correct_reviews: row.get(9)?,
            })
        },
    )
    .unwrap_or_default()
}

/// Get the timestamp of the most recent review for a card
fn get_last_review_time(conn: &rusqlite::Connection, card_id: i64) -> Option<DateTime<Utc>> {
    conn.query_row(
        "SELECT reviewed_at FROM review_logs WHERE card_id = ?1 ORDER BY reviewed_at DESC LIMIT 1",
        [card_id],
        |row| {
            let ts: String = row.get(0)?;
            Ok(chrono::DateTime::parse_from_rfc3339(&ts)
                .map(|dt| dt.with_timezone(&Utc))
                .ok())
        },
    )
    .ok()
    .flatten()
}
