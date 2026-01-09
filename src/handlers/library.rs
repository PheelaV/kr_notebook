use askama::Template;
use axum::response::Html;

use crate::auth::AuthContext;
use crate::config;
use crate::db::{self, LogOnError};
use crate::filters;
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

/// Library section for the index page
pub struct LibrarySection {
  pub id: String,
  pub name: String,
  pub description: String,
  pub href: String,
  pub count: Option<usize>,
  pub enabled: bool,
}

use super::NavContext;

#[derive(Template)]
#[template(path = "library/index.html")]
pub struct LibraryIndexTemplate {
  pub sections: Vec<LibrarySection>,
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "library/characters.html")]
pub struct LibraryCharactersTemplate {
  pub tiers: Vec<TierGroup>,
  pub max_unlocked_tier: u8,
  pub nav: NavContext,
}

/// Library index/landing page
pub async fn library_index(auth: AuthContext) -> Html<String> {
  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()),
  };

  // Get character count
  let cards = db::get_all_unlocked_cards(&conn).log_warn_default("Failed to get unlocked cards");
  let char_count = cards.iter().filter(|c| !c.is_reverse).count();

  // Start with characters section (always available)
  let mut sections = vec![
    LibrarySection {
      id: "characters".to_string(),
      name: "Characters".to_string(),
      description: "Hangul consonants, vowels, and compound characters organized by tier".to_string(),
      href: "/library/characters".to_string(),
      count: Some(char_count),
      enabled: true,
    },
  ];

  // Only show vocabulary section if user has access to at least one vocab pack
  if auth.has_vocab_access {
    sections.push(LibrarySection {
      id: "vocabulary".to_string(),
      name: "Vocabulary".to_string(),
      description: "Vocabulary words with examples and usage notes".to_string(),
      href: "/library/vocabulary".to_string(),
      count: None,
      enabled: true,
    });
  }

  let template = LibraryIndexTemplate {
    sections,
    nav: NavContext::from_auth(&auth),
  };
  Html(template.render().unwrap_or_default())
}

/// Character library page (formerly /library)
pub async fn library_characters(auth: AuthContext) -> Html<String> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/library/characters".into(),
    method: "GET".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Html("<h1>Database Error</h1><p>Please refresh the page.</p>".to_string()),
  };

  let cards = db::get_all_unlocked_cards(&conn).log_warn_default("Failed to get unlocked cards");
  let max_unlocked_tier = db::get_max_unlocked_tier(&conn).log_warn_default("Failed to get max unlocked tier");

  // Group cards by tier, filtering to only show single-character fronts (the actual jamo)
  let mut tier_map: std::collections::BTreeMap<u8, Vec<LibraryEntry>> = std::collections::BTreeMap::new();

  for card in cards {
    // Only include forward cards (Korean -> romanization)
    // Skip reverse cards (romanization -> Korean)
    if !card.is_reverse {
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
      tier_name: config::get_tier_name(tier),
      entries,
    })
    .collect();

  let template = LibraryCharactersTemplate {
    tiers,
    max_unlocked_tier,
    nav: NavContext::from_auth(&auth),
  };

  Html(template.render().unwrap_or_default())
}
