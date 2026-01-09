use askama::Template;
use axum::extract::State;
use axum::response::{Html, Redirect};

use crate::auth::AuthContext;
use super::NavContext;
use crate::content::cards::list_enabled_packs;
use crate::db::{
    self, CharacterStats, LogOnError, PackProgress, TierProgress,
    get_pack_progress, get_pack_ui_metadata,
};
use crate::filters;
use crate::state::AppState;
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// A card that the user frequently gets wrong - shows high confusion rates
#[derive(Debug, Clone)]
pub struct ProblemCard {
  pub id: i64,
  pub front: String,
  pub confusion_count: i64,
  pub top_wrong_answers: Vec<String>,
}

/// Character stats formatted for display
#[derive(Debug, Clone)]
pub struct CharacterStatsDisplay {
  pub character: String,
  pub character_type: String,
  pub lifetime_pct: i32,
  pub rate_7d_pct: i32,
  pub rate_1d_pct: i32,
  pub attempts_1d: i64,
  pub status: &'static str,
  pub status_color: &'static str,
}

impl From<CharacterStats> for CharacterStatsDisplay {
  fn from(stats: CharacterStats) -> Self {
    let lifetime_pct = (stats.lifetime_rate() * 100.0).round() as i32;
    let rate_7d_pct = (stats.rate_7d() * 100.0).round() as i32;
    let rate_1d_pct = (stats.rate_1d() * 100.0).round() as i32;

    // Determine status based on 24-hour rate
    let (status, status_color) = if stats.attempts_1d == 0 {
      ("—", "text-gray-400")
    } else if rate_1d_pct >= 90 {
      ("Strong", "text-green-600 dark:text-green-400")
    } else if rate_1d_pct >= 70 {
      ("OK", "text-yellow-600 dark:text-yellow-400")
    } else {
      ("Weak", "text-red-600 dark:text-red-400")
    };

    Self {
      character: stats.character,
      character_type: stats.character_type,
      lifetime_pct,
      rate_7d_pct,
      rate_1d_pct,
      attempts_1d: stats.attempts_1d,
      status,
      status_color,
    }
  }
}

/// Group of character stats by type
#[derive(Debug, Clone)]
pub struct CharacterStatsGroup {
  pub type_name: String,
  pub type_label: String,
  pub stats: Vec<CharacterStatsDisplay>,
}

/// Generic progress unit - can be Hangul tiers or vocabulary pack lessons
#[derive(Debug, Clone)]
pub enum ProgressUnit {
  /// Hangul character learning tiers
  HangulTiers {
    tiers: Vec<TierProgress>,
    max_unlocked_tier: u8,
    can_unlock_next: bool,
    all_tiers_unlocked: bool,
  },
  /// Vocabulary pack with lesson-based progression
  VocabularyPack(PackProgress),
}

#[derive(Template)]
#[template(path = "progress.html")]
pub struct ProgressTemplate {
  pub total_cards: i64,
  pub total_reviews: i64,
  pub cards_learned: i64,
  // Hangul tier progress (kept for backwards compatibility with existing template)
  pub tiers: Vec<TierProgress>,
  pub max_unlocked_tier: u8,
  pub can_unlock_next: bool,
  pub all_tiers_unlocked: bool,
  // Generic progress units (includes vocabulary packs)
  pub units: Vec<ProgressUnit>,
  pub problem_cards: Vec<ProblemCard>,
  pub character_stats_groups: Vec<CharacterStatsGroup>,
  pub nav: NavContext,
}

pub async fn progress(
  State(state): State<AppState>,
  auth: AuthContext,
) -> axum::response::Response {
  use axum::response::IntoResponse;

  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/progress".into(),
    method: "GET".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/").into_response(),
  };

  let all_tiers_unlocked = db::get_all_tiers_unlocked(&conn).log_warn_default("Failed to get all_tiers_unlocked");
  let (total_cards, total_reviews, cards_learned) =
    db::get_total_stats(&conn).log_warn_default("Failed to get total stats");
  let tiers = db::get_progress_by_tier(&conn).log_warn_default("Failed to get progress by tier");
  let max_unlocked_tier = db::get_max_unlocked_tier(&conn).log_warn_default("Failed to get max unlocked tier");

  // Can unlock next tier if current tier has >= 80% learned (disabled if all unlocked)
  let can_unlock_next = if !all_tiers_unlocked && max_unlocked_tier < 4 {
    tiers
      .iter()
      .find(|t| t.tier == max_unlocked_tier)
      .map(|t| t.percentage() >= 80)
      .unwrap_or(false)
  } else {
    false
  };

  // Get problem cards (cards with most confusions)
  let problem_cards_raw = db::get_problem_cards(&conn, 5).log_warn_default("Failed to get problem cards");
  let problem_cards: Vec<ProblemCard> = problem_cards_raw
    .into_iter()
    .map(|(id, front, count)| {
      let top_wrong = db::get_card_confusions(&conn, id, 3)
        .log_warn_default("Failed to get card confusions")
        .into_iter()
        .map(|(answer, _)| answer)
        .collect();
      ProblemCard {
        id,
        front,
        confusion_count: count,
        top_wrong_answers: top_wrong,
      }
    })
    .collect();

  // Get character stats grouped by type
  let all_stats = db::get_all_character_stats(&conn).log_warn_default("Failed to get character stats");
  let character_stats_groups = build_character_stats_groups(all_stats);

  // Build progress units (Hangul tiers + vocabulary packs)
  let mut units = Vec::new();

  // Unit 1: Hangul tiers (always present)
  units.push(ProgressUnit::HangulTiers {
    tiers: tiers.clone(),
    max_unlocked_tier,
    can_unlock_next,
    all_tiers_unlocked,
  });

  // Additional units: enabled vocabulary packs with lessons
  // Need app_db connection for pack UI metadata
  if let Ok(app_conn) = state.auth_db.lock() {
    let enabled_packs = list_enabled_packs(&conn);

    for pack_id in enabled_packs {
      // Only include packs that have UI metadata (lesson-based progression)
      if let Ok(Some(ui_metadata)) = get_pack_ui_metadata(&app_conn, &pack_id) {
        if ui_metadata.total_lessons.unwrap_or(0) > 0 {
          match get_pack_progress(&conn, &app_conn, &pack_id, &ui_metadata) {
            Ok(pack_progress) => {
              units.push(ProgressUnit::VocabularyPack(pack_progress));
            }
            Err(e) => {
              tracing::warn!("Failed to get pack progress for {}: {}", pack_id, e);
            }
          }
        }
      }
    }
  }

  let template = ProgressTemplate {
    total_cards,
    total_reviews,
    cards_learned,
    tiers,
    max_unlocked_tier,
    can_unlock_next,
    all_tiers_unlocked,
    units,
    problem_cards,
    character_stats_groups,
    nav: NavContext::from_auth(&auth),
  };

  Html(template.render().unwrap_or_default()).into_response()
}

/// Build character stats groups from raw stats
fn build_character_stats_groups(all_stats: Vec<CharacterStats>) -> Vec<CharacterStatsGroup> {
  let type_order = [
    ("consonant", "Basic Consonants"),
    ("vowel", "Basic Vowels"),
    ("aspirated_consonant", "Aspirated Consonants"),
    ("tense_consonant", "Tense Consonants"),
    ("compound_vowel", "Compound Vowels"),
  ];

  let mut groups = Vec::new();

  for (type_name, type_label) in type_order {
    let stats: Vec<CharacterStatsDisplay> = all_stats
      .iter()
      .filter(|s| s.character_type == type_name)
      .cloned()
      .map(CharacterStatsDisplay::from)
      .collect();

    if !stats.is_empty() {
      groups.push(CharacterStatsGroup {
        type_name: type_name.to_string(),
        type_label: type_label.to_string(),
        stats,
      });
    }
  }

  groups
}

pub async fn unlock_tier(auth: AuthContext) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/unlock-tier".into(),
    method: "POST".into(),
    username: Some(auth.username.clone()),
  });

  let conn = match auth.user_db.lock() {
    Ok(conn) => conn,
    Err(_) => return Redirect::to("/progress"),
  };

  #[cfg(feature = "profiling")]
  if let Ok(new_tier) = db::unlock_next_tier(&conn) {
    crate::profile_log!(EventType::TierUnlock {
      tier: new_tier,
      username: auth.username.clone(),
    });
  }

  #[cfg(not(feature = "profiling"))]
  let _ = db::unlock_next_tier(&conn);

  Redirect::to("/progress")
}
