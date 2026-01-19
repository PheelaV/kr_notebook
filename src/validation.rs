//! Answer validation module with flexible matching for Hangul learning.
//!
//! Supports grammar syntax for answer definitions:
//! - `[a, b, c]` - Variants (any is correct)
//! - `word(s)` - Optional suffix (no space before paren)
//! - `<context>` - Disambiguation (partial credit for core only)
//! - `(info)` - Grammar tag (ignored in validation)
//! - `a, b` - Synonyms (any order accepted)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ============================================================================
// Normalization tables
// ============================================================================

/// British/American spelling equivalences (normalized to American)
static SPELLING_EQUIVALENCES: &[(&str, &str)] = &[
  ("colour", "color"),
  ("behaviour", "behavior"),
  ("favour", "favor"),
  ("honour", "honor"),
  ("humour", "humor"),
  ("labour", "labor"),
  ("neighbour", "neighbor"),
  ("rumour", "rumor"),
  ("savour", "savor"),
  ("vapour", "vapor"),
  ("centre", "center"),
  ("fibre", "fiber"),
  ("litre", "liter"),
  ("metre", "meter"),
  ("theatre", "theater"),
  ("grey", "gray"),
  ("analyse", "analyze"),
  ("catalyse", "catalyze"),
  ("paralyse", "paralyze"),
  ("defence", "defense"),
  ("offence", "offense"),
  ("licence", "license"),
  ("practise", "practice"),
  ("traveller", "traveler"),
  ("cancelled", "canceled"),
  ("jewellery", "jewelry"),
  ("mum", "mom"),
  ("favourite", "favorite"),
  ("organise", "organize"),
  ("realise", "realize"),
  ("recognise", "recognize"),
];

/// Contraction expansions (normalized to expanded form)
static CONTRACTIONS: &[(&str, &str)] = &[
  ("i'm", "i am"),
  ("i've", "i have"),
  ("i'll", "i will"),
  ("i'd", "i would"),
  ("you're", "you are"),
  ("you've", "you have"),
  ("you'll", "you will"),
  ("you'd", "you would"),
  ("he's", "he is"),
  ("he'll", "he will"),
  ("he'd", "he would"),
  ("she's", "she is"),
  ("she'll", "she will"),
  ("she'd", "she would"),
  ("it's", "it is"),
  ("it'll", "it will"),
  ("we're", "we are"),
  ("we've", "we have"),
  ("we'll", "we will"),
  ("we'd", "we would"),
  ("they're", "they are"),
  ("they've", "they have"),
  ("they'll", "they will"),
  ("they'd", "they would"),
  ("that's", "that is"),
  ("there's", "there is"),
  ("here's", "here is"),
  ("what's", "what is"),
  ("who's", "who is"),
  ("where's", "where is"),
  ("how's", "how is"),
  ("isn't", "is not"),
  ("aren't", "are not"),
  ("wasn't", "was not"),
  ("weren't", "were not"),
  ("haven't", "have not"),
  ("hasn't", "has not"),
  ("hadn't", "had not"),
  ("won't", "will not"),
  ("wouldn't", "would not"),
  ("don't", "do not"),
  ("doesn't", "does not"),
  ("didn't", "did not"),
  ("can't", "cannot"),
  ("couldn't", "could not"),
  ("shouldn't", "should not"),
  ("mightn't", "might not"),
  ("mustn't", "must not"),
  ("let's", "let us"),
];

// ============================================================================
// Result types
// ============================================================================

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
  /// Key distinction: typos (CloseEnough) penalize less than missing knowledge (PartialMatch)
  pub fn to_quality(&self, used_hint: bool) -> u8 {
    match (self, used_hint) {
      (Self::Correct, false) => 4,     // Good - full interval
      (Self::Correct, true) => 3,      // Good but needed hint
      (Self::CloseEnough, false) => 4, // Typo - no penalty (you knew it)
      (Self::CloseEnough, true) => 3,  // Typo with hint
      (Self::PartialMatch, _) => 2,    // Hard - missing disambiguation (knowledge gap)
      (Self::Incorrect, _) => 0,       // Again - reset
    }
  }

  /// Whether to trigger shake/retry opportunity
  pub fn allows_retry(&self) -> bool {
    matches!(self, Self::PartialMatch)
  }
}

// ============================================================================
// Grammar parsing
// ============================================================================

/// Parsed answer structure with semantic types
#[derive(Debug, Clone, Default)]
pub struct ParsedAnswer {
  /// Main answer text (without any brackets)
  pub core: String,
  /// [variant] items - acceptable alternatives
  pub variants: Vec<String>,
  /// word(s) suffix - optional ending
  pub suffix: Option<String>,
  /// <context> - disambiguation (tested with partial credit)
  pub disambiguation: Option<String>,
  /// (info) - grammar info (stripped but stored for input matching)
  pub info: Option<String>,
  /// Whether this is a phonetic modifier answer (tense/aspirated)
  pub is_phonetic_modifier: bool,
}

/// Parse answer using bracket grammar
/// - `[a, b, c]` → variants (any acceptable)
/// - `word(s)` → optional suffix (no space before paren)
/// - `<context>` → disambiguation (partial credit)
/// - `(info)` → grammar info (ignored for validation)
pub fn parse_answer_grammar(main_answer: &str) -> ParsedAnswer {
  let mut result = ParsedAnswer::default();
  let input = main_answer.trim();

  // Check for phonetic modifiers first (special case for Hangul consonants)
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
      // Variants: [a, b, c]
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
      // Disambiguation: <context>
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
      // Grammar info or suffix: (...)
      '(' => {
        if let Some(end) = find_closing_bracket(&chars, i, '(', ')') {
          // Check if this is a suffix (no space before paren)
          let has_space_before = i > 0 && chars[i - 1] == ' ';

          let content: String = chars[i + 1..end].iter().collect();
          if !has_space_before && i > 0 {
            // Suffix pattern: word(s)
            result.suffix = Some(content.trim().to_string());
          } else {
            // Grammar info: (noun), (formal) - store for input matching
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

/// Find the closing bracket, handling nesting
fn find_closing_bracket(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
  let mut depth = 0;
  for (i, &ch) in chars.iter().enumerate().skip(start) {
    if ch == open {
      depth += 1;
    } else if ch == close {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    }
  }
  None
}

/// Check if answer is a phonetic modifier pattern (Hangul consonant learning)
fn is_phonetic_modifier_answer(answer: &str) -> bool {
  let normalized = answer.to_lowercase();
  // Pattern: short consonant + (tense) or (aspirated)
  normalized.contains("(tense)") || normalized.contains("(aspirated)")
}

// ============================================================================
// Normalization functions
// ============================================================================

/// Normalize an answer for comparison
/// - Converts to lowercase
/// - Trims whitespace
/// - Expands contractions
/// - Normalizes British/American spellings
/// - Removes punctuation except /
fn normalize_answer(input: &str) -> String {
  let mut result = input
    .to_lowercase()
    .trim()
    .chars()
    .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '/' || *c == '\'')
    .collect::<String>();

  // Expand contractions
  result = expand_contractions(&result);

  // Normalize spellings (British to American)
  result = normalize_spellings(&result);

  // Final cleanup
  result
    .chars()
    .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '/')
    .collect::<String>()
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

/// Expand contractions to full form (word-by-word)
fn expand_contractions(input: &str) -> String {
  input
    .split_whitespace()
    .map(|word| {
      // Check each word against contraction table
      for (contraction, expanded) in CONTRACTIONS {
        if word == *contraction {
          return expanded.to_string();
        }
      }
      word.to_string()
    })
    .collect::<Vec<_>>()
    .join(" ")
}

/// Normalize British spellings to American (word-by-word)
fn normalize_spellings(input: &str) -> String {
  input
    .split_whitespace()
    .map(|word| {
      // Check each word against spelling table
      for (british, american) in SPELLING_EQUIVALENCES {
        if word == *british {
          return american.to_string();
        }
      }
      word.to_string()
    })
    .collect::<Vec<_>>()
    .join(" ")
}

// ============================================================================
// Variant extraction
// ============================================================================

/// Generate all acceptable answers from a parsed answer
/// Returns (full_answers, partial_answers) where:
/// - full_answers: core + disambiguation (if present)
/// - partial_answers: core only (for partial credit)
fn generate_valid_answers(parsed: &ParsedAnswer) -> (Vec<String>, Vec<String>) {
  let mut partial_answers: Vec<String> = Vec::new();

  // Add normalized core answer
  let normalized_core = normalize_answer(&parsed.core);
  if !normalized_core.is_empty() {
    partial_answers.push(normalized_core.clone());
  }

  // Add each variant
  for variant in &parsed.variants {
    let normalized = normalize_answer(variant);
    if !normalized.is_empty() && !partial_answers.contains(&normalized) {
      partial_answers.push(normalized);
    }
  }

  // Add suffix variation if present (e.g., "eye" -> "eyes")
  if let Some(ref suffix) = parsed.suffix {
    let with_suffix = format!("{}{}", parsed.core, suffix);
    let normalized = normalize_answer(&with_suffix);
    if !normalized.is_empty() && !partial_answers.contains(&normalized) {
      partial_answers.push(normalized);
    }
  }

  // Handle comma-separated synonyms in core (e.g., "sofa, couch")
  if parsed.core.contains(',') {
    let parts: Vec<&str> = parsed.core.split(',').map(|s| s.trim()).collect();
    for part in &parts {
      let normalized = normalize_answer(part);
      if !normalized.is_empty() && !partial_answers.contains(&normalized) {
        partial_answers.push(normalized);
      }
    }
  }

  // Handle slash alternatives in core (e.g., "g / k")
  if parsed.core.contains('/') {
    let parts: Vec<&str> = parsed.core.split('/').map(|s| s.trim()).collect();
    for part in &parts {
      let normalized = normalize_answer(part);
      if !normalized.is_empty() && !partial_answers.contains(&normalized) {
        partial_answers.push(normalized);
      }
    }
    // Also add joined version
    let joined = parts.join("/");
    let normalized = normalize_answer(&joined);
    if !normalized.is_empty() && !partial_answers.contains(&normalized) {
      partial_answers.push(normalized);
    }
  }

  // Accept "core + info" when grammar info is present (e.g., "that (thing)" → accept "that thing")
  // This allows users to type the full phrase including parenthetical content
  if let Some(ref info) = parsed.info {
    let normalized_info = normalize_answer(info);
    if !normalized_info.is_empty() {
      let with_info = format!("{} {}", normalized_core, normalized_info);
      if !partial_answers.contains(&with_info) {
        partial_answers.push(with_info);
      }
    }
  }

  // Generate full answers (with disambiguation if present)
  let full_answers = if let Some(ref disambig) = parsed.disambiguation {
    let mut full = Vec::new();
    let normalized_disambig = normalize_answer(disambig);

    for partial in &partial_answers {
      // "that" + "far" → "that far", "far that"
      let forward = format!("{} {}", partial, normalized_disambig);
      let backward = format!("{} {}", normalized_disambig, partial);

      if !full.contains(&forward) {
        full.push(forward);
      }
      if !full.contains(&backward) {
        full.push(backward);
      }
    }
    full
  } else {
    // No disambiguation - full = partial
    partial_answers.clone()
  };

  (full_answers, partial_answers)
}

/// Check if user input matches any permutation/subset of comma-separated terms
/// e.g., "big large" matches "big, large, huge" (subset of synonyms)
fn matches_permutation(user_input: &str, expected: &str) -> bool {
  let input_words: HashSet<String> = user_input.split_whitespace().map(|s| s.to_string()).collect();

  if input_words.is_empty() {
    return false;
  }

  let expected_words: HashSet<String> = expected
    .split([',', ' '])
    .map(|s| s.trim())
    .filter(|s| !s.is_empty())
    .map(|s| s.to_string())
    .collect();

  // Match if all input words are found in expected words (subset check)
  // This allows "sofa couch", "couch sofa", or "big large" for "big, large, huge"
  input_words.iter().all(|word| expected_words.contains(word))
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
  // Parse user input to strip any grammar syntax they might have typed
  // (e.g., if they type "I, me <formal>" exactly as displayed)
  let input_parsed = parse_answer_grammar(user_input);
  let normalized_input = normalize_answer(&input_parsed.core);

  if normalized_input.is_empty() {
    return AnswerResult::Incorrect;
  }

  // Parse the answer grammar
  let parsed = parse_answer_grammar(correct_answer);

  // Special handling for phonetic modifiers (tense/aspirated)
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

  // Generate valid answers
  let (full_answers, partial_answers) = generate_valid_answers(&parsed);

  // Check for exact match with full answers (includes disambiguation if present)
  if full_answers.contains(&normalized_input) {
    return AnswerResult::Correct;
  }

  // Check if user typed "core + info" (e.g., "this thing" for "this (thing)")
  // Only exact match, not typo tolerance - info is supplementary
  if let Some(ref info) = parsed.info {
    let normalized_info = normalize_answer(info);
    if !normalized_info.is_empty() {
      for partial in &partial_answers {
        let with_info = format!("{} {}", partial, normalized_info);
        if normalized_input == with_info {
          return AnswerResult::Correct;
        }
      }
    }
  }

  // Check permutation matching for comma-separated synonyms
  if parsed.core.contains(',') && matches_permutation(&normalized_input, &parsed.core) {
    // If user also provided correct disambiguation, it's full correct
    if user_has_correct_disambig {
      return AnswerResult::Correct;
    }
    return if parsed.disambiguation.is_some() {
      // Core matched via permutation but missing disambiguation
      AnswerResult::PartialMatch
    } else {
      AnswerResult::Correct
    };
  }

  // Check if partial answer matches (core without disambiguation)
  if parsed.disambiguation.is_some() && partial_answers.contains(&normalized_input) {
    // If user also provided correct disambiguation, it's full correct
    if user_has_correct_disambig {
      return AnswerResult::Correct;
    }
    return AnswerResult::PartialMatch;
  }

  // Typo tolerance: check Levenshtein distance against all valid answers
  let all_answers: Vec<&String> = full_answers
    .iter()
    .chain(partial_answers.iter())
    .collect();

  for answer in &all_answers {
    let distance = levenshtein_distance(&normalized_input, answer);
    let char_count = answer.chars().count();

    // Typo tolerance based on answer length
    let max_distance = match char_count {
      0..=2 => 0, // 1-2 chars must be exact (e.g., "I", "me")
      3..=4 => 1, // Short answers: 1 char tolerance
      _ => 2,     // Longer answers: 2 char tolerance
    };

    if distance > 0 && distance <= max_distance {
      // If typo matches a partial answer but disambiguation exists, still PartialMatch
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

  // Core letter part MUST be exact - no tolerance for wrong consonants
  if input_letter != correct_letter {
    return AnswerResult::Incorrect;
  }

  // If the correct answer has a modifier
  if let Some(ref correct_mod) = correct_modifier {
    if let Some(ref input_mod) = input_modifier {
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
    // User typed just the letter without modifier - that's incomplete
    return AnswerResult::Incorrect;
  }

  // No modifier required, letter matches
  AnswerResult::Correct
}

/// Generate progressive hints for an answer
pub struct HintGenerator {
  /// Core answer for hints (grammar stripped)
  core_answer: String,
  /// Full answer with grammar for final hint
  full_answer: String,
  description: Option<String>,
}

impl HintGenerator {
  pub fn new(answer: &str, description: Option<&str>) -> Self {
    // Parse the grammar to extract just the core answer for hints
    let parsed = parse_answer_grammar(answer);
    Self {
      core_answer: parsed.core,
      full_answer: answer.to_string(),
      description: description.map(|s| s.to_string()),
    }
  }

  /// Get hint level 1: First letter and length (uses core answer)
  pub fn hint_level_1(&self) -> String {
    let chars: Vec<char> = self.core_answer.chars().collect();
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
      // Show first two characters of core answer
      let chars: Vec<char> = self.core_answer.chars().collect();
      if chars.len() <= 2 {
        self.core_answer.clone()
      } else {
        let first_two: String = chars[..2].iter().collect();
        let underscores = "_".repeat(chars.len() - 2);
        format!("{}{}", first_two, underscores)
      }
    }
  }

  /// Get final hint: The full answer with grammar markers
  pub fn hint_final(&self) -> String {
    self.full_answer.clone()
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
    // Typo tolerance for 3+ char answers
    assert_eq!(validate_answer("helllo", "hello"), AnswerResult::CloseEnough);
    assert_eq!(validate_answer("helo", "hello"), AnswerResult::CloseEnough);

    // 1-2 char answers require exact match (no typo tolerance)
    assert_eq!(validate_answer("yaa", "ya"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("yo", "ya"), AnswerResult::Incorrect);
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
    // Correct answers
    assert_eq!(AnswerResult::Correct.to_quality(false), 4);
    assert_eq!(AnswerResult::Correct.to_quality(true), 3);
    // CloseEnough (typo) - no penalty since you knew it
    assert_eq!(AnswerResult::CloseEnough.to_quality(false), 4);
    assert_eq!(AnswerResult::CloseEnough.to_quality(true), 3);
    // PartialMatch - knowledge gap (disambiguation missing)
    assert_eq!(AnswerResult::PartialMatch.to_quality(false), 2);
    assert_eq!(AnswerResult::PartialMatch.to_quality(true), 2);
    // Incorrect
    assert_eq!(AnswerResult::Incorrect.to_quality(false), 0);
    assert_eq!(AnswerResult::Incorrect.to_quality(true), 0);
  }

  #[test]
  fn test_allows_retry() {
    assert!(!AnswerResult::Correct.allows_retry());
    assert!(!AnswerResult::CloseEnough.allows_retry());
    assert!(AnswerResult::PartialMatch.allows_retry()); // shake mechanic
    assert!(!AnswerResult::Incorrect.allows_retry());
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
  fn test_hint_generation_with_grammar() {
    // Hints should strip grammar markup and show just the core answer
    let hint_gen = HintGenerator::new("a [short vowel]", None);
    assert_eq!(hint_gen.hint_level_1(), "a (1 letters)"); // Just "a", not 15 chars
    assert_eq!(hint_gen.hint_level_2(), "a");
    assert_eq!(hint_gen.hint_final(), "a [short vowel]"); // Final hint shows full

    // Variants should use core for hints
    let hint_gen = HintGenerator::new("to be [is, am, are, was, were]", None);
    assert_eq!(hint_gen.hint_level_1(), "t____ (5 letters)"); // "to be" = 5 chars
    assert_eq!(hint_gen.hint_level_2(), "to___");
    assert_eq!(hint_gen.hint_final(), "to be [is, am, are, was, were]");

    // Info should be stripped for hints
    let hint_gen = HintGenerator::new("school (noun)", None);
    assert_eq!(hint_gen.hint_level_1(), "s_____ (6 letters)"); // "school" = 6 chars
    assert_eq!(hint_gen.hint_level_2(), "sc____");
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

  // ============================================================================
  // Grammar syntax tests
  // ============================================================================

  #[test]
  fn test_bracket_variants() {
    // [variant] syntax - any item is correct
    assert_eq!(validate_answer("to be", "to be [is, am, are]"), AnswerResult::Correct);
    assert_eq!(validate_answer("is", "to be [is, am, are]"), AnswerResult::Correct);
    assert_eq!(validate_answer("am", "to be [is, am, are]"), AnswerResult::Correct);
    assert_eq!(validate_answer("are", "to be [is, am, are]"), AnswerResult::Correct);
    assert_eq!(validate_answer("being", "to be [is, am, are]"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("was", "to be [is, am, are]"), AnswerResult::Incorrect);
  }

  #[test]
  fn test_suffix_syntax() {
    // word(s) suffix - accept with or without
    assert_eq!(validate_answer("eye", "eye(s)"), AnswerResult::Correct);
    assert_eq!(validate_answer("eyes", "eye(s)"), AnswerResult::Correct);
    assert_eq!(validate_answer("vegetable", "vegetable(s)"), AnswerResult::Correct);
    assert_eq!(validate_answer("vegetables", "vegetable(s)"), AnswerResult::Correct);
    // "eyess" is a typo on "eyes" - CloseEnough (typo tolerance)
    assert_eq!(validate_answer("eyess", "eye(s)"), AnswerResult::CloseEnough);
    // Completely wrong is incorrect
    assert_eq!(validate_answer("ear", "eye(s)"), AnswerResult::Incorrect);

    // Suffix with longer text - note: base must match exactly
    assert_eq!(validate_answer("read", "read(ing)"), AnswerResult::Correct);
    assert_eq!(validate_answer("reading", "read(ing)"), AnswerResult::Correct);
    assert_eq!(validate_answer("town", "town(s)"), AnswerResult::Correct);
    assert_eq!(validate_answer("towns", "town(s)"), AnswerResult::Correct);
  }

  #[test]
  fn test_info_ignored() {
    // (info) syntax - info alone is not valid, core is required
    assert_eq!(validate_answer("that", "that (far)"), AnswerResult::Correct);
    assert_eq!(validate_answer("far", "that (far)"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("run", "run (verb)"), AnswerResult::Correct);
    assert_eq!(validate_answer("verb", "run (verb)"), AnswerResult::Incorrect);

    // User types full phrase including info content - should be accepted
    assert_eq!(validate_answer("that far", "that (far)"), AnswerResult::Correct);
    assert_eq!(validate_answer("run verb", "run (verb)"), AnswerResult::Correct);
    assert_eq!(validate_answer("that thing", "that (thing)"), AnswerResult::Correct);

    // Korean with romanization hint
    assert_eq!(validate_answer("소파", "소파 (so-pa)"), AnswerResult::Correct);
    assert_eq!(validate_answer("so-pa", "소파 (so-pa)"), AnswerResult::Incorrect);

    // Comma-separated synonyms with info - CRITICAL: user may type the whole thing
    assert_eq!(validate_answer("I", "I, me (formal)"), AnswerResult::Correct);
    assert_eq!(validate_answer("me", "I, me (formal)"), AnswerResult::Correct);
    assert_eq!(validate_answer("I me", "I, me (formal)"), AnswerResult::Correct);
    assert_eq!(validate_answer("I, me (formal)", "I, me (formal)"), AnswerResult::Correct); // User types exact answer
    assert_eq!(validate_answer("I, me", "I, me (formal)"), AnswerResult::Correct);
    assert_eq!(validate_answer("formal", "I, me (formal)"), AnswerResult::Incorrect);
  }

  #[test]
  fn test_disambiguation_partial_match() {
    // <context> syntax - partial credit for core only
    assert_eq!(validate_answer("that far", "that <far>"), AnswerResult::Correct);
    assert_eq!(validate_answer("far that", "that <far>"), AnswerResult::Correct);
    assert_eq!(validate_answer("that", "that <far>"), AnswerResult::PartialMatch);
    assert_eq!(validate_answer("this", "that <far>"), AnswerResult::Incorrect);

    // Disambiguation with variants
    assert_eq!(validate_answer("this near", "this <near>"), AnswerResult::Correct);
    assert_eq!(validate_answer("this", "this <near>"), AnswerResult::PartialMatch);

    // Korean formality disambiguation - 저/제 (formal) vs 나/내 (informal)
    assert_eq!(validate_answer("I formal", "I, me <formal>"), AnswerResult::Correct);
    assert_eq!(validate_answer("me formal", "I, me <formal>"), AnswerResult::Correct);
    assert_eq!(validate_answer("formal I", "I, me <formal>"), AnswerResult::Correct);
    assert_eq!(validate_answer("I", "I, me <formal>"), AnswerResult::PartialMatch);
    assert_eq!(validate_answer("me", "I, me <formal>"), AnswerResult::PartialMatch);
    assert_eq!(validate_answer("I me", "I, me <formal>"), AnswerResult::PartialMatch);
    // User types exact displayed answer with angle brackets
    assert_eq!(validate_answer("I, me <formal>", "I, me <formal>"), AnswerResult::Correct);
    // User types with parentheses instead of angle brackets - should still work
    assert_eq!(validate_answer("I, me (formal)", "I, me <formal>"), AnswerResult::Correct);
    assert_eq!(validate_answer("I me (formal)", "I, me <formal>"), AnswerResult::Correct);
    assert_eq!(validate_answer("me (formal)", "I, me <formal>"), AnswerResult::Correct);
    assert_eq!(validate_answer("I (formal)", "I, me <formal>"), AnswerResult::Correct);
  }

  #[test]
  fn test_permutation_matching() {
    // Comma-separated synonyms accept any order
    assert_eq!(validate_answer("sofa", "sofa, couch"), AnswerResult::Correct);
    assert_eq!(validate_answer("couch", "sofa, couch"), AnswerResult::Correct);
    assert_eq!(validate_answer("sofa couch", "sofa, couch"), AnswerResult::Correct);
    assert_eq!(validate_answer("couch sofa", "sofa, couch"), AnswerResult::Correct);
    assert_eq!(validate_answer("chair", "sofa, couch"), AnswerResult::Incorrect);

    // Three synonyms
    assert_eq!(validate_answer("big", "big, large, huge"), AnswerResult::Correct);
    assert_eq!(validate_answer("large", "big, large, huge"), AnswerResult::Correct);
    assert_eq!(validate_answer("huge", "big, large, huge"), AnswerResult::Correct);
    assert_eq!(validate_answer("big large", "big, large, huge"), AnswerResult::Correct);
  }

  #[test]
  fn test_grammar_parser() {
    // Test parse_answer_grammar directly
    let parsed = parse_answer_grammar("to be [is, am, are]");
    assert_eq!(parsed.core, "to be");
    assert_eq!(parsed.variants, vec!["is", "am", "are"]);
    assert!(parsed.suffix.is_none());
    assert!(parsed.disambiguation.is_none());

    let parsed = parse_answer_grammar("eye(s)");
    assert_eq!(parsed.core, "eye");
    assert_eq!(parsed.suffix, Some("s".to_string()));

    let parsed = parse_answer_grammar("that <far>");
    assert_eq!(parsed.core, "that");
    assert_eq!(parsed.disambiguation, Some("far".to_string()));

    let parsed = parse_answer_grammar("run (verb)");
    assert_eq!(parsed.core, "run");
    assert!(parsed.disambiguation.is_none()); // (verb) is info, not disambiguation
  }

  // ============================================================================
  // Normalization tests
  // ============================================================================

  #[test]
  fn test_spelling_normalization() {
    // British to American spelling
    assert_eq!(validate_answer("color", "colour"), AnswerResult::Correct);
    assert_eq!(validate_answer("colour", "color"), AnswerResult::Correct);
    assert_eq!(validate_answer("favorite", "favourite"), AnswerResult::Correct);
    assert_eq!(validate_answer("favourite", "favorite"), AnswerResult::Correct);
    assert_eq!(validate_answer("center", "centre"), AnswerResult::Correct);
    assert_eq!(validate_answer("centre", "center"), AnswerResult::Correct);
  }

  #[test]
  fn test_contraction_normalization() {
    // Contractions expand to full form
    assert_eq!(validate_answer("I am", "I'm"), AnswerResult::Correct);
    assert_eq!(validate_answer("I'm", "I am"), AnswerResult::Correct);
    assert_eq!(validate_answer("do not", "don't"), AnswerResult::Correct);
    assert_eq!(validate_answer("don't", "do not"), AnswerResult::Correct);
    assert_eq!(validate_answer("cannot", "can't"), AnswerResult::Correct);
  }

  #[test]
  fn test_case_normalization() {
    // Case insensitive
    assert_eq!(validate_answer("HELLO", "hello"), AnswerResult::Correct);
    assert_eq!(validate_answer("Hello", "HELLO"), AnswerResult::Correct);
    assert_eq!(validate_answer("hElLo", "hello"), AnswerResult::Correct);
  }

  #[test]
  fn test_whitespace_normalization() {
    // Extra whitespace is ignored
    assert_eq!(validate_answer("  hello  ", "hello"), AnswerResult::Correct);
    assert_eq!(validate_answer("hello   world", "hello world"), AnswerResult::Correct);
  }

  // ============================================================================
  // Combined syntax tests
  // ============================================================================

  #[test]
  fn test_complex_grammar() {
    // Multiple grammar elements combined
    // "I, me (formal)" - comma synonyms + info
    assert_eq!(validate_answer("I", "I, me (formal)"), AnswerResult::Correct);
    assert_eq!(validate_answer("me", "I, me (formal)"), AnswerResult::Correct);
    assert_eq!(validate_answer("formal", "I, me (formal)"), AnswerResult::Incorrect);

    // Variants with suffix
    // Note: this tests that parsing doesn't break with complex input
    assert_eq!(validate_answer("book", "book(s)"), AnswerResult::Correct);
    assert_eq!(validate_answer("books", "book(s)"), AnswerResult::Correct);
  }

  #[test]
  fn test_typo_tolerance_with_grammar() {
    // Typo tolerance applies to grammar-parsed answers
    assert_eq!(validate_answer("to bee", "to be [is, am, are]"), AnswerResult::CloseEnough);
    assert_eq!(validate_answer("soffa", "sofa, couch"), AnswerResult::CloseEnough);
    // But completely wrong is still incorrect
    assert_eq!(validate_answer("xyz", "sofa, couch"), AnswerResult::Incorrect);
  }

  #[test]
  fn test_info_tag_with_user_included() {
    // User types the info tag content - should be accepted
    assert_eq!(validate_answer("this thing", "this (thing)"), AnswerResult::Correct);
    assert_eq!(validate_answer("me formal", "I, me (formal)"), AnswerResult::Correct);
    assert_eq!(validate_answer("I formal", "I, me (formal)"), AnswerResult::Correct);

    // Core alone is still correct (info is optional)
    assert_eq!(validate_answer("this", "this (thing)"), AnswerResult::Correct);
    assert_eq!(validate_answer("I", "I, me (formal)"), AnswerResult::Correct);

    // Info alone should NOT match
    assert_eq!(validate_answer("thing", "this (thing)"), AnswerResult::Incorrect);
    assert_eq!(validate_answer("formal", "I, me (formal)"), AnswerResult::Incorrect);
  }
}
