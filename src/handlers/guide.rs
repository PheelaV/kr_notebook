use askama::Template;
use axum::response::Html;

use super::NavContext;
use crate::filters;

/// TOC item for navigation
pub struct TocItem {
    pub id: String,
    pub short_label: String,
    pub full_label: String,
}

#[derive(Template)]
#[template(path = "guide.html")]
pub struct GuideTemplate {
    pub toc_items: Vec<TocItem>,
    pub toc_title: String,
    pub nav: NavContext,
}

pub async fn guide() -> Html<String> {
    let toc_items = vec![
        TocItem { id: "quick-start".into(), short_label: "Quick Start".into(), full_label: "Quick Start".into() },
        TocItem { id: "interactive".into(), short_label: "Interactive".into(), full_label: "Interactive Learning".into() },
        TocItem { id: "hints".into(), short_label: "Hints".into(), full_label: "Using Hints".into() },
        TocItem { id: "srs".into(), short_label: "SRS".into(), full_label: "Spaced Repetition".into() },
        TocItem { id: "scoring".into(), short_label: "Scoring".into(), full_label: "Answer Scoring".into() },
        TocItem { id: "tiers".into(), short_label: "Tiers".into(), full_label: "Learning Tiers".into() },
        TocItem { id: "accelerated".into(), short_label: "Accelerated".into(), full_label: "Accelerated Mode".into() },
        TocItem { id: "focus".into(), short_label: "Focus".into(), full_label: "Focus Mode".into() },
        TocItem { id: "retention".into(), short_label: "Retention".into(), full_label: "Retention Target".into() },
        TocItem { id: "problem-areas".into(), short_label: "Problems".into(), full_label: "Problem Areas".into() },
        TocItem { id: "tips".into(), short_label: "Tips".into(), full_label: "Tips for Success".into() },
        TocItem { id: "learned".into(), short_label: "Learned".into(), full_label: "What \"Learned\" Means".into() },
        TocItem { id: "practice".into(), short_label: "Practice".into(), full_label: "Practice Mode".into() },
        TocItem { id: "content-packs".into(), short_label: "Packs".into(), full_label: "Content Packs".into() },
        TocItem { id: "shortcuts".into(), short_label: "Shortcuts".into(), full_label: "Keyboard Shortcuts".into() },
    ];

    let template = GuideTemplate {
        toc_items,
        toc_title: "Contents".to_string(),
        nav: NavContext::public(),
    };
    Html(template.render().unwrap_or_default())
}
