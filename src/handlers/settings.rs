use askama::Template;
use axum::{
  extract::State,
  response::{Html, Redirect},
  Form,
};
use serde::Deserialize;

use crate::db::{self, DbPool};

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
  pub all_tiers_unlocked: bool,
  pub enabled_tiers: Vec<u8>,
}

pub async fn settings_page(State(pool): State<DbPool>) -> Html<String> {
  let conn = pool.lock().unwrap();
  let all_tiers_unlocked = db::get_all_tiers_unlocked(&conn).unwrap_or(false);
  let enabled_tiers = db::get_enabled_tiers(&conn).unwrap_or_else(|_| vec![1, 2, 3, 4]);

  let template = SettingsTemplate {
    all_tiers_unlocked,
    enabled_tiers,
  };
  Html(template.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct SettingsForm {
  #[serde(default)]
  pub all_tiers_unlocked: Option<String>,
  #[serde(default)]
  pub tier_1: Option<String>,
  #[serde(default)]
  pub tier_2: Option<String>,
  #[serde(default)]
  pub tier_3: Option<String>,
  #[serde(default)]
  pub tier_4: Option<String>,
}

pub async fn update_settings(
  State(pool): State<DbPool>,
  Form(form): Form<SettingsForm>,
) -> Redirect {
  let conn = pool.lock().unwrap();

  // Update all_tiers_unlocked
  let all_tiers_unlocked = form.all_tiers_unlocked.is_some();
  let _ = db::set_all_tiers_unlocked(&conn, all_tiers_unlocked);

  // Update enabled tiers
  let mut enabled_tiers = Vec::new();
  if form.tier_1.is_some() {
    enabled_tiers.push(1);
  }
  if form.tier_2.is_some() {
    enabled_tiers.push(2);
  }
  if form.tier_3.is_some() {
    enabled_tiers.push(3);
  }
  if form.tier_4.is_some() {
    enabled_tiers.push(4);
  }

  // Ensure at least tier 1 is enabled
  if enabled_tiers.is_empty() {
    enabled_tiers.push(1);
  }

  let _ = db::set_enabled_tiers(&conn, &enabled_tiers);

  Redirect::to("/settings")
}
