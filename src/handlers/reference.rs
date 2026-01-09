use askama::Template;
use axum::response::Html;

use super::NavContext;
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

pub async fn reference_index() -> Html<String> {
  let template = ReferenceIndexTemplate { nav: NavContext::public() };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_basics() -> Html<String> {
  let template = ReferenceBasicsTemplate { nav: NavContext::public() };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier1() -> Html<String> {
  let template = ReferenceTier1Template { nav: NavContext::public() };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier2() -> Html<String> {
  let template = ReferenceTier2Template { nav: NavContext::public() };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier3() -> Html<String> {
  let template = ReferenceTier3Template { nav: NavContext::public() };
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier4() -> Html<String> {
  let template = ReferenceTier4Template { nav: NavContext::public() };
  Html(template.render().unwrap_or_default())
}
