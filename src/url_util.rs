//! URL utilities -- parsing, resolution, and normalization.
//!
//! Handles relative URL resolution, search engine queries, and URL validation.

use url::Url;

/// Resolve a possibly-relative URL against a base URL.
///
/// If the input is already absolute, it's returned as-is.
/// If relative, it's resolved against the base.
pub fn resolve_url(base: &str, href: &str) -> String {
    // Handle special protocols.
    if href.starts_with("about:")
        || href.starts_with("data:")
        || href.starts_with("javascript:")
        || href.starts_with("mailto:")
    {
        return href.to_string();
    }

    // If it's already an absolute URL, return it.
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }

    // Protocol-relative URL.
    if href.starts_with("//") {
        let scheme = base.split("://").next().unwrap_or("https");
        return format!("{scheme}:{href}");
    }

    // Resolve relative URL against base.
    match Url::parse(base) {
        Ok(base_url) => match base_url.join(href) {
            Ok(resolved) => resolved.to_string(),
            Err(_) => href.to_string(),
        },
        Err(_) => href.to_string(),
    }
}

/// Normalize a user-entered URL or search query.
///
/// - If it looks like a URL (has a dot and no spaces), add https://
/// - If it's already a full URL, return as-is
/// - Otherwise, treat it as a search query
pub fn normalize_input(input: &str, search_engine: &str) -> String {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return "about:blank".to_string();
    }

    // Already a full URL.
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("about:")
        || trimmed.starts_with("file://")
    {
        return trimmed.to_string();
    }

    // Looks like a domain (has dots, no spaces, or is localhost).
    if (trimmed.contains('.') && !trimmed.contains(' '))
        || trimmed.starts_with("localhost")
        || trimmed.contains("://")
    {
        return format!("https://{trimmed}");
    }

    // Treat as a search query.
    search_engine.replace("%s", &url_encode(trimmed))
}

/// URL-encode a string for use in query parameters.
fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}

/// Extract the domain from a URL.
#[must_use]
pub fn extract_domain(url: &str) -> String {
    match Url::parse(url) {
        Ok(parsed) => parsed.host_str().unwrap_or("").to_string(),
        Err(_) => {
            // Fallback: manual extraction.
            url.split("://")
                .nth(1)
                .unwrap_or(url)
                .split('/')
                .next()
                .unwrap_or("")
                .split(':')
                .next()
                .unwrap_or("")
                .to_string()
        }
    }
}

/// Check if a URL is HTTPS.
#[must_use]
pub fn is_https(url: &str) -> bool {
    url.starts_with("https://")
}

/// Check if a URL is a special internal page.
#[must_use]
pub fn is_internal(url: &str) -> bool {
    url.starts_with("about:")
}

/// Get a display-friendly version of a URL (truncated for UI).
#[must_use]
pub fn display_url(url: &str, max_len: usize) -> String {
    let cleaned = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);

    let cleaned = cleaned.strip_suffix('/').unwrap_or(cleaned);

    if cleaned.len() <= max_len {
        cleaned.to_string()
    } else {
        format!("{}...", &cleaned[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_absolute_url() {
        assert_eq!(
            resolve_url("https://base.com", "https://other.com/page"),
            "https://other.com/page"
        );
    }

    #[test]
    fn resolve_relative_path() {
        assert_eq!(
            resolve_url("https://example.com/page", "/about"),
            "https://example.com/about"
        );
    }

    #[test]
    fn resolve_relative_file() {
        assert_eq!(
            resolve_url("https://example.com/dir/page", "other.html"),
            "https://example.com/dir/other.html"
        );
    }

    #[test]
    fn resolve_protocol_relative() {
        assert_eq!(
            resolve_url("https://base.com", "//cdn.example.com/file.js"),
            "https://cdn.example.com/file.js"
        );
    }

    #[test]
    fn normalize_full_url() {
        assert_eq!(
            normalize_input("https://example.com", "https://google.com/search?q=%s"),
            "https://example.com"
        );
    }

    #[test]
    fn normalize_domain() {
        assert_eq!(
            normalize_input("example.com", "https://google.com/search?q=%s"),
            "https://example.com"
        );
    }

    #[test]
    fn normalize_search_query() {
        let result = normalize_input("rust programming", "https://google.com/search?q=%s");
        assert!(result.starts_with("https://google.com/search?q="));
        assert!(result.contains("rust"));
    }

    #[test]
    fn normalize_empty() {
        assert_eq!(
            normalize_input("", "https://google.com/search?q=%s"),
            "about:blank"
        );
    }

    #[test]
    fn extract_domain_basic() {
        assert_eq!(extract_domain("https://example.com/path"), "example.com");
        assert_eq!(
            extract_domain("https://sub.example.com:8080/"),
            "sub.example.com"
        );
    }

    #[test]
    fn is_https_check() {
        assert!(is_https("https://example.com"));
        assert!(!is_https("http://example.com"));
    }

    #[test]
    fn display_url_truncation() {
        assert_eq!(display_url("https://example.com", 30), "example.com");
        assert_eq!(
            display_url("https://example.com/very/long/path/that/exceeds", 20),
            "example.com/very/..."
        );
    }
}
