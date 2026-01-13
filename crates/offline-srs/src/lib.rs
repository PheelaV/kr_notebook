//! WASM module for offline study mode.
//!
//! Provides answer validation and FSRS scheduling that can run in the browser.

use wasm_bindgen::prelude::*;
use chrono::{DateTime, Utc};
use rs_fsrs::{FSRS, Card, Rating};
use serde::{Deserialize, Serialize};

#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

// ============================================================================
// Answer Validation (ported from src/validation.rs)
// ============================================================================

/// Result of answer validation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnswerResult {
    Correct,
    CloseEnough,
    Incorrect,
}

impl AnswerResult {
    pub fn to_quality(&self, used_hint: bool) -> u8 {
        match (self, used_hint) {
            (Self::Correct, false) => 4,
            (Self::Correct, true) => 2,
            (Self::CloseEnough, _) => 2,
            (Self::Incorrect, _) => 0,
        }
    }
}

/// Normalize an answer for comparison
fn normalize_answer(input: &str) -> String {
    input
        .to_lowercase()
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '/')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract acceptable answer variants from main answer
fn extract_variants(main_answer: &str) -> Vec<String> {
    let mut variants = Vec::new();
    let normalized = normalize_answer(main_answer);
    variants.push(normalized.clone());

    if main_answer.contains(" / ") || main_answer.contains("/") {
        let parts: Vec<&str> = main_answer
            .split('/')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for part in &parts {
            let normalized_part = normalize_answer(part);
            if !normalized_part.is_empty() && !variants.contains(&normalized_part) {
                variants.push(normalized_part);
            }
        }

        let joined = parts.join("/");
        let normalized_joined = normalize_answer(&joined);
        if !variants.contains(&normalized_joined) {
            variants.push(normalized_joined);
        }
    }

    variants
}

/// Levenshtein distance between two strings
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len { matrix[i][0] = i; }
    for j in 0..=b_len { matrix[0][j] = j; }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

/// Parse phonetic answer into letter and optional modifier
fn parse_phonetic_answer(answer: &str) -> (String, Option<String>) {
    let normalized = normalize_answer(answer);
    let parts: Vec<&str> = normalized.split_whitespace().collect();

    if parts.len() >= 2 {
        let last_part = parts[parts.len() - 1];
        let letter_parts = &parts[..parts.len() - 1];
        let letter_part = letter_parts.join(" ");

        let modifiers = ["tense", "aspirated"];
        for modifier in &modifiers {
            if last_part == *modifier || levenshtein_distance(last_part, modifier) <= 1 {
                return (letter_part, Some(last_part.to_string()));
            }
        }
    }

    (normalized, None)
}

/// Core validation logic
fn validate_answer_internal(user_input: &str, correct_answer: &str) -> AnswerResult {
    let normalized_input = normalize_answer(user_input);

    if normalized_input.is_empty() {
        return AnswerResult::Incorrect;
    }

    let variants = extract_variants(correct_answer);

    if variants.contains(&normalized_input) {
        return AnswerResult::Correct;
    }

    let (correct_letter, correct_modifier) = parse_phonetic_answer(correct_answer);

    if correct_modifier.is_some() {
        let (input_letter, input_modifier) = parse_phonetic_answer(user_input);

        if input_letter != correct_letter {
            return AnswerResult::Incorrect;
        }

        if let (Some(correct_mod), Some(input_mod)) = (&correct_modifier, &input_modifier) {
            let mod_distance = levenshtein_distance(input_mod, correct_mod);
            if mod_distance == 0 { return AnswerResult::Correct; }
            if mod_distance == 1 { return AnswerResult::CloseEnough; }
            return AnswerResult::Incorrect;
        }

        if input_modifier.is_none() && correct_modifier.is_some() {
            return AnswerResult::Incorrect;
        }
    }

    for variant in &variants {
        let distance = levenshtein_distance(&normalized_input, variant);
        let char_count = variant.chars().count();
        let max_distance = match char_count {
            0..=1 => 0,
            2..=4 => 1,
            _ => 1,
        };
        if distance > 0 && distance <= max_distance {
            return AnswerResult::CloseEnough;
        }
    }

    AnswerResult::Incorrect
}

// ============================================================================
// FSRS Scheduling (using rs-fsrs)
// ============================================================================

/// Learning steps in minutes (normal mode)
const LEARNING_STEPS_NORMAL: [i64; 4] = [1, 10, 60, 240];
/// Learning steps in minutes (focus mode - faster graduation)
const LEARNING_STEPS_FOCUS: [i64; 4] = [1, 5, 15, 30];
/// Step at which card graduates to FSRS long-term scheduling
const GRADUATING_STEP: i64 = 4;

/// Input card state for scheduling
#[derive(Debug, Serialize, Deserialize)]
pub struct CardState {
    pub learning_step: i64,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub repetitions: i64,
    /// ISO8601 timestamp of next review
    pub next_review: String,
}

/// Result of scheduling calculation
#[derive(Debug, Serialize, Deserialize)]
pub struct SchedulingResult {
    /// ISO8601 timestamp
    pub next_review: String,
    pub stability: f64,
    pub difficulty: f64,
    pub learning_step: i64,
    pub repetitions: i64,
    /// "New", "Learning", "Review", "Relearning"
    pub state: String,
}

/// Calculate next review using hybrid learning steps + FSRS
fn calculate_scheduling(
    card_state: &CardState,
    quality: u8,
    desired_retention: f64,
    focus_mode: bool,
) -> SchedulingResult {
    let now = Utc::now();
    let is_correct = quality >= 2;
    let learning_steps = if focus_mode { &LEARNING_STEPS_FOCUS } else { &LEARNING_STEPS_NORMAL };

    // In learning phase (step 0-3): use learning steps
    if card_state.learning_step < GRADUATING_STEP {
        return calculate_learning_phase(card_state, is_correct, now, learning_steps);
    }

    // Graduated but failed: return to learning phase
    if !is_correct {
        return return_to_learning(card_state, now, learning_steps);
    }

    // Correct answer on graduated card: use FSRS
    calculate_fsrs_graduated(card_state, quality, desired_retention, now)
}

fn calculate_learning_phase(
    card_state: &CardState,
    is_correct: bool,
    now: DateTime<Utc>,
    learning_steps: &[i64; 4],
) -> SchedulingResult {
    if !is_correct {
        let next_review = now + chrono::Duration::minutes(learning_steps[0]);
        return SchedulingResult {
            next_review: next_review.to_rfc3339(),
            stability: card_state.stability.unwrap_or(0.0),
            difficulty: card_state.difficulty.unwrap_or(5.0),
            learning_step: 0,
            repetitions: 0,
            state: "Learning".to_string(),
        };
    }

    let next_step = card_state.learning_step + 1;

    if next_step >= GRADUATING_STEP {
        // Graduating! Initialize FSRS state
        let fsrs = FSRS::default();
        let card = Card::new();
        let now_utc = Utc::now();
        let record_log = fsrs.repeat(card, now_utc);

        // Use "Good" rating for graduation
        let scheduled = &record_log[&Rating::Good];

        SchedulingResult {
            next_review: (now + chrono::Duration::days(1)).to_rfc3339(),
            stability: scheduled.card.stability as f64,
            difficulty: scheduled.card.difficulty as f64,
            learning_step: GRADUATING_STEP,
            repetitions: 1,
            state: "Review".to_string(),
        }
    } else {
        let minutes = learning_steps[next_step as usize];
        let next_review = now + chrono::Duration::minutes(minutes);

        SchedulingResult {
            next_review: next_review.to_rfc3339(),
            stability: card_state.stability.unwrap_or(0.0),
            difficulty: card_state.difficulty.unwrap_or(5.0),
            learning_step: next_step,
            repetitions: 0,
            state: "Learning".to_string(),
        }
    }
}

fn return_to_learning(
    card_state: &CardState,
    now: DateTime<Utc>,
    learning_steps: &[i64; 4],
) -> SchedulingResult {
    let next_review = now + chrono::Duration::minutes(learning_steps[0]);
    SchedulingResult {
        next_review: next_review.to_rfc3339(),
        stability: card_state.stability.unwrap_or(0.0),
        difficulty: card_state.difficulty.unwrap_or(5.0),
        learning_step: 0,
        repetitions: 0,
        state: "Relearning".to_string(),
    }
}

fn calculate_fsrs_graduated(
    card_state: &CardState,
    quality: u8,
    _desired_retention: f64,
    now: DateTime<Utc>,
) -> SchedulingResult {
    // Create FSRS instance with default parameters
    // TODO: Support custom desired_retention via Parameters
    let fsrs = FSRS::default();

    // Create card with current state
    let mut card = Card::new();
    card.stability = card_state.stability.unwrap_or(1.0);
    card.difficulty = card_state.difficulty.unwrap_or(5.0);

    // Parse last review time
    if let Ok(last_review) = DateTime::parse_from_rfc3339(&card_state.next_review) {
        card.last_review = last_review.with_timezone(&Utc);
    }

    // Get scheduling for the rating
    let record_log = fsrs.repeat(card, now);

    // Map quality to rs-fsrs Rating
    let rating = match quality {
        0 => Rating::Again,
        2 => Rating::Hard,
        4 => Rating::Good,
        5 => Rating::Easy,
        _ => Rating::Good,
    };

    let scheduled = &record_log[&rating];
    let next_review = scheduled.card.due;

    SchedulingResult {
        next_review: next_review.to_rfc3339(),
        stability: scheduled.card.stability as f64,
        difficulty: scheduled.card.difficulty as f64,
        learning_step: card_state.learning_step,
        repetitions: card_state.repetitions + 1,
        state: "Review".to_string(),
    }
}

// ============================================================================
// WASM Exports
// ============================================================================

/// Initialize panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    set_panic_hook();
}

/// Validate a user's answer against the correct answer.
///
/// Returns JSON: {"result": "Correct"|"CloseEnough"|"Incorrect", "quality": 0-4}
#[wasm_bindgen]
pub fn validate_answer(user_input: &str, correct_answer: &str, used_hint: bool) -> String {
    let result = validate_answer_internal(user_input, correct_answer);
    let quality = result.to_quality(used_hint);

    let response = serde_json::json!({
        "result": match result {
            AnswerResult::Correct => "Correct",
            AnswerResult::CloseEnough => "CloseEnough",
            AnswerResult::Incorrect => "Incorrect",
        },
        "quality": quality,
        "is_correct": matches!(result, AnswerResult::Correct | AnswerResult::CloseEnough),
    });

    response.to_string()
}

/// Calculate the next review schedule for a card.
///
/// card_state_json: JSON with {learning_step, stability, difficulty, repetitions, next_review}
/// quality: 0=Again, 2=Hard, 4=Good, 5=Easy
/// desired_retention: Target retention rate (e.g., 0.9 for 90%)
/// focus_mode: If true, uses faster learning steps
///
/// Returns JSON with new scheduling state.
#[wasm_bindgen]
pub fn calculate_next_review(
    card_state_json: &str,
    quality: u8,
    desired_retention: f64,
    focus_mode: bool,
) -> String {
    let card_state: CardState = match serde_json::from_str(card_state_json) {
        Ok(state) => state,
        Err(e) => {
            return serde_json::json!({
                "error": format!("Failed to parse card state: {}", e)
            }).to_string();
        }
    };

    let result = calculate_scheduling(&card_state, quality, desired_retention, focus_mode);

    match serde_json::to_string(&result) {
        Ok(json) => json,
        Err(e) => serde_json::json!({
            "error": format!("Failed to serialize result: {}", e)
        }).to_string(),
    }
}

/// Get hint for an answer (progressive reveal)
#[wasm_bindgen]
pub fn get_hint(answer: &str, level: u8) -> String {
    let chars: Vec<char> = answer.chars().collect();
    let char_count = chars.len();

    match level {
        1 => {
            // First letter + length
            let first_char = chars.first().copied().unwrap_or('?');
            let underscores = "_".repeat(char_count.saturating_sub(1));
            format!("{}{} ({} letters)", first_char, underscores, char_count)
        }
        2 => {
            // First two characters
            if chars.len() <= 2 {
                answer.to_string()
            } else {
                let first_two: String = chars[..2].iter().collect();
                let underscores = "_".repeat(chars.len() - 2);
                format!("{}{}", first_two, underscores)
            }
        }
        _ => answer.to_string(), // Full answer
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_exact_match() {
        let result = validate_answer_internal("g", "g / k");
        assert_eq!(result, AnswerResult::Correct);
    }

    #[test]
    fn test_validate_alternative() {
        let result = validate_answer_internal("k", "g / k");
        assert_eq!(result, AnswerResult::Correct);
    }

    #[test]
    fn test_validate_close_enough() {
        let result = validate_answer_internal("yaa", "ya");
        assert_eq!(result, AnswerResult::CloseEnough);
    }

    #[test]
    fn test_validate_incorrect() {
        let result = validate_answer_internal("m", "g / k");
        assert_eq!(result, AnswerResult::Incorrect);
    }

    #[test]
    fn test_quality_mapping() {
        assert_eq!(AnswerResult::Correct.to_quality(false), 4);
        assert_eq!(AnswerResult::Correct.to_quality(true), 2);
        assert_eq!(AnswerResult::Incorrect.to_quality(false), 0);
    }

    #[test]
    fn test_scheduling_learning_phase() {
        let card_state = CardState {
            learning_step: 0,
            stability: None,
            difficulty: None,
            repetitions: 0,
            next_review: Utc::now().to_rfc3339(),
        };

        let result = calculate_scheduling(&card_state, 4, 0.9, false);
        assert_eq!(result.learning_step, 1);
        assert_eq!(result.state, "Learning");
    }

    #[test]
    fn test_wasm_validate_answer() {
        let json = validate_answer("g", "g / k", false);
        assert!(json.contains("\"result\":\"Correct\""));
        assert!(json.contains("\"quality\":4"));
    }
}
