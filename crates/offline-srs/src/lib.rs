//! WASM module for offline study mode.
//!
//! Provides answer validation and FSRS scheduling that can run in the browser.

use std::collections::HashSet;
use wasm_bindgen::prelude::*;
use chrono::{DateTime, Utc};
use rs_fsrs::{FSRS, Card, Rating};
use serde::{Deserialize, Serialize};

#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

// ============================================================================
// Answer Validation (ported from src/validation.rs)
// ============================================================================

/// British/American spelling equivalences (normalized to American)
static SPELLING_EQUIVALENCES: &[(&str, &str)] = &[
    ("colour", "color"),
    ("behaviour", "behavior"),
    ("favour", "favor"),
    ("honour", "honor"),
    ("centre", "center"),
    ("fibre", "fiber"),
    ("grey", "gray"),
    ("defence", "defense"),
    ("licence", "license"),
    ("favourite", "favorite"),
    ("organise", "organize"),
    ("realise", "realize"),
    ("mum", "mom"),
];

/// Contraction expansions (normalized to expanded form)
static CONTRACTIONS: &[(&str, &str)] = &[
    ("i'm", "i am"),
    ("you're", "you are"),
    ("he's", "he is"),
    ("she's", "she is"),
    ("it's", "it is"),
    ("we're", "we are"),
    ("they're", "they are"),
    ("isn't", "is not"),
    ("aren't", "are not"),
    ("don't", "do not"),
    ("doesn't", "does not"),
    ("didn't", "did not"),
    ("can't", "cannot"),
    ("won't", "will not"),
    ("wouldn't", "would not"),
    ("couldn't", "could not"),
    ("shouldn't", "should not"),
];

/// Result of answer validation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnswerResult {
    /// Full match (core + disambiguation if present)
    Correct,
    /// Core matched but disambiguation missing - knowledge gap
    PartialMatch,
    /// Close enough (typo tolerance) - execution error
    CloseEnough,
    /// Wrong answer
    Incorrect,
}

impl AnswerResult {
    pub fn is_correct(&self) -> bool {
        !matches!(self, Self::Incorrect)
    }

    /// Convert to quality rating for SRS
    pub fn to_quality(&self, used_hint: bool) -> u8 {
        match (self, used_hint) {
            (Self::Correct, false) => 4,
            (Self::Correct, true) => 3,
            (Self::CloseEnough, false) => 4, // Typo - no penalty
            (Self::CloseEnough, true) => 3,
            (Self::PartialMatch, _) => 2,    // Knowledge gap
            (Self::Incorrect, _) => 0,
        }
    }

    /// Whether to trigger shake/retry opportunity
    pub fn allows_retry(&self) -> bool {
        matches!(self, Self::PartialMatch)
    }
}

/// Parsed answer structure with semantic types
#[derive(Debug, Clone, Default)]
pub struct ParsedAnswer {
    pub core: String,
    pub variants: Vec<String>,
    pub suffix: Option<String>,
    pub disambiguation: Option<String>,
    pub info: Option<String>,
    pub is_phonetic_modifier: bool,
}

/// Parse answer using bracket grammar
fn parse_answer_grammar(main_answer: &str) -> ParsedAnswer {
    let mut result = ParsedAnswer::default();
    let input = main_answer.trim();

    // Check for phonetic modifiers
    if is_phonetic_modifier_answer(input) {
        result.is_phonetic_modifier = true;
        result.core = input.to_string();
        return result;
    }

    let mut core_parts: Vec<String> = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = input.chars().collect();

    while i < chars.len() {
        match chars[i] {
            '[' => {
                if let Some(end) = find_closing_bracket(&chars, i, '[', ']') {
                    let content: String = chars[i + 1..end].iter().collect();
                    for item in content.split(',') {
                        let trimmed = item.trim();
                        if !trimmed.is_empty() {
                            result.variants.push(trimmed.to_string());
                        }
                    }
                    i = end + 1;
                } else {
                    core_parts.push(chars[i].to_string());
                    i += 1;
                }
            }
            '<' => {
                if let Some(end) = find_closing_bracket(&chars, i, '<', '>') {
                    let content: String = chars[i + 1..end].iter().collect();
                    result.disambiguation = Some(content.trim().to_string());
                    i = end + 1;
                } else {
                    core_parts.push(chars[i].to_string());
                    i += 1;
                }
            }
            '(' => {
                if let Some(end) = find_closing_bracket(&chars, i, '(', ')') {
                    let has_space_before = i > 0 && chars[i - 1] == ' ';
                    let content: String = chars[i + 1..end].iter().collect();
                    if !has_space_before && i > 0 {
                        result.suffix = Some(content.trim().to_string());
                    } else {
                        result.info = Some(content.trim().to_string());
                    }
                    i = end + 1;
                } else {
                    core_parts.push(chars[i].to_string());
                    i += 1;
                }
            }
            _ => {
                core_parts.push(chars[i].to_string());
                i += 1;
            }
        }
    }

    result.core = core_parts.join("").trim().to_string();
    result
}

fn find_closing_bracket(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (i, &ch) in chars.iter().enumerate().skip(start) {
        if ch == open { depth += 1; }
        else if ch == close {
            depth -= 1;
            if depth == 0 { return Some(i); }
        }
    }
    None
}

fn is_phonetic_modifier_answer(answer: &str) -> bool {
    let normalized = answer.to_lowercase();
    normalized.contains("(tense)") || normalized.contains("(aspirated)")
}

/// Expand contractions to full form
fn expand_contractions(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            for (contraction, expanded) in CONTRACTIONS {
                if word == *contraction { return expanded.to_string(); }
            }
            word.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Normalize British spellings to American
fn normalize_spellings(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            for (british, american) in SPELLING_EQUIVALENCES {
                if word == *british { return american.to_string(); }
            }
            word.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Normalize an answer for comparison
fn normalize_answer(input: &str) -> String {
    let mut result = input
        .to_lowercase()
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '/' || *c == '\'')
        .collect::<String>();

    result = expand_contractions(&result);
    result = normalize_spellings(&result);

    result
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '/')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Generate all valid answer forms from parsed structure
fn generate_valid_answers(parsed: &ParsedAnswer) -> (Vec<String>, Vec<String>) {
    let mut partial_answers: Vec<String> = Vec::new();

    let normalized_core = normalize_answer(&parsed.core);
    if !normalized_core.is_empty() {
        partial_answers.push(normalized_core.clone());
    }

    for variant in &parsed.variants {
        let normalized = normalize_answer(variant);
        if !normalized.is_empty() && !partial_answers.contains(&normalized) {
            partial_answers.push(normalized);
        }
    }

    if let Some(ref suffix) = parsed.suffix {
        let with_suffix = format!("{}{}", parsed.core, suffix);
        let normalized = normalize_answer(&with_suffix);
        if !normalized.is_empty() && !partial_answers.contains(&normalized) {
            partial_answers.push(normalized);
        }
    }

    if parsed.core.contains(',') {
        let parts: Vec<&str> = parsed.core.split(',').map(|s| s.trim()).collect();
        for part in &parts {
            let normalized = normalize_answer(part);
            if !normalized.is_empty() && !partial_answers.contains(&normalized) {
                partial_answers.push(normalized);
            }
        }
    }

    if parsed.core.contains('/') {
        let parts: Vec<&str> = parsed.core.split('/').map(|s| s.trim()).collect();
        for part in &parts {
            let normalized = normalize_answer(part);
            if !normalized.is_empty() && !partial_answers.contains(&normalized) {
                partial_answers.push(normalized);
            }
        }
        let joined = parts.join("/");
        let normalized = normalize_answer(&joined);
        if !normalized.is_empty() && !partial_answers.contains(&normalized) {
            partial_answers.push(normalized);
        }
    }

    let full_answers = if let Some(ref disambig) = parsed.disambiguation {
        let mut full = Vec::new();
        let normalized_disambig = normalize_answer(disambig);
        for partial in &partial_answers {
            let forward = format!("{} {}", partial, normalized_disambig);
            let backward = format!("{} {}", normalized_disambig, partial);
            if !full.contains(&forward) { full.push(forward); }
            if !full.contains(&backward) { full.push(backward); }
        }
        full
    } else {
        partial_answers.clone()
    };

    (full_answers, partial_answers)
}

/// Check if user input matches any permutation/subset of comma-separated terms
fn matches_permutation(user_input: &str, expected: &str) -> bool {
    let input_words: HashSet<String> = user_input.split_whitespace().map(|s| s.to_string()).collect();
    if input_words.is_empty() { return false; }

    let expected_words: HashSet<String> = expected
        .split(|c| c == ',' || c == ' ')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    input_words.iter().all(|word| expected_words.contains(word))
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
    // Parse user input to strip any grammar syntax they might have typed
    // (e.g., if they type "I, me <formal>" exactly as displayed)
    let input_parsed = parse_answer_grammar(user_input);
    let normalized_input = normalize_answer(&input_parsed.core);

    if normalized_input.is_empty() {
        return AnswerResult::Incorrect;
    }

    let parsed = parse_answer_grammar(correct_answer);

    // Special handling for phonetic modifiers
    if parsed.is_phonetic_modifier {
        return validate_phonetic_answer(&normalized_input, correct_answer);
    }

    // Check if user provided disambiguation matches expected
    // Accept both <formal> syntax and (formal) syntax from user input
    let user_has_correct_disambig = if let Some(expected_disambig) = &parsed.disambiguation {
        let expected_norm = normalize_answer(expected_disambig);
        // Check user's <disambiguation> syntax
        let from_disambig = input_parsed
            .disambiguation
            .as_ref()
            .map(|d| normalize_answer(d) == expected_norm)
            .unwrap_or(false);
        // Also check user's (info) syntax - user may type (formal) instead of <formal>
        let from_info = input_parsed
            .info
            .as_ref()
            .map(|i| normalize_answer(i) == expected_norm)
            .unwrap_or(false);
        from_disambig || from_info
    } else {
        false
    };

    let (full_answers, partial_answers) = generate_valid_answers(&parsed);

    // Exact match with full answers
    if full_answers.contains(&normalized_input) {
        return AnswerResult::Correct;
    }

    // Permutation matching for comma-separated synonyms
    if parsed.core.contains(',') {
        if matches_permutation(&normalized_input, &parsed.core) {
            // If user also provided correct disambiguation, it's full correct
            if user_has_correct_disambig {
                return AnswerResult::Correct;
            }
            return if parsed.disambiguation.is_some() {
                AnswerResult::PartialMatch
            } else {
                AnswerResult::Correct
            };
        }
    }

    // Partial answer matches (core without disambiguation)
    if parsed.disambiguation.is_some() && partial_answers.contains(&normalized_input) {
        // If user also provided correct disambiguation, it's full correct
        if user_has_correct_disambig {
            return AnswerResult::Correct;
        }
        return AnswerResult::PartialMatch;
    }

    // Typo tolerance
    let all_answers: Vec<&String> = full_answers.iter().chain(partial_answers.iter()).collect();

    for answer in &all_answers {
        let distance = levenshtein_distance(&normalized_input, answer);
        let char_count = answer.chars().count();
        let max_distance = match char_count {
            0..=1 => 0,
            2..=4 => 1,
            _ => 2,
        };
        if distance > 0 && distance <= max_distance {
            if parsed.disambiguation.is_some() && partial_answers.contains(*answer) {
                return AnswerResult::PartialMatch;
            }
            return AnswerResult::CloseEnough;
        }
    }

    AnswerResult::Incorrect
}

/// Validate phonetic answers with modifiers (tense/aspirated)
fn validate_phonetic_answer(normalized_input: &str, correct_answer: &str) -> AnswerResult {
    let (correct_letter, correct_modifier) = parse_phonetic_answer(correct_answer);
    let (input_letter, input_modifier) = parse_phonetic_answer(normalized_input);

    if input_letter != correct_letter {
        return AnswerResult::Incorrect;
    }

    if let Some(ref correct_mod) = correct_modifier {
        if let Some(ref input_mod) = input_modifier {
            let mod_distance = levenshtein_distance(input_mod, correct_mod);
            if mod_distance == 0 { return AnswerResult::Correct; }
            if mod_distance == 1 { return AnswerResult::CloseEnough; }
            return AnswerResult::Incorrect;
        }
        return AnswerResult::Incorrect;
    }

    AnswerResult::Correct
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
/// Returns JSON: {"result": "Correct"|"PartialMatch"|"CloseEnough"|"Incorrect", "quality": 0-4}
#[wasm_bindgen]
pub fn validate_answer(user_input: &str, correct_answer: &str, used_hint: bool) -> String {
    let result = validate_answer_internal(user_input, correct_answer);
    let quality = result.to_quality(used_hint);

    let response = serde_json::json!({
        "result": match &result {
            AnswerResult::Correct => "Correct",
            AnswerResult::PartialMatch => "PartialMatch",
            AnswerResult::CloseEnough => "CloseEnough",
            AnswerResult::Incorrect => "Incorrect",
        },
        "quality": quality,
        "is_correct": result.is_correct(),
        "allows_retry": result.allows_retry(),
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
        // Correct answers
        assert_eq!(AnswerResult::Correct.to_quality(false), 4);
        assert_eq!(AnswerResult::Correct.to_quality(true), 3);
        // CloseEnough (typo) - no penalty
        assert_eq!(AnswerResult::CloseEnough.to_quality(false), 4);
        assert_eq!(AnswerResult::CloseEnough.to_quality(true), 3);
        // PartialMatch - knowledge gap
        assert_eq!(AnswerResult::PartialMatch.to_quality(false), 2);
        assert_eq!(AnswerResult::PartialMatch.to_quality(true), 2);
        // Incorrect
        assert_eq!(AnswerResult::Incorrect.to_quality(false), 0);
    }

    #[test]
    fn test_allows_retry() {
        assert!(!AnswerResult::Correct.allows_retry());
        assert!(!AnswerResult::CloseEnough.allows_retry());
        assert!(AnswerResult::PartialMatch.allows_retry());
        assert!(!AnswerResult::Incorrect.allows_retry());
    }

    #[test]
    fn test_bracket_variants() {
        assert_eq!(validate_answer_internal("to be", "to be [is, am, are]"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("is", "to be [is, am, are]"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("am", "to be [is, am, are]"), AnswerResult::Correct);
    }

    #[test]
    fn test_suffix_syntax() {
        assert_eq!(validate_answer_internal("eye", "eye(s)"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("eyes", "eye(s)"), AnswerResult::Correct);
    }

    #[test]
    fn test_disambiguation_partial_match() {
        assert_eq!(validate_answer_internal("that far", "that <far>"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("that", "that <far>"), AnswerResult::PartialMatch);
    }

    #[test]
    fn test_permutation_matching() {
        assert_eq!(validate_answer_internal("sofa", "sofa, couch"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("couch", "sofa, couch"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("sofa couch", "sofa, couch"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("couch sofa", "sofa, couch"), AnswerResult::Correct);
    }

    #[test]
    fn test_spelling_normalization() {
        assert_eq!(validate_answer_internal("color", "colour"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("favourite", "favorite"), AnswerResult::Correct);
    }

    #[test]
    fn test_contraction_normalization() {
        assert_eq!(validate_answer_internal("I am", "I'm"), AnswerResult::Correct);
        assert_eq!(validate_answer_internal("don't", "do not"), AnswerResult::Correct);
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
        assert!(json.contains("\"allows_retry\":false"));
    }

    #[test]
    fn test_wasm_validate_partial_match() {
        let json = validate_answer("that", "that <far>", false);
        assert!(json.contains("\"result\":\"PartialMatch\""));
        assert!(json.contains("\"quality\":2"));
        assert!(json.contains("\"allows_retry\":true"));
    }
}
