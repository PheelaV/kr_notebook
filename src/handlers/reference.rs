use askama::Template;
use axum::response::Html;

use super::NavContext;
use crate::auth::OptionalAuth;
use crate::filters;

#[derive(Template)]
#[template(path = "reference/index.html")]
pub struct ReferenceIndexTemplate {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/basics.html")]
pub struct ReferenceBasicsTemplate {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/tier1.html")]
pub struct ReferenceTier1Template {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/tier2.html")]
pub struct ReferenceTier2Template {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/tier3.html")]
pub struct ReferenceTier3Template {
  pub nav: NavContext,
}

#[derive(Template)]
#[template(path = "reference/tier4.html")]
pub struct ReferenceTier4Template {
  pub nav: NavContext,
}

pub async fn reference_index(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceIndexTemplate { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_basics(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceBasicsTemplate { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier1(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceTier1Template { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier2(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceTier2Template { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier3(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceTier3Template { nav };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier4(OptionalAuth(auth): OptionalAuth) -> Html<String> {
  let nav = auth.map(|a| NavContext::from_auth(&a)).unwrap_or_default();
  let template = ReferenceTier4Template { nav };
  Html(template.render().unwrap_or_default())
}
