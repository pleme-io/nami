//! Content blocking -- tracker and ad blocking.
//!
//! EasyList-compatible filter engine with domain blocking,
//! pattern matching, and exception rules.

use crate::config::ContentBlockingConfig;

/// A content blocker that checks URLs against filter rules.
#[derive(Debug)]
pub struct ContentBlocker {
    /// Domain block rules (e.g., "doubleclick.net").
    blocked_domains: Vec<String>,
    /// Pattern block rules (substring match).
    blocked_patterns: Vec<String>,
    /// Exception rules that override blocks.
    exception_domains: Vec<String>,
    /// Whether blocking is enabled.
    enabled: bool,
    /// Statistics.
    stats: BlockStats,
}

/// Blocking statistics for the current session.
#[derive(Debug, Default, Clone)]
pub struct BlockStats {
    /// Total requests checked.
    pub checked: u64,
    /// Total requests blocked.
    pub blocked: u64,
    /// Total requests allowed.
    pub allowed: u64,
}

/// Default tracker domains to block.
const DEFAULT_TRACKERS: &[&str] = &[
    "doubleclick.net",
    "google-analytics.com",
    "googletagmanager.com",
    "googlesyndication.com",
    "facebook.net",
    "connect.facebook.net",
    "analytics.google.com",
    "pixel.facebook.com",
    "bat.bing.com",
    "scorecardresearch.com",
    "quantserve.com",
    "adservice.google.com",
    "pagead2.googlesyndication.com",
    "amazon-adsystem.com",
    "ads-twitter.com",
    "ads.linkedin.com",
    "hotjar.com",
    "clarity.ms",
    "mixpanel.com",
    "segment.com",
    "amplitude.com",
    "newrelic.com",
    "nr-data.net",
    "bugsnag.com",
    "sentry.io",
];

impl ContentBlocker {
    /// Create a new content blocker from config.
    #[must_use]
    pub fn new(config: &ContentBlockingConfig) -> Self {
        let mut blocked_domains = config.blocked_domains.clone();

        if config.block_trackers {
            for &domain in DEFAULT_TRACKERS {
                if !blocked_domains.contains(&domain.to_string()) {
                    blocked_domains.push(domain.to_string());
                }
            }
        }

        Self {
            blocked_domains,
            blocked_patterns: Vec::new(),
            exception_domains: Vec::new(),
            enabled: config.enabled,
            stats: BlockStats::default(),
        }
    }

    /// Load filter rules from an EasyList-format file.
    pub fn load_filter_list(&mut self, content: &str) {
        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines.
            if line.is_empty() || line.starts_with('!') || line.starts_with('[') {
                continue;
            }

            // Exception rules: @@||domain.com^
            if let Some(exception) = line.strip_prefix("@@||") {
                let domain = exception.trim_end_matches('^');
                self.exception_domains.push(domain.to_string());
                continue;
            }

            // Domain rules: ||domain.com^
            if let Some(domain_rule) = line.strip_prefix("||") {
                let domain = domain_rule.trim_end_matches('^');
                if !domain.is_empty() {
                    self.blocked_domains.push(domain.to_string());
                }
                continue;
            }

            // Pattern rules (substring match).
            if !line.contains('$') && !line.contains('#') {
                self.blocked_patterns.push(line.to_string());
            }
        }
    }

    /// Check if a URL should be blocked.
    ///
    /// Returns `true` if the URL should be blocked.
    pub fn should_block(&mut self, url: &str) -> bool {
        if !self.enabled {
            return false;
        }

        self.stats.checked += 1;

        // Extract domain from URL.
        let domain = extract_domain(url);

        // Check exception rules first.
        if self.exception_domains.iter().any(|d| domain_matches(&domain, d)) {
            self.stats.allowed += 1;
            return false;
        }

        // Check domain blocks.
        if self.blocked_domains.iter().any(|d| domain_matches(&domain, d)) {
            self.stats.blocked += 1;
            tracing::debug!(url, "blocked by domain rule");
            return true;
        }

        // Check pattern blocks.
        if self.blocked_patterns.iter().any(|p| url.contains(p.as_str())) {
            self.stats.blocked += 1;
            tracing::debug!(url, "blocked by pattern rule");
            return true;
        }

        self.stats.allowed += 1;
        false
    }

    /// Add a domain to the block list.
    pub fn block_domain(&mut self, domain: &str) {
        if !self.blocked_domains.contains(&domain.to_string()) {
            self.blocked_domains.push(domain.to_string());
        }
    }

    /// Remove a domain from the block list.
    pub fn unblock_domain(&mut self, domain: &str) {
        self.blocked_domains.retain(|d| d != domain);
    }

    /// Get current blocking statistics.
    #[must_use]
    pub fn stats(&self) -> &BlockStats {
        &self.stats
    }

    /// Reset statistics.
    pub fn reset_stats(&mut self) {
        self.stats = BlockStats::default();
    }

    /// Enable or disable blocking.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if blocking is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the number of blocked domains.
    #[must_use]
    pub fn blocked_domain_count(&self) -> usize {
        self.blocked_domains.len()
    }
}

/// Extract the domain from a URL string.
fn extract_domain(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_lowercase()
}

/// Check if a domain matches a rule domain (with subdomain matching).
fn domain_matches(actual: &str, rule: &str) -> bool {
    actual == rule || actual.ends_with(&format!(".{rule}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ContentBlockingConfig {
        ContentBlockingConfig {
            enabled: true,
            block_trackers: true,
            block_ads: false,
            filter_lists: Vec::new(),
            blocked_domains: Vec::new(),
        }
    }

    #[test]
    fn blocks_tracker_domains() {
        let mut blocker = ContentBlocker::new(&test_config());
        assert!(blocker.should_block("https://google-analytics.com/collect"));
        assert!(blocker.should_block("https://www.google-analytics.com/analytics.js"));
    }

    #[test]
    fn allows_normal_domains() {
        let mut blocker = ContentBlocker::new(&test_config());
        assert!(!blocker.should_block("https://example.com/page"));
        assert!(!blocker.should_block("https://rust-lang.org/"));
    }

    #[test]
    fn disabled_allows_everything() {
        let mut config = test_config();
        config.enabled = false;
        let mut blocker = ContentBlocker::new(&config);
        assert!(!blocker.should_block("https://google-analytics.com/collect"));
    }

    #[test]
    fn custom_domain_blocking() {
        let mut blocker = ContentBlocker::new(&test_config());
        blocker.block_domain("evil.com");
        assert!(blocker.should_block("https://evil.com/track"));
        assert!(blocker.should_block("https://sub.evil.com/track"));

        blocker.unblock_domain("evil.com");
        assert!(!blocker.should_block("https://evil.com/track"));
    }

    #[test]
    fn stats_tracking() {
        let mut blocker = ContentBlocker::new(&test_config());
        blocker.should_block("https://example.com/");
        blocker.should_block("https://google-analytics.com/");
        blocker.should_block("https://rust-lang.org/");

        let stats = blocker.stats();
        assert_eq!(stats.checked, 3);
        assert_eq!(stats.blocked, 1);
        assert_eq!(stats.allowed, 2);
    }

    #[test]
    fn exception_rules() {
        let mut blocker = ContentBlocker::new(&test_config());
        blocker.exception_domains.push("google-analytics.com".to_string());
        assert!(!blocker.should_block("https://google-analytics.com/collect"));
    }

    #[test]
    fn extract_domain_from_urls() {
        assert_eq!(extract_domain("https://example.com/path"), "example.com");
        assert_eq!(extract_domain("http://sub.example.com:8080/"), "sub.example.com");
        assert_eq!(extract_domain("https://example.com"), "example.com");
    }

    #[test]
    fn load_filter_list() {
        let mut blocker = ContentBlocker::new(&test_config());
        let filter = r#"! Comment
[Adblock Plus 2.0]
||evil-tracker.com^
||bad-ads.net^
@@||allowed-tracker.com^
/tracking.js
"#;
        blocker.load_filter_list(filter);
        assert!(blocker.should_block("https://evil-tracker.com/pixel"));
        assert!(blocker.should_block("https://bad-ads.net/ad.js"));
    }
}
