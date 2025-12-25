use askama::Template;
use axum::{
  extract::State,
  response::{Html, Redirect},
};

use crate::db::{self, DbPool, TierProgress};
#[cfg(feature = "profiling")]
use crate::profiling::EventType;

/// A card that the user frequently gets wrong
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProblemCard {
  pub id: i64,
  pub front: String,
  pub confusion_count: i64,
  pub top_wrong_answers: Vec<String>,
}

#[derive(Template)]
#[template(path = "progress.html")]
pub struct ProgressTemplate {
  pub total_cards: i64,
  pub total_reviews: i64,
  pub cards_learned: i64,
  pub tiers: Vec<TierProgress>,
  pub max_unlocked_tier: u8,
  pub can_unlock_next: bool,
  pub all_tiers_unlocked: bool,
  pub problem_cards: Vec<ProblemCard>,
}

pub async fn progress(State(pool): State<DbPool>) -> Html<String> {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/progress".into(),
    method: "GET".into(),
  });

  let conn = pool.lock().unwrap();

  let all_tiers_unlocked = db::get_all_tiers_unlocked(&conn).unwrap_or(false);
  let (total_cards, total_reviews, cards_learned) =
    db::get_total_stats(&conn).unwrap_or((0, 0, 0));
  let tiers = db::get_progress_by_tier(&conn).unwrap_or_default();
  let max_unlocked_tier = db::get_max_unlocked_tier(&conn).unwrap_or(1);

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
  let problem_cards_raw = db::get_problem_cards(&conn, 5).unwrap_or_default();
  let problem_cards: Vec<ProblemCard> = problem_cards_raw
    .into_iter()
    .map(|(id, front, count)| {
      let top_wrong = db::get_card_confusions(&conn, id, 3)
        .unwrap_or_default()
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

  let template = ProgressTemplate {
    total_cards,
    total_reviews,
    cards_learned,
    tiers,
    max_unlocked_tier,
    can_unlock_next,
    all_tiers_unlocked,
    problem_cards,
  };

  Html(template.render().unwrap_or_default())
}

pub async fn unlock_tier(State(pool): State<DbPool>) -> Redirect {
  #[cfg(feature = "profiling")]
  crate::profile_log!(EventType::HandlerStart {
    route: "/unlock-tier".into(),
    method: "POST".into(),
  });

  let conn = pool.lock().unwrap();
  let _ = db::unlock_next_tier(&conn);
  Redirect::to("/progress")
}
