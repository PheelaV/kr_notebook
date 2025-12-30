//! Study handlers for SRS review sessions.

mod classic;
mod interactive;
mod practice;
mod templates;

use rand::seq::SliceRandom;

use crate::config;
use crate::db::{self, LogOnError};
use crate::domain::{Card, ReviewDirection};

// Re-export public items
pub use classic::{study_start, submit_review};
pub use interactive::{
  next_card_interactive, study_start_interactive, submit_review_interactive,
  validate_answer_handler,
};
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

/// Generate multiple choice options for a card
pub(crate) fn generate_choices(card: &Card, all_cards: &[Card]) -> Vec<String> {
  let correct = card.main_answer.clone();

  // Get other cards from the same tier with Korean answers
  let mut distractors: Vec<String> = all_cards
    .iter()
    .filter(|c| c.id != card.id && c.tier == card.tier && is_korean(&c.main_answer))
    .map(|c| c.main_answer.clone())
    .collect();

  // Shuffle and take distractors
  let mut rng = rand::rng();
  distractors.shuffle(&mut rng);
  distractors.truncate(config::DISTRACTOR_COUNT);

  // Combine correct answer with distractors
  let mut choices = vec![correct];
  choices.extend(distractors);

  // Shuffle final choices
  choices.shuffle(&mut rng);

  choices
}

/// Get all available cards for study (due + unreviewed in accelerated mode)
pub(crate) fn get_available_study_cards(
  conn: &std::sync::MutexGuard<'_, rusqlite::Connection>,
) -> Vec<Card> {
  let use_interleaving =
    db::get_use_interleaving(conn).log_warn_default("Failed to get interleaving setting");
  let accelerated =
    db::get_all_tiers_unlocked(conn).log_warn_default("Failed to get accelerated mode setting");

  let mut cards = Vec::new();

  // Get due cards
  let due = if use_interleaving {
    db::get_due_cards_interleaved(conn, 50, None)
      .log_warn_default("Failed to get interleaved due cards")
  } else {
    db::get_due_cards(conn, 50, None).log_warn_default("Failed to get due cards")
  };
  cards.extend(due);

  // In accelerated mode, also get unreviewed cards
  if accelerated {
    let unreviewed =
      db::get_unreviewed_today(conn, 50, None).log_warn_default("Failed to get unreviewed cards");
    // Avoid duplicates
    for card in unreviewed {
      if !cards.iter().any(|c| c.id == card.id) {
        cards.push(card);
      }
    }
  }

  cards
}
