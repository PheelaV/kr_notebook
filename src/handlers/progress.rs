use askama::Template;
use axum::{
  extract::State,
  response::{Html, Redirect},
};

use crate::db::{self, DbPool, TierProgress};

#[derive(Template)]
#[template(path = "progress.html")]
pub struct ProgressTemplate {
  pub total_cards: i64,
  pub total_reviews: i64,
  pub cards_learned: i64,
  pub tiers: Vec<TierProgress>,
  pub max_unlocked_tier: u8,
  pub can_unlock_next: bool,
}

pub async fn progress(State(pool): State<DbPool>) -> Html<String> {
  let conn = pool.lock().unwrap();

  let (total_cards, total_reviews, cards_learned) =
    db::get_total_stats(&conn).unwrap_or((0, 0, 0));
  let tiers = db::get_progress_by_tier(&conn).unwrap_or_default();
  let max_unlocked_tier = db::get_max_unlocked_tier(&conn).unwrap_or(1);

  // Can unlock next tier if current tier has >= 80% learned
  let can_unlock_next = if max_unlocked_tier < 4 {
    tiers
      .iter()
      .find(|t| t.tier == max_unlocked_tier)
      .map(|t| t.percentage() >= 80)
      .unwrap_or(false)
  } else {
    false
  };

  let template = ProgressTemplate {
    total_cards,
    total_reviews,
    cards_learned,
    tiers,
    max_unlocked_tier,
    can_unlock_next,
  };

  Html(template.render().unwrap_or_default())
}

pub async fn unlock_tier(State(pool): State<DbPool>) -> Redirect {
  let conn = pool.lock().unwrap();
  let _ = db::unlock_next_tier(&conn);
  Redirect::to("/progress")
}
