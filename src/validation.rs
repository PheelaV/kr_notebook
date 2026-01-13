//! Answer validation module with flexible matching for Hangul learning.
//!
//! Handles romanization variations like "g / k" matching "g", "k", "g/k", etc.

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
      .split('/')
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

/// Parse a phonetic answer into core letter and optional modifier
/// e.g., "jj (tense)" -> ("jj", Some("tense"))
/// e.g., "g / k" -> ("g / k", None)
/// e.g., "ya" -> ("ya", None)
fn parse_phonetic_answer(answer: &str) -> (String, Option<String>) {
  let normalized = normalize_answer(answer);

  // Split by whitespace and check if last part looks like a modifier
  let parts: Vec<&str> = normalized.split_whitespace().collect();
  if parts.len() >= 2 {
    let last_part = parts[parts.len() - 1];
    let letter_parts = &parts[..parts.len() - 1];
    let letter_part = letter_parts.join(" ");

    // Check if last part is or resembles a known modifier
    let modifiers = ["tense", "aspirated"];
    for modifier in &modifiers {
      // Exact match or close match (1 edit distance for typos like "tenss")
      if last_part == *modifier || levenshtein_distance(last_part, modifier) <= 1 {
        return (letter_part, Some(last_part.to_string()));
      }
    }
  }

  (normalized, None)
}

/// Validate a user's answer against the correct answer
pub fn validate_answer(user_input: &str, correct_answer: &str) -> AnswerResult {
  let normalized_input = normalize_answer(user_input);

  if normalized_input.is_empty() {
    return AnswerResult::Incorrect;
  }

  let variants = extract_variants(correct_answer);

  // Check for exact match with any variant
  if variants.contains(&normalized_input) {
    return AnswerResult::Correct;
  }

  // Parse correct answer to check for phonetic pattern (letter + modifier)
  let (correct_letter, correct_modifier) = parse_phonetic_answer(correct_answer);

  // If the correct answer has a modifier (tense/aspirated), validate letter separately
  if correct_modifier.is_some() {
    let (input_letter, input_modifier) = parse_phonetic_answer(user_input);

    // Core letter part MUST be exact - no tolerance for wrong consonants
    if input_letter != correct_letter {
      return AnswerResult::Incorrect;
    }

    // If letter is correct, check modifier
    if let (Some(correct_mod), Some(input_mod)) = (&correct_modifier, &input_modifier) {
      // Compare input modifier against the canonical correct modifier
      let mod_distance = levenshtein_distance(input_mod, correct_mod);
      if mod_distance == 0 {
        return AnswerResult::Correct;
      }
      if mod_distance == 1 {
        return AnswerResult::CloseEnough;
      }
      // Wrong modifier (e.g., "tense" vs "aspirated")
      return AnswerResult::Incorrect;
    }

    // Letter correct but modifier missing or wrong format
    if input_modifier.is_none() && correct_modifier.is_some() {
      // User typed just the letter without modifier - that's incomplete
      return AnswerResult::Incorrect;
    }
  }

  // Standard validation for non-phonetic answers (simple letters, vowels, etc.)
  for variant in &variants {
    let distance = levenshtein_distance(&normalized_input, variant);
    let char_count = variant.chars().count();
    // For simple answers, allow 1 char tolerance for 2+ char answers
    let max_distance = match char_count {
      0..=1 => 0, // Single char must be exact
      2..=4 => 1, // Short answers: 1 char tolerance
      _ => 1,     // Longer answers: 1 char tolerance
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
    let chars: Vec<char> = self.answer.chars().collect();
    let first_char = chars.first().copied().unwrap_or('?');
    let char_count = chars.len();
    let underscores = "_".repeat(char_count.saturating_sub(1));
    format!("{}{} ({} letters)", first_char, underscores, char_count)
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
    // Different consonants should be incorrect even with same modifier
    assert_eq!(validate_answer("ch tense", "jj (tense)"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("dd tense", "jj (tense)"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("gg tense", "jj (tense)"), AnswerResult::Incorrect);
    // Missing modifier should be incorrect
    assert_eq!(validate_answer("jj", "jj (tense)"), AnswerResult::Incorrect);
    // Wrong modifier should be incorrect
    assert_eq!(validate_answer("jj aspirated", "jj (tense)"), AnswerResult::Incorrect);
  }

  #[test]
  fn test_phonetic_modifiers() {
    // Exact match with modifier
    assert_eq!(validate_answer("jj tense", "jj (tense)"), AnswerResult::Correct);
    assert_eq!(validate_answer("jj (tense)", "jj (tense)"), AnswerResult::Correct);
    assert_eq!(validate_answer("ch aspirated", "ch (aspirated)"), AnswerResult::Correct);

    // Typo in modifier is close enough (1 edit distance)
    assert_eq!(validate_answer("jj tenss", "jj (tense)"), AnswerResult::CloseEnough);  // extra s
    assert_eq!(validate_answer("jj tensee", "jj (tense)"), AnswerResult::CloseEnough); // extra e
    // 2+ edits in modifier is incorrect
    assert_eq!(validate_answer("jj tenes", "jj (tense)"), AnswerResult::Incorrect);    // transposition = 2 edits

    // Wrong letter is always incorrect
    assert_eq!(validate_answer("ch tense", "jj (tense)"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("gg aspirated", "ch (aspirated)"), AnswerResult::Incorrect);
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
    assert!(hint_gen.hint_level_1().contains("5")); // 5 characters
    assert_eq!(hint_gen.hint_level_2(), "Like 'g' in 'go'");
    assert_eq!(hint_gen.hint_final(), "g / k");
  }

  #[test]
  fn test_hint_generation_korean() {
    // Korean characters should count correctly (not byte length)
    let hint_gen = HintGenerator::new("안녕", None);
    assert_eq!(hint_gen.hint_level_1(), "안_ (2 letters)");
    assert_eq!(hint_gen.hint_level_2(), "안녕"); // 2 chars, shows full answer
    assert_eq!(hint_gen.hint_final(), "안녕");

    let hint_gen = HintGenerator::new("안녕하세요", Some("Hello (formal)"));
    assert_eq!(hint_gen.hint_level_1(), "안____ (5 letters)");
    assert_eq!(hint_gen.hint_level_2(), "Hello (formal)");
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

  #[test]
  fn test_korean_characters() {
    // Single Korean jamo (consonants/vowels)
    assert_eq!(validate_answer("ㄱ", "ㄱ"), AnswerResult::Correct);
    assert_eq!(validate_answer("ㄴ", "ㄱ"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("ㅏ", "ㅏ"), AnswerResult::Correct);
    assert_eq!(validate_answer("ㅓ", "ㅏ"), AnswerResult::Incorrect);

    // Korean syllables
    assert_eq!(validate_answer("가", "가"), AnswerResult::Correct);
    assert_eq!(validate_answer("나", "가"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("바", "가"), AnswerResult::Incorrect);

    // Different but similar looking
    assert_eq!(validate_answer("ㄲ", "ㄱ"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("ㅃ", "ㅂ"), AnswerResult::Incorrect);
  }

  #[test]
  fn test_korean_normalize() {
    // Test that Korean characters survive normalization
    let normalized = normalize_answer("가");
    assert_eq!(normalized, "가");

    let normalized = normalize_answer("ㄱ");
    assert_eq!(normalized, "ㄱ");

    // With whitespace
    let normalized = normalize_answer("  가  ");
    assert_eq!(normalized, "가");
  }
}
