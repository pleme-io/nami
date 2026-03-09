//! Navigation history -- back/forward navigation and persistent history.
//!
//! Maintains both session navigation history (back/forward) and persistent
//! browsing history (timestamped entries saved to disk).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single entry in the browsing history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The URL that was visited.
    pub url: String,
    /// The page title at time of visit.
    pub title: String,
    /// Unix timestamp of when the page was visited.
    pub timestamp: u64,
}

/// Persistent browsing history stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowsingHistory {
    entries: Vec<HistoryEntry>,
    #[serde(skip)]
    path: PathBuf,
    #[serde(skip)]
    max_entries: usize,
}

impl BrowsingHistory {
    /// Create a new browsing history, loading from disk if available.
    #[must_use]
    pub fn load(path: &PathBuf) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(data) => match serde_json::from_str::<BrowsingHistory>(&data) {
                    Ok(mut hist) => {
                        hist.path = path.clone();
                        hist.max_entries = 10_000;
                        return hist;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to parse history file");
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "failed to read history file");
                }
            }
        }

        Self {
            entries: Vec::new(),
            path: path.clone(),
            max_entries: 10_000,
        }
    }

    /// Add a URL visit to the history.
    pub fn add(&mut self, url: &str, title: &str) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.entries.push(HistoryEntry {
            url: url.to_string(),
            title: title.to_string(),
            timestamp,
        });

        // Trim if over max.
        if self.entries.len() > self.max_entries {
            let drain_count = self.entries.len() - self.max_entries;
            self.entries.drain(..drain_count);
        }
    }

    /// Save history to disk.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.path, data)?;
        Ok(())
    }

    /// Get all history entries, most recent first.
    #[must_use]
    pub fn entries(&self) -> Vec<&HistoryEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        sorted
    }

    /// Search history entries by URL or title substring.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.url.to_lowercase().contains(&query_lower)
                    || e.title.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get the total number of history entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Session navigation stack for back/forward.
#[derive(Debug)]
pub struct NavigationStack {
    /// Pages behind the current position (for "back").
    back: Vec<String>,
    /// Current URL.
    current: Option<String>,
    /// Pages ahead of the current position (for "forward").
    forward: Vec<String>,
}

impl NavigationStack {
    /// Create a new empty navigation stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            back: Vec::new(),
            current: None,
            forward: Vec::new(),
        }
    }

    /// Navigate to a new URL (clears forward history).
    pub fn navigate(&mut self, url: &str) {
        if let Some(current) = self.current.take() {
            self.back.push(current);
        }
        self.current = Some(url.to_string());
        self.forward.clear();
    }

    /// Go back one page. Returns the URL to navigate to, if any.
    pub fn go_back(&mut self) -> Option<String> {
        if let Some(prev) = self.back.pop() {
            if let Some(current) = self.current.take() {
                self.forward.push(current);
            }
            self.current = Some(prev.clone());
            Some(prev)
        } else {
            None
        }
    }

    /// Go forward one page. Returns the URL to navigate to, if any.
    pub fn go_forward(&mut self) -> Option<String> {
        if let Some(next) = self.forward.pop() {
            if let Some(current) = self.current.take() {
                self.back.push(current);
            }
            self.current = Some(next.clone());
            Some(next)
        } else {
            None
        }
    }

    /// Get the current URL.
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Check if we can go back.
    #[must_use]
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    /// Check if we can go forward.
    #[must_use]
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

impl Default for NavigationStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_stack_basic() {
        let mut stack = NavigationStack::new();
        assert!(stack.current().is_none());

        stack.navigate("https://a.com");
        assert_eq!(stack.current(), Some("https://a.com"));

        stack.navigate("https://b.com");
        assert_eq!(stack.current(), Some("https://b.com"));

        let back = stack.go_back();
        assert_eq!(back.as_deref(), Some("https://a.com"));
        assert_eq!(stack.current(), Some("https://a.com"));

        let fwd = stack.go_forward();
        assert_eq!(fwd.as_deref(), Some("https://b.com"));
    }

    #[test]
    fn navigation_clears_forward() {
        let mut stack = NavigationStack::new();
        stack.navigate("https://a.com");
        stack.navigate("https://b.com");
        stack.navigate("https://c.com");
        stack.go_back();
        stack.go_back();

        // Now at a.com. Navigate to d.com should clear forward (b, c).
        stack.navigate("https://d.com");
        assert!(!stack.can_go_forward());
        assert!(stack.can_go_back());
    }

    #[test]
    fn navigation_back_at_start() {
        let mut stack = NavigationStack::new();
        stack.navigate("https://a.com");
        assert!(stack.go_back().is_none());
    }

    #[test]
    fn navigation_forward_at_end() {
        let mut stack = NavigationStack::new();
        stack.navigate("https://a.com");
        assert!(stack.go_forward().is_none());
    }

    #[test]
    fn browsing_history_add_and_search() {
        let mut hist = BrowsingHistory {
            entries: Vec::new(),
            path: PathBuf::from("/tmp/nami-test-history.json"),
            max_entries: 100,
        };

        hist.add("https://example.com", "Example");
        hist.add("https://rust-lang.org", "Rust Lang");

        assert_eq!(hist.len(), 2);
        assert!(!hist.is_empty());

        let results = hist.search("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://rust-lang.org");
    }

    #[test]
    fn browsing_history_max_entries() {
        let mut hist = BrowsingHistory {
            entries: Vec::new(),
            path: PathBuf::from("/tmp/nami-test-history2.json"),
            max_entries: 5,
        };

        for i in 0..10 {
            hist.add(&format!("https://page{i}.com"), &format!("Page {i}"));
        }

        assert_eq!(hist.len(), 5);
    }

    #[test]
    fn browsing_history_clear() {
        let mut hist = BrowsingHistory {
            entries: Vec::new(),
            path: PathBuf::from("/tmp/nami-test-history3.json"),
            max_entries: 100,
        };
        hist.add("https://example.com", "Example");
        hist.clear();
        assert!(hist.is_empty());
    }
}
