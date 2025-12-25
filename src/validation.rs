/// Answer validation module with flexible matching for Hangul learning
///
/// Handles romanization variations like "g / k" matching "g", "k", "g/k", etc.

use serde::{Deserialize, Serialize};

/// Result of answer validation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnswerResult {
  /// Exact or acceptable match
  Correct,
  /// Close enough (minor typo, acceptable variation)
  CloseEnough,
  /// Incorrect answer
  Incorrect,
}

impl AnswerResult {
  pub fn is_correct(&self) -> bool {
    matches!(self, Self::Correct | Self::CloseEnough)
  }

  /// Convert to quality rating for SRS
  /// Correct first try = Good (4), CloseEnough = Hard (2), Incorrect = Again (0)
  pub fn to_quality(&self, used_hint: bool) -> u8 {
    match (self, used_hint) {
      (Self::Correct, false) => 4,      // Good
      (Self::Correct, true) => 2,       // Hard (needed hint)
      (Self::CloseEnough, _) => 2,      // Hard (close but not exact)
      (Self::Incorrect, _) => 0,        // Again
    }
  }
}

/// Normalize an answer for comparison
/// - Converts to lowercase
/// - Trims whitespace
/// - Normalizes separators (/ becomes space-separated alternatives)
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

/// Extract all acceptable answer variants from a main answer
/// e.g., "g / k" -> ["g", "k", "g / k", "g/k"]
fn extract_variants(main_answer: &str) -> Vec<String> {
  let mut variants = Vec::new();

  // Add the original normalized answer
  let normalized = normalize_answer(main_answer);
  variants.push(normalized.clone());

  // If answer contains " / ", split into alternatives
  if main_answer.contains(" / ") || main_answer.contains("/") {
    let parts: Vec<&str> = main_answer
      .split(|c| c == '/')
      .map(|s| s.trim())
      .filter(|s| !s.is_empty())
      .collect();

    // Add each part as a valid alternative
    for part in &parts {
      let normalized_part = normalize_answer(part);
      if !normalized_part.is_empty() && !variants.contains(&normalized_part) {
        variants.push(normalized_part);
      }
    }

    // Also add the joined version without spaces around slash
    let joined = parts.join("/");
    let normalized_joined = normalize_answer(&joined);
    if !variants.contains(&normalized_joined) {
      variants.push(normalized_joined);
    }
  }

  variants
}

/// Calculate simple Levenshtein distance between two strings
fn levenshtein_distance(a: &str, b: &str) -> usize {
  let a_chars: Vec<char> = a.chars().collect();
  let b_chars: Vec<char> = b.chars().collect();
  let a_len = a_chars.len();
  let b_len = b_chars.len();

  if a_len == 0 {
    return b_len;
  }
  if b_len == 0 {
    return a_len;
  }

  let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

  for i in 0..=a_len {
    matrix[i][0] = i;
  }
  for j in 0..=b_len {
    matrix[0][j] = j;
  }

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

/// Validate a user's answer against the correct answer
pub fn validate_answer(user_input: &str, correct_answer: &str) -> AnswerResult {
  let normalized_input = normalize_answer(user_input);

  if normalized_input.is_empty() {
    return AnswerResult::Incorrect;
  }

  let variants = extract_variants(correct_answer);

  // Check for exact match with any variant
  if variants.iter().any(|v| *v == normalized_input) {
    return AnswerResult::Correct;
  }

  // Check for close match (Levenshtein distance based on length) with any variant
  for variant in &variants {
    let distance = levenshtein_distance(&normalized_input, variant);
    // For single-char answers, must be exact; 2-3 char allows 1 diff; 4+ allows 2
    let max_distance = match variant.len() {
      0..=1 => 0, // Single char must be exact
      2..=3 => 1, // Short answers: 1 char tolerance
      _ => 2,     // Longer answers: 2 char tolerance
    };
    if distance > 0 && distance <= max_distance {
      return AnswerResult::CloseEnough;
    }
  }

  AnswerResult::Incorrect
}

/// Generate progressive hints for an answer
pub struct HintGenerator {
  answer: String,
  description: Option<String>,
}

impl HintGenerator {
  pub fn new(answer: &str, description: Option<&str>) -> Self {
    Self {
      answer: answer.to_string(),
      description: description.map(|s| s.to_string()),
    }
  }

  /// Get hint level 1: First letter and length
  pub fn hint_level_1(&self) -> String {
    let first_char = self.answer.chars().next().unwrap_or('?');
    let len = self.answer.len();
    let underscores = "_".repeat(len.saturating_sub(1));
    format!("{}{} ({} letters)", first_char, underscores, len)
  }

  /// Get hint level 2: Description if available, otherwise more letters
  pub fn hint_level_2(&self) -> String {
    if let Some(desc) = &self.description {
      desc.clone()
    } else {
      // Show first two characters
      let chars: Vec<char> = self.answer.chars().collect();
      if chars.len() <= 2 {
        self.answer.clone()
      } else {
        let first_two: String = chars[..2].iter().collect();
        let underscores = "_".repeat(chars.len() - 2);
        format!("{}{}", first_two, underscores)
      }
    }
  }

  /// Get final hint: The full answer
  pub fn hint_final(&self) -> String {
    self.answer.clone()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_exact_match() {
    assert_eq!(validate_answer("g", "g / k"), AnswerResult::Correct);
    assert_eq!(validate_answer("k", "g / k"), AnswerResult::Correct);
    assert_eq!(validate_answer("g / k", "g / k"), AnswerResult::Correct);
    assert_eq!(validate_answer("g/k", "g / k"), AnswerResult::Correct);
  }

  #[test]
  fn test_case_insensitive() {
    assert_eq!(validate_answer("G", "g / k"), AnswerResult::Correct);
    assert_eq!(validate_answer("K", "g / k"), AnswerResult::Correct);
    assert_eq!(validate_answer("YA", "ya"), AnswerResult::Correct);
  }

  #[test]
  fn test_close_match() {
    // One character typo
    assert_eq!(validate_answer("yaa", "ya"), AnswerResult::CloseEnough);
    assert_eq!(validate_answer("yo", "ya"), AnswerResult::CloseEnough); // 1 char diff is close enough
  }

  #[test]
  fn test_incorrect() {
    assert_eq!(validate_answer("m", "g / k"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("xyz", "ya"), AnswerResult::Incorrect);  // 3 char diff = incorrect
    assert_eq!(validate_answer("abc", "ya"), AnswerResult::Incorrect);  // completely wrong
    assert_eq!(validate_answer("", "g / k"), AnswerResult::Incorrect);
  }

  #[test]
  fn test_quality_mapping() {
    assert_eq!(AnswerResult::Correct.to_quality(false), 4);
    assert_eq!(AnswerResult::Correct.to_quality(true), 2);
    assert_eq!(AnswerResult::CloseEnough.to_quality(false), 2);
    assert_eq!(AnswerResult::Incorrect.to_quality(false), 0);
  }

  #[test]
  fn test_hint_generation() {
    let hint_gen = HintGenerator::new("g / k", Some("Like 'g' in 'go'"));
    assert!(hint_gen.hint_level_1().contains("g"));
    assert!(hint_gen.hint_level_1().contains("5")); // length
    assert_eq!(hint_gen.hint_level_2(), "Like 'g' in 'go'");
    assert_eq!(hint_gen.hint_final(), "g / k");
  }

  #[test]
  fn test_simple_answers() {
    assert_eq!(validate_answer("eo", "eo"), AnswerResult::Correct);
    assert_eq!(validate_answer("ya", "ya"), AnswerResult::Correct);
    assert_eq!(validate_answer("ng", "ng"), AnswerResult::Correct);
  }

  #[test]
  fn test_levenshtein() {
    assert_eq!(levenshtein_distance("cat", "cat"), 0);
    assert_eq!(levenshtein_distance("cat", "bat"), 1);
    assert_eq!(levenshtein_distance("cat", "cars"), 2);
    assert_eq!(levenshtein_distance("", "abc"), 3);
  }
}
