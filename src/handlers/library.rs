use askama::Template;
use axum::{extract::State, response::Html};

use crate::db::{self, DbPool};
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// A character entry for the library display
pub struct LibraryEntry {
  pub front: String,
  pub main_answer: String,
  pub description: Option<String>,
}

/// Characters grouped by tier
pub struct TierGroup {
  pub tier: u8,
  pub tier_name: String,
  pub entries: Vec<LibraryEntry>,
}

#[derive(Template)]
#[template(path = "library.html")]
pub struct LibraryTemplate {
  pub tiers: Vec<TierGroup>,
  pub max_unlocked_tier: u8,
}

fn get_tier_name(tier: u8) -> String {
  match tier {
    1 => "Basic Consonants & Vowels".to_string(),
    2 => "Y-Vowels & Special".to_string(),
    3 => "Aspirated & Tense Consonants".to_string(),
    4 => "Compound Vowels".to_string(),
    _ => format!("Tier {}", tier),
  }
}

pub async fn library(State(pool): State<DbPool>) -> Html<String> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/library".into(),
    method: "GET".into(),
  });

  let conn = pool.lock().unwrap();

  let cards = db::get_unlocked_cards(&conn).unwrap_or_default();
  let max_unlocked_tier = db::get_max_unlocked_tier(&conn).unwrap_or(1);

  // Group cards by tier, filtering to only show single-character fronts (the actual jamo)
  let mut tier_map: std::collections::BTreeMap<u8, Vec<LibraryEntry>> = std::collections::BTreeMap::new();

  for card in cards {
    // Only include cards where front is a single Korean character (jamo)
    // Skip reverse cards like "Which letter sounds like..."
    if card.front.chars().count() == 1 {
      let entry = LibraryEntry {
        front: card.front,
        main_answer: card.main_answer,
        description: card.description,
      };

      tier_map.entry(card.tier).or_default().push(entry);
    }
  }

  // Convert to Vec<TierGroup>
  let tiers: Vec<TierGroup> = tier_map
    .into_iter()
    .map(|(tier, entries)| TierGroup {
      tier,
      tier_name: get_tier_name(tier),
      entries,
    })
    .collect();

  let template = LibraryTemplate {
    tiers,
    max_unlocked_tier,
  };

  Html(template.render().unwrap_or_default())
}
