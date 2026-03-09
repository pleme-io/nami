//! Bookmarks -- persistent bookmark storage with tags.
//!
//! Stores bookmarks as JSON, supports tagging, search, and import/export.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single bookmark entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    /// The bookmarked URL.
    pub url: String,
    /// The page title.
    pub title: String,
    /// Tags for categorisation.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unix timestamp when the bookmark was added.
    pub created_at: u64,
}

/// Persistent bookmark collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmarks {
    bookmarks: Vec<Bookmark>,
    #[serde(skip)]
    path: PathBuf,
}

impl Bookmarks {
    /// Load bookmarks from a file, or create empty collection.
    #[must_use]
    pub fn load(path: &PathBuf) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(data) => match serde_json::from_str::<Bookmarks>(&data) {
                    Ok(mut bm) => {
                        bm.path = path.clone();
                        return bm;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to parse bookmarks file");
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "failed to read bookmarks file");
                }
            }
        }

        Self {
            bookmarks: Vec::new(),
            path: path.clone(),
        }
    }

    /// Add a bookmark. Returns `true` if it was added, `false` if it already exists.
    pub fn add(&mut self, url: &str, title: &str, tags: Vec<String>) -> bool {
        if self.bookmarks.iter().any(|b| b.url == url) {
            return false;
        }

        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.bookmarks.push(Bookmark {
            url: url.to_string(),
            title: title.to_string(),
            tags,
            created_at,
        });

        true
    }

    /// Remove a bookmark by URL. Returns `true` if it was removed.
    pub fn remove(&mut self, url: &str) -> bool {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.url != url);
        self.bookmarks.len() < before
    }

    /// Check if a URL is bookmarked.
    #[must_use]
    pub fn is_bookmarked(&self, url: &str) -> bool {
        self.bookmarks.iter().any(|b| b.url == url)
    }

    /// Get all bookmarks, most recent first.
    #[must_use]
    pub fn all(&self) -> Vec<&Bookmark> {
        let mut sorted: Vec<_> = self.bookmarks.iter().collect();
        sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        sorted
    }

    /// Search bookmarks by URL, title, or tag.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&Bookmark> {
        let query_lower = query.to_lowercase();
        self.bookmarks
            .iter()
            .filter(|b| {
                b.url.to_lowercase().contains(&query_lower)
                    || b.title.to_lowercase().contains(&query_lower)
                    || b.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Get bookmarks by tag.
    #[must_use]
    pub fn by_tag(&self, tag: &str) -> Vec<&Bookmark> {
        let tag_lower = tag.to_lowercase();
        self.bookmarks
            .iter()
            .filter(|b| b.tags.iter().any(|t| t.to_lowercase() == tag_lower))
            .collect()
    }

    /// Get all unique tags.
    #[must_use]
    pub fn tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .bookmarks
            .iter()
            .flat_map(|b| b.tags.clone())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Save bookmarks to disk.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.path, data)?;
        Ok(())
    }

    /// Get total number of bookmarks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bookmarks.len()
    }

    /// Check if bookmarks collection is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bookmarks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bookmarks() -> Bookmarks {
        Bookmarks {
            bookmarks: Vec::new(),
            path: PathBuf::from("/tmp/nami-test-bookmarks.json"),
        }
    }

    #[test]
    fn add_and_check() {
        let mut bm = test_bookmarks();
        assert!(bm.add("https://example.com", "Example", vec![]));
        assert!(bm.is_bookmarked("https://example.com"));
        assert!(!bm.is_bookmarked("https://other.com"));
    }

    #[test]
    fn add_duplicate() {
        let mut bm = test_bookmarks();
        assert!(bm.add("https://example.com", "Example", vec![]));
        assert!(!bm.add("https://example.com", "Example 2", vec![]));
        assert_eq!(bm.len(), 1);
    }

    #[test]
    fn remove_bookmark() {
        let mut bm = test_bookmarks();
        bm.add("https://example.com", "Example", vec![]);
        assert!(bm.remove("https://example.com"));
        assert!(!bm.is_bookmarked("https://example.com"));
        assert!(!bm.remove("https://example.com"));
    }

    #[test]
    fn search_bookmarks() {
        let mut bm = test_bookmarks();
        bm.add("https://rust-lang.org", "Rust Language", vec!["programming".into()]);
        bm.add("https://example.com", "Example Site", vec!["reference".into()]);

        let results = bm.search("rust");
        assert_eq!(results.len(), 1);

        let results = bm.search("programming");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn tags_collection() {
        let mut bm = test_bookmarks();
        bm.add("https://a.com", "A", vec!["rust".into(), "code".into()]);
        bm.add("https://b.com", "B", vec!["code".into(), "docs".into()]);

        let tags = bm.tags();
        assert_eq!(tags, vec!["code", "docs", "rust"]);
    }

    #[test]
    fn by_tag() {
        let mut bm = test_bookmarks();
        bm.add("https://a.com", "A", vec!["rust".into()]);
        bm.add("https://b.com", "B", vec!["python".into()]);

        let rust = bm.by_tag("rust");
        assert_eq!(rust.len(), 1);
        assert_eq!(rust[0].url, "https://a.com");
    }
}
