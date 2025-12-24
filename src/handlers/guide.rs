use askama::Template;
use axum::response::Html;

#[derive(Template)]
#[template(path = "guide.html")]
pub struct GuideTemplate {}

pub async fn guide() -> Html<String> {
  let template = GuideTemplate {};
  Html(template.render().unwrap_or_default())
}
