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
        // Add more assets here as needed
        _ => path_str,
    })
}
