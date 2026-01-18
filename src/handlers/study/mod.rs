//! Study handlers for SRS review sessions.

mod classic;
mod interactive;
mod offline;
mod practice;
mod templates;

use rand::seq::SliceRandom;

use crate::config;
use crate::db::{self, LogOnError};
use crate::domain::{Card, ReviewDirection};

// Re-export public items
pub use classic::{study_start, submit_review};
pub use interactive::{
  next_card_interactive, override_ruling_handler, set_study_filter, study_start_interactive,
  submit_review_interactive, toggle_focus_mode, validate_answer_handler, OverrideForm,
};
pub use offline::{download_session, sync_session, DownloadSessionRequest, SyncSessionRequest};
pub use practice::{practice_next, practice_start, practice_validate};
pub use templates::{
  CardTemplate, InteractiveCardTemplate, NextCardForm, NoCardsTemplate, PracticeCardTemplate,
  PracticeForm, PracticeQuery, PracticeTemplate, PracticeValidateForm, ReviewForm,
  StudyInteractiveTemplate, StudyTemplate, ValidateAnswerForm,
};

/// Determine the review direction based on card type
pub(crate) fn get_review_direction(card: &Card) -> ReviewDirection {
  if card.is_reverse {
    ReviewDirection::RomToKr
  } else {
    ReviewDirection::KrToRom
  }
}

/// Get character type string for stats tracking
pub(crate) fn get_character_type(card: &Card) -> &'static str {
  card.card_type.as_str()
}

/// Get the character to track stats for (the Korean character being learned)
pub(crate) fn get_tracked_character(card: &Card) -> &str {
  if card.is_reverse {
    // Reverse card: answer is Korean
    &card.main_answer
  } else {
    // Forward card: front is Korean
    &card.front
  }
}

/// Check if a string contains Korean characters (Hangul)
pub(crate) fn is_korean(s: &str) -> bool {
  s.chars().any(|c| {
    // Hangul Syllables: U+AC00 to U+D7A3
    // Hangul Jamo: U+1100 to U+11FF
    // Hangul Compatibility Jamo: U+3130 to U+318F
    let code = c as u32;
    (0xAC00..=0xD7A3).contains(&code)
      || (0x1100..=0x11FF).contains(&code)
      || (0x3130..=0x318F).contains(&code)
  })
}

/// Generate multiple choice options for a card with fallback expansion.
///
/// Uses a 3-phase approach to ensure enough distractors:
/// 1. Same tier/lesson (most relevant)
/// 2. Adjacent tiers ±1 (for Hangul cards)
/// 3. Any Korean answer (last resort)
pub(crate) fn generate_choices(card: &Card, all_cards: &[Card]) -> Vec<String> {
  let correct = card.main_answer.clone();
  let needed = config::DISTRACTOR_COUNT; // 3 distractors for 4 total choices

  let mut distractors: Vec<String> = Vec::new();
  let mut rng = rand::rng();

  // --- Phase 1: Same tier/lesson (most relevant) ---
  if card.pack_id.is_some() && card.lesson.is_some() {
    // Vocabulary card: get distractors from same pack/lesson
    let same_lesson: Vec<String> = all_cards
      .iter()
      .filter(|c| {
        c.id != card.id
          && c.pack_id == card.pack_id
          && c.lesson == card.lesson
          && is_korean(&c.main_answer)
          && c.main_answer != correct
      })
      .map(|c| c.main_answer.clone())
      .collect();
    distractors.extend(same_lesson);
  } else {
    // Hangul card: get distractors from same tier
    let same_tier: Vec<String> = all_cards
      .iter()
      .filter(|c| {
        c.id != card.id
          && c.tier == card.tier
          && is_korean(&c.main_answer)
          && c.main_answer != correct
      })
      .map(|c| c.main_answer.clone())
      .collect();
    distractors.extend(same_tier);
  }

  // Deduplicate and shuffle phase 1 results
  distractors.sort();
  distractors.dedup();
  distractors.shuffle(&mut rng);

  // --- Phase 2: Adjacent tiers (if needed, Hangul cards only) ---
  if distractors.len() < needed && card.pack_id.is_none() {
    let adjacent: Vec<String> = all_cards
      .iter()
      .filter(|c| {
        c.id != card.id
          && (c.tier == card.tier.saturating_sub(1) || c.tier == card.tier + 1)
          && is_korean(&c.main_answer)
          && c.main_answer != correct
          && !distractors.contains(&c.main_answer)
      })
      .map(|c| c.main_answer.clone())
      .collect();

    let mut adjacent_unique: Vec<String> = adjacent;
    adjacent_unique.sort();
    adjacent_unique.dedup();
    adjacent_unique.shuffle(&mut rng);
    distractors.extend(adjacent_unique);
  }

  // --- Phase 3: Any Korean answer (last resort) ---
  if distractors.len() < needed {
    let any_korean: Vec<String> = all_cards
      .iter()
      .filter(|c| {
        c.id != card.id
          && is_korean(&c.main_answer)
          && c.main_answer != correct
          && !distractors.contains(&c.main_answer)
      })
      .map(|c| c.main_answer.clone())
      .collect();

    let mut any_unique: Vec<String> = any_korean;
    any_unique.sort();
    any_unique.dedup();
    any_unique.shuffle(&mut rng);
    distractors.extend(any_unique);
  }

  // Take only what we need
  distractors.truncate(needed);

  // Combine correct answer with distractors
  let mut choices = vec![correct];
  choices.extend(distractors);

  // Shuffle final choices
  choices.shuffle(&mut rng);

  choices
}

/// Convert filter string to StudyFilterMode
/// Supports: "all", "hangul", "pack:X", "pack:X:lesson:N"
pub(crate) fn parse_filter_mode(filter: &str) -> db::StudyFilterMode {
  match filter {
    "all" => db::StudyFilterMode::All,
    "hangul" => db::StudyFilterMode::HangulOnly,
    s if s.starts_with("pack:") => {
      let rest = s.strip_prefix("pack:").unwrap_or("");
      // Check for lesson format: pack:X:lesson:N
      if let Some((pack_id, lesson_part)) = rest.split_once(":lesson:") {
        if let Ok(lesson_num) = lesson_part.parse::<u8>() {
          return db::StudyFilterMode::PackLesson(pack_id.to_string(), lesson_num);
        }
      }
      // Otherwise, it's just pack:X
      db::StudyFilterMode::PackOnly(rest.to_string())
    }
    _ => db::StudyFilterMode::All,
  }
}

/// Get all available cards for study (due + unreviewed in accelerated mode)
/// Optionally filtered by content type
pub(crate) fn get_available_study_cards(
  conn: &std::sync::MutexGuard<'_, rusqlite::Connection>,
  app_conn: &std::sync::MutexGuard<'_, rusqlite::Connection>,
  user_id: i64,
) -> Vec<Card> {
  // Get study filter from settings
  let filter_str = db::get_setting(conn, "study_filter_mode")
    .ok()
    .flatten()
    .unwrap_or_else(|| "all".to_string());
  let filter = parse_filter_mode(&filter_str);

  get_available_study_cards_filtered(conn, app_conn, user_id, &filter)
}

/// Get all available cards for study with explicit filter
pub(crate) fn get_available_study_cards_filtered(
  conn: &std::sync::MutexGuard<'_, rusqlite::Connection>,
  app_conn: &std::sync::MutexGuard<'_, rusqlite::Connection>,
  user_id: i64,
  filter: &db::StudyFilterMode,
) -> Vec<Card> {
  let use_interleaving =
    db::get_use_interleaving(conn).log_warn_default("Failed to get interleaving setting");
  let accelerated =
    db::get_all_tiers_unlocked(conn).log_warn_default("Failed to get accelerated mode setting");

  // Check if we can introduce new cards (daily limit)
  let can_add_new = db::can_introduce_new_card(conn).unwrap_or(true);

  let mut cards = Vec::new();

  // Get due cards (filtered)
  let due = if use_interleaving {
    db::get_due_cards_interleaved_filtered(conn, app_conn, user_id, 50, None, filter)
      .log_warn_default("Failed to get interleaved due cards")
  } else {
    db::get_due_cards_filtered(conn, app_conn, user_id, 50, None, filter)
      .log_warn_default("Failed to get due cards")
  };

  // Filter out brand new cards if daily limit is reached
  for card in due {
    if card.total_reviews > 0 || can_add_new {
      cards.push(card);
    }
  }

  // In accelerated mode, also get unreviewed cards (filtered)
  if accelerated && can_add_new {
    let unreviewed = db::get_unreviewed_today_filtered(conn, app_conn, user_id, 50, None, filter)
      .log_warn_default("Failed to get unreviewed cards");
    // Avoid duplicates
    for card in unreviewed {
      if !cards.iter().any(|c| c.id == card.id) {
        cards.push(card);
      }
    }
  }

  cards
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::CardType;

  #[test]
  fn test_is_korean_hangul_syllables() {
    // Hangul syllables (U+AC00 to U+D7A3)
    assert!(is_korean("가"));
    assert!(is_korean("나"));
    assert!(is_korean("한글"));
    assert!(is_korean("안녕하세요"));
  }

  #[test]
  fn test_is_korean_hangul_jamo() {
    // Hangul Jamo (U+1100 to U+11FF)
    assert!(is_korean("\u{1100}")); // ᄀ
    assert!(is_korean("\u{1161}")); // ᅡ
  }

  #[test]
  fn test_is_korean_compatibility_jamo() {
    // Hangul Compatibility Jamo (U+3130 to U+318F)
    assert!(is_korean("ㄱ"));
    assert!(is_korean("ㅏ"));
    assert!(is_korean("ㅎ"));
  }

  #[test]
  fn test_is_korean_non_korean() {
    assert!(!is_korean("abc"));
    assert!(!is_korean("hello"));
    assert!(!is_korean("123"));
    assert!(!is_korean(""));
    assert!(!is_korean("日本語"));
    assert!(!is_korean("中文"));
  }

  #[test]
  fn test_is_korean_mixed_content() {
    // Mixed content with at least one Korean character
    assert!(is_korean("hello가"));
    assert!(is_korean("123한글abc"));
    assert!(is_korean("test ㄱ test"));
  }

  #[test]
  fn test_get_review_direction_forward() {
    let card = Card {
      id: 1,
      front: "가".to_string(),
      main_answer: "ga".to_string(),
      description: None,
      card_type: CardType::Vowel,
      tier: 1,
      audio_hint: None,
      is_reverse: false,
      pack_id: None,
      lesson: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: chrono::Utc::now(),
      total_reviews: 0,
      correct_reviews: 0,
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
    };

    assert_eq!(get_review_direction(&card), ReviewDirection::KrToRom);
  }

  #[test]
  fn test_get_review_direction_reverse() {
    let card = Card {
      id: 1,
      front: "ga".to_string(),
      main_answer: "가".to_string(),
      description: None,
      card_type: CardType::Vowel,
      tier: 1,
      audio_hint: None,
      is_reverse: true,
      pack_id: None,
      lesson: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: chrono::Utc::now(),
      total_reviews: 0,
      correct_reviews: 0,
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
    };

    assert_eq!(get_review_direction(&card), ReviewDirection::RomToKr);
  }

  #[test]
  fn test_get_character_type() {
    let mut card = Card {
      id: 1,
      front: "가".to_string(),
      main_answer: "ga".to_string(),
      description: None,
      card_type: CardType::Vowel,
      tier: 1,
      audio_hint: None,
      is_reverse: false,
      pack_id: None,
      lesson: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: chrono::Utc::now(),
      total_reviews: 0,
      correct_reviews: 0,
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
    };

    assert_eq!(get_character_type(&card), "vowel");

    card.card_type = CardType::Consonant;
    assert_eq!(get_character_type(&card), "consonant");

    card.card_type = CardType::Syllable;
    assert_eq!(get_character_type(&card), "syllable");

    card.card_type = CardType::Vocabulary;
    assert_eq!(get_character_type(&card), "vocabulary");
  }

  #[test]
  fn test_get_tracked_character_forward() {
    let card = Card {
      id: 1,
      front: "가".to_string(),
      main_answer: "ga".to_string(),
      description: None,
      card_type: CardType::Vowel,
      tier: 1,
      audio_hint: None,
      is_reverse: false,
      pack_id: None,
      lesson: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: chrono::Utc::now(),
      total_reviews: 0,
      correct_reviews: 0,
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
    };

    // Forward card: front is Korean
    assert_eq!(get_tracked_character(&card), "가");
  }

  #[test]
  fn test_get_tracked_character_reverse() {
    let card = Card {
      id: 1,
      front: "ga".to_string(),
      main_answer: "가".to_string(),
      description: None,
      card_type: CardType::Vowel,
      tier: 1,
      audio_hint: None,
      is_reverse: true,
      pack_id: None,
      lesson: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: chrono::Utc::now(),
      total_reviews: 0,
      correct_reviews: 0,
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
    };

    // Reverse card: answer is Korean
    assert_eq!(get_tracked_character(&card), "가");
  }

  #[test]
  fn test_parse_filter_mode_all() {
    assert!(matches!(parse_filter_mode("all"), db::StudyFilterMode::All));
  }

  #[test]
  fn test_parse_filter_mode_hangul() {
    assert!(matches!(
      parse_filter_mode("hangul"),
      db::StudyFilterMode::HangulOnly
    ));
  }

  #[test]
  fn test_parse_filter_mode_pack() {
    match parse_filter_mode("pack:vocabulary-lesson1") {
      db::StudyFilterMode::PackOnly(id) => assert_eq!(id, "vocabulary-lesson1"),
      _ => panic!("Expected PackOnly variant"),
    }
  }

  #[test]
  fn test_parse_filter_mode_unknown_defaults_to_all() {
    assert!(matches!(
      parse_filter_mode("unknown"),
      db::StudyFilterMode::All
    ));
    assert!(matches!(parse_filter_mode(""), db::StudyFilterMode::All));
  }

  #[test]
  fn test_generate_choices_includes_correct_answer() {
    let card = Card {
      id: 1,
      front: "가".to_string(),
      main_answer: "나".to_string(),
      description: None,
      card_type: CardType::Vowel,
      tier: 1,
      audio_hint: None,
      is_reverse: false,
      pack_id: None,
      lesson: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: chrono::Utc::now(),
      total_reviews: 0,
      correct_reviews: 0,
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
    };

    let other_cards = vec![
      Card {
        id: 2,
        front: "다".to_string(),
        main_answer: "라".to_string(),
        description: None,
        card_type: CardType::Vowel,
        tier: 1,
        audio_hint: None,
        is_reverse: false,
        pack_id: None,
        lesson: None,
        ease_factor: 2.5,
        interval_days: 0,
        repetitions: 0,
        next_review: chrono::Utc::now(),
        total_reviews: 0,
        correct_reviews: 0,
        learning_step: 0,
        fsrs_stability: None,
        fsrs_difficulty: None,
        fsrs_state: None,
      },
      Card {
        id: 3,
        front: "마".to_string(),
        main_answer: "바".to_string(),
        description: None,
        card_type: CardType::Vowel,
        tier: 1,
        audio_hint: None,
        is_reverse: false,
        pack_id: None,
        lesson: None,
        ease_factor: 2.5,
        interval_days: 0,
        repetitions: 0,
        next_review: chrono::Utc::now(),
        total_reviews: 0,
        correct_reviews: 0,
        learning_step: 0,
        fsrs_stability: None,
        fsrs_difficulty: None,
        fsrs_state: None,
      },
    ];

    let choices = generate_choices(&card, &other_cards);

    // Correct answer should always be included
    assert!(choices.contains(&"나".to_string()));
  }

  #[test]
  fn test_generate_choices_excludes_non_korean_distractors() {
    let card = Card {
      id: 1,
      front: "가".to_string(),
      main_answer: "나".to_string(),
      description: None,
      card_type: CardType::Vowel,
      tier: 1,
      audio_hint: None,
      is_reverse: false,
      pack_id: None,
      lesson: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: chrono::Utc::now(),
      total_reviews: 0,
      correct_reviews: 0,
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
    };

    let other_cards = vec![Card {
      id: 2,
      front: "다".to_string(),
      main_answer: "romanized".to_string(), // Non-Korean answer
      description: None,
      card_type: CardType::Vowel,
      tier: 1,
      audio_hint: None,
      is_reverse: false,
      pack_id: None,
      lesson: None,
      ease_factor: 2.5,
      interval_days: 0,
      repetitions: 0,
      next_review: chrono::Utc::now(),
      total_reviews: 0,
      correct_reviews: 0,
      learning_step: 0,
      fsrs_stability: None,
      fsrs_difficulty: None,
      fsrs_state: None,
    }];

    let choices = generate_choices(&card, &other_cards);

    // Non-Korean answers should not appear as distractors
    assert!(!choices.contains(&"romanized".to_string()));
  }
}
