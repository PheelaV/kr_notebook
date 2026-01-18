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
        "/static/js/sw-register.js" => {
            format!("{}?v={}", path_str, SW_REGISTER_JS_HASH)
        }
        "/static/sw.js" => {
            format!("{}?v={}", path_str, SW_JS_HASH)
        }
        "/static/js/offline-storage.js" => {
            format!("{}?v={}", path_str, OFFLINE_STORAGE_JS_HASH)
        }
        "/static/js/offline-sync.js" => {
            format!("{}?v={}", path_str, OFFLINE_SYNC_JS_HASH)
        }
        "/static/js/offline-study.js" => {
            format!("{}?v={}", path_str, OFFLINE_STUDY_JS_HASH)
        }
        "/static/js/vocabulary-search.js" => {
            format!("{}?v={}", path_str, VOCABULARY_SEARCH_JS_HASH)
        }
        _ => path_str,
    })
}

/// Core JS escaping logic - escapes special characters for safe use in JS strings.
/// This is the testable core of the filter.
pub fn escape_js_string(input: &str) -> String {
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
    result
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
    Ok(escape_js_string(&s.to_string()))
}

// ============================================================================
// Answer Display Formatting
// ============================================================================

/// Core logic for formatting answer grammar for display with visual markers.
///
/// Transforms grammar syntax to HTML with accessibility markers:
/// - `[a, b, c]` → `<span class="variant-marker" title="Acceptable variants">[a, b, c]</span>`
/// - `word(s)` (suffix) → `word<span class="variant-marker" title="Optional suffix">(s)</span>`
/// - `(info)` (space before) → `<span class="info-marker" title="Additional info">(info)</span>`
/// - `<context>` → `<span class="disambig-marker" title="Disambiguation">&lt;context&gt;</span>`
pub fn format_answer_display_core(answer: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = answer.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            // Variants: [a, b, c]
            '[' => {
                if let Some(end) = find_closing(&chars, i, '[', ']') {
                    let content: String = chars[i..=end].iter().collect();
                    result.push_str(&format!(
                        r#"<span class="variant-marker" title="Acceptable variants">{}</span>"#,
                        html_escape(&content)
                    ));
                    i = end + 1;
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            // Disambiguation: <context>
            '<' => {
                if let Some(end) = find_closing(&chars, i, '<', '>') {
                    let content: String = chars[i + 1..end].iter().collect();
                    result.push_str(&format!(
                        r#"<span class="disambig-marker" title="Disambiguation">&lt;{}&gt;</span>"#,
                        html_escape(&content)
                    ));
                    i = end + 1;
                } else {
                    result.push_str("&lt;");
                    i += 1;
                }
            }
            // Parentheses: either suffix or info
            '(' => {
                if let Some(end) = find_closing(&chars, i, '(', ')') {
                    let content: String = chars[i..=end].iter().collect();
                    let has_space_before = i > 0 && chars[i - 1] == ' ';

                    if has_space_before {
                        // Info marker (space before paren)
                        result.push_str(&format!(
                            r#"<span class="info-marker" title="Additional info">{}</span>"#,
                            html_escape(&content)
                        ));
                    } else if i > 0 {
                        // Suffix marker (no space before paren)
                        result.push_str(&format!(
                            r#"<span class="variant-marker" title="Optional suffix">{}</span>"#,
                            html_escape(&content)
                        ));
                    } else {
                        // At start of string, treat as info
                        result.push_str(&format!(
                            r#"<span class="info-marker" title="Additional info">{}</span>"#,
                            html_escape(&content)
                        ));
                    }
                    i = end + 1;
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            '>' => {
                result.push_str("&gt;");
                i += 1;
            }
            c => {
                result.push(c);
                i += 1;
            }
        }
    }

    result
}

/// Find closing bracket, handling nesting
fn find_closing(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (i, &ch) in chars.iter().enumerate().skip(start) {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Basic HTML escaping for user content within markers
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Format answer for display with visual markers.
/// This filter converts grammar syntax to HTML with accessibility indicators.
///
/// Usage in templates:
/// ```html
/// <span class="answer">{{ card.main_answer|format_answer_display|safe }}</span>
/// ```
#[askama::filter_fn]
pub fn format_answer_display(
    answer: impl std::fmt::Display,
    _: &dyn askama::Values,
) -> askama::Result<String> {
    Ok(format_answer_display_core(&answer.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{escape_js_string, format_answer_display_core};

    // Note: We cannot easily test asset_url() as it depends on compile-time
    // generated hashes. We focus on testing escape_js_string() which is critical
    // for security (XSS prevention).

    #[test]
    fn test_js_escape_empty() {
        assert_eq!(escape_js_string(""), "");
    }

    #[test]
    fn test_js_escape_plain_text() {
        assert_eq!(escape_js_string("hello world"), "hello world");
    }

    #[test]
    fn test_js_escape_backslash() {
        // Input: C:\path\file -> Output: C:\\path\\file
        let result = escape_js_string("C:\\path\\file");
        assert_eq!(result, "C:\\\\path\\\\file");
    }

    #[test]
    fn test_js_escape_double_quote() {
        // Input: say "hello" -> Output: say \"hello\"
        let result = escape_js_string("say \"hello\"");
        assert!(result.contains("\\\""));
    }

    #[test]
    fn test_js_escape_single_quote() {
        // Input: it's -> Output: it\'s
        let result = escape_js_string("it's");
        assert!(result.contains("\\'"));
    }

    #[test]
    fn test_js_escape_newline() {
        let result = escape_js_string("line1\nline2");
        assert!(result.contains("\\n"));
        assert!(!result.contains('\n'));
    }

    #[test]
    fn test_js_escape_carriage_return() {
        let result = escape_js_string("line1\rline2");
        assert!(result.contains("\\r"));
        assert!(!result.contains('\r'));
    }

    #[test]
    fn test_js_escape_tab() {
        let result = escape_js_string("col1\tcol2");
        assert!(result.contains("\\t"));
        assert!(!result.contains('\t'));
    }

    #[test]
    fn test_js_escape_less_than() {
        // < should become \x3c
        let result = escape_js_string("<tag>");
        assert!(!result.contains('<'));
        assert!(result.contains("\\x3c"));
    }

    #[test]
    fn test_js_escape_greater_than() {
        // > should become \x3e
        let result = escape_js_string("<tag>");
        assert!(!result.contains('>'));
        assert!(result.contains("\\x3e"));
    }

    #[test]
    fn test_js_escape_ampersand() {
        // & should become \x26
        let result = escape_js_string("a & b");
        assert!(!result.contains('&'));
        assert!(result.contains("\\x26"));
    }

    #[test]
    fn test_js_escape_korean_passthrough() {
        // Korean characters should pass through unchanged
        assert_eq!(escape_js_string("한글"), "한글");
        assert_eq!(escape_js_string("ㄱ ㄴ ㄷ"), "ㄱ ㄴ ㄷ");
    }

    #[test]
    fn test_js_escape_xss_script_tag() {
        let result = escape_js_string("<script>alert(1)</script>");
        // Should not contain literal < or >
        assert!(!result.contains('<'));
        assert!(!result.contains('>'));
        // Should contain escaped versions
        assert!(result.contains("\\x3c"));
        assert!(result.contains("\\x3e"));
    }

    #[test]
    fn test_js_escape_closing_script_tag() {
        // The dangerous </script> sequence should be escaped
        let result = escape_js_string("</script>");
        assert!(!result.contains('<'));
        assert!(!result.contains('>'));
    }

    #[test]
    fn test_js_escape_string_breakout_attempt() {
        // Attempt to break out of JS string context with quotes
        let result = escape_js_string("\"; alert(1); //");
        // Double quote should be escaped
        assert!(result.starts_with("\\\""));
    }

    #[test]
    fn test_js_escape_null_byte_passthrough() {
        // Null bytes pass through (not a security concern for JS strings)
        let result = escape_js_string("a\0b");
        assert_eq!(result, "a\0b");
    }

    #[test]
    fn test_js_escape_unicode_passthrough() {
        // Various unicode should pass through unchanged
        assert_eq!(escape_js_string("café"), "café");
        assert_eq!(escape_js_string("日本語"), "日本語");
        assert_eq!(escape_js_string("🎉"), "🎉");
    }

    #[test]
    fn test_js_escape_preserves_length_for_simple() {
        // Simple text should not change length
        let input = "hello";
        let result = escape_js_string(input);
        assert_eq!(result.len(), input.len());
    }

    #[test]
    fn test_js_escape_increases_length_for_special() {
        // Escaping should increase length
        let input = "\\";
        let result = escape_js_string(input);
        assert!(result.len() > input.len());
    }

    // ============================================================================
    // format_answer_display tests
    // ============================================================================

    #[test]
    fn test_format_plain_text() {
        let result = format_answer_display_core("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_format_variants() {
        let result = format_answer_display_core("to be [is, am, are]");
        assert!(result.contains("variant-marker"));
        assert!(result.contains("Acceptable variants"));
        assert!(result.contains("[is, am, are]"));
    }

    #[test]
    fn test_format_suffix() {
        let result = format_answer_display_core("eye(s)");
        assert!(result.contains("variant-marker"));
        assert!(result.contains("Optional suffix"));
        assert!(result.contains("(s)"));
        // Should start with "eye" followed by the span
        assert!(result.starts_with("eye<span"));
    }

    #[test]
    fn test_format_info_marker() {
        let result = format_answer_display_core("that (far)");
        assert!(result.contains("info-marker"));
        assert!(result.contains("Additional info"));
        assert!(result.contains("(far)"));
    }

    #[test]
    fn test_format_disambiguation() {
        let result = format_answer_display_core("that <far>");
        assert!(result.contains("disambig-marker"));
        assert!(result.contains("Disambiguation"));
        assert!(result.contains("&lt;far&gt;"));
    }

    #[test]
    fn test_format_korean_with_romanization() {
        let result = format_answer_display_core("소파 (so-pa)");
        assert!(result.contains("소파"));
        assert!(result.contains("info-marker"));
        assert!(result.contains("(so-pa)"));
    }

    #[test]
    fn test_format_complex() {
        // Multiple elements
        let result = format_answer_display_core("to be [is, am] (verb)");
        assert!(result.contains("variant-marker"));
        assert!(result.contains("info-marker"));
    }

    #[test]
    fn test_format_escapes_html() {
        // Make sure HTML in content is escaped
        let result = format_answer_display_core("[<script>]");
        assert!(result.contains("&lt;script&gt;"));
        assert!(!result.contains("<script>"));
    }
}
