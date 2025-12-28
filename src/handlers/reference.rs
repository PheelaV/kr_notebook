use askama::Template;
use axum::response::Html;

use crate::filters;

#[derive(Template)]
#[template(path = "reference/index.html")]
pub struct ReferenceIndexTemplate {}

#[derive(Template)]
#[template(path = "reference/basics.html")]
pub struct ReferenceBasicsTemplate {}

#[derive(Template)]
#[template(path = "reference/tier1.html")]
pub struct ReferenceTier1Template {}

#[derive(Template)]
#[template(path = "reference/tier2.html")]
pub struct ReferenceTier2Template {}

#[derive(Template)]
#[template(path = "reference/tier3.html")]
pub struct ReferenceTier3Template {}

#[derive(Template)]
#[template(path = "reference/tier4.html")]
pub struct ReferenceTier4Template {}

pub async fn reference_index() -> Html<String> {
  let template = ReferenceIndexTemplate {};
  Html(template.render().unwrap_or_default())
}

pub async fn reference_basics() -> Html<String> {
  let template = ReferenceBasicsTemplate {};
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier1() -> Html<String> {
  let template = ReferenceTier1Template {};
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier2() -> Html<String> {
  let template = ReferenceTier2Template {};
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier3() -> Html<String> {
  let template = ReferenceTier3Template {};
  Html(template.render().unwrap_or_default())
}

pub async fn reference_tier4() -> Html<String> {
  let template = ReferenceTier4Template {};
  Html(template.render().unwrap_or_default())
}
