//! Askama template filters for asset management

// Include compile-time generated asset hashes
include!(concat!(env!("OUT_DIR"), "/asset_hashes.rs"));

/// Append cache-busting hash to static asset URLs.
///
/// Usage in templates:
/// ```html
/// <script src="{{ "/static/js/card-interactions.js"|asset_url }}"></script>
/// ```
#[askama::filter_fn]
pub fn asset_url(path: impl std::fmt::Display, _: &dyn askama::Values) -> askama::Result<String> {
    let path_str = path.to_string();
    Ok(match path_str.as_str() {
        "/static/js/card-interactions.js" => {
            format!("{}?v={}", path_str, CARD_INTERACTIONS_JS_HASH)
        }
        "/static/css/styles.css" => {
            format!("{}?v={}", path_str, STYLES_CSS_HASH)
        }
        _ => path_str,
    })
}

/// Escape a string for use inside JavaScript string literals.
/// This escapes backslashes, quotes, and newlines to prevent injection.
///
/// Usage in templates:
/// ```html
/// <script>var x = "{{ user_input|js_escape }}";</script>
/// ```
#[askama::filter_fn]
pub fn js_escape(s: impl std::fmt::Display, _: &dyn askama::Values) -> askama::Result<String> {
    let input = s.to_string();
    let mut result = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\'' => result.push_str("\\'"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '<' => result.push_str("\\x3c"),  // Prevent </script> injection
            '>' => result.push_str("\\x3e"),
            '&' => result.push_str("\\x26"),
            c => result.push(c),
        }
    }
    Ok(result)
}
