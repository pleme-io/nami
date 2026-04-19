//! Tab management -- multiple browser tabs.
//!
//! Each tab has its own page state, history stack, scroll position, and title.

use crate::css::Stylesheet;
use crate::dom::Document;
use crate::history::NavigationStack;
use crate::layout::LayoutTree;

/// State of a single tab's page.
#[derive(Debug, Clone)]
pub enum PageState {
    /// Tab has no content yet.
    Blank,
    /// Page is being loaded.
    Loading { url: String },
    /// Page is fully loaded.
    Loaded {
        url: String,
        title: String,
        html: String,
    },
    /// Page load failed.
    Error { url: String, error: String },
}

/// A single browser tab.
pub struct Tab {
    /// Current page state.
    pub page_state: PageState,
    /// Navigation history stack (back/forward).
    pub nav_stack: NavigationStack,
    /// Vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// Horizontal scroll offset in pixels.
    pub scroll_x: f32,
    /// The parsed document (if loaded).
    pub document: Option<Document>,
    /// Parsed stylesheets for the current page.
    pub stylesheets: Vec<Stylesheet>,
    /// Computed layout tree (if computed).
    pub layout: Option<LayoutTree>,
    /// Search matches on the current page.
    pub search_matches: Vec<SearchMatch>,
    /// Current search match index.
    pub current_match: Option<usize>,
    /// Whether this tab is pinned.
    pub pinned: bool,
}

/// A search match location.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// The matched text.
    pub text: String,
    /// Position in the page content.
    pub offset: usize,
    /// Y coordinate in the layout.
    pub y: f32,
}

impl Tab {
    /// Create a new blank tab.
    #[must_use]
    pub fn new() -> Self {
        Self {
            page_state: PageState::Blank,
            nav_stack: NavigationStack::new(),
            scroll_y: 0.0,
            scroll_x: 0.0,
            document: None,
            stylesheets: Vec::new(),
            layout: None,
            search_matches: Vec::new(),
            current_match: None,
            pinned: false,
        }
    }

    /// Create a new tab loading a URL.
    #[must_use]
    pub fn with_url(url: &str) -> Self {
        let mut tab = Self::new();
        tab.page_state = PageState::Loading {
            url: url.to_string(),
        };
        tab.nav_stack.navigate(url);
        tab
    }

    /// Get the tab title (for the tab bar).
    #[must_use]
    pub fn title(&self) -> &str {
        match &self.page_state {
            PageState::Blank => "New Tab",
            PageState::Loading { url } => url.as_str(),
            PageState::Loaded { title, .. } => {
                if title.is_empty() {
                    "Untitled"
                } else {
                    title.as_str()
                }
            }
            PageState::Error { url, .. } => url.as_str(),
        }
    }

    /// Get the current URL.
    #[must_use]
    pub fn url(&self) -> &str {
        match &self.page_state {
            PageState::Blank => "about:blank",
            PageState::Loading { url }
            | PageState::Loaded { url, .. }
            | PageState::Error { url, .. } => url.as_str(),
        }
    }

    /// Set the page as loaded with HTML content.
    pub fn set_loaded(&mut self, url: &str, html: String) {
        let doc = Document::parse(&html);
        let title = doc.title().unwrap_or("Untitled").to_string();

        // Extract inline stylesheets.
        let mut stylesheets = Vec::new();
        for css_text in doc.inline_styles() {
            stylesheets.push(Stylesheet::parse(&css_text));
        }

        self.page_state = PageState::Loaded {
            url: url.to_string(),
            title,
            html,
        };
        self.document = Some(doc);
        self.stylesheets = stylesheets;
        self.scroll_y = 0.0;
        self.scroll_x = 0.0;
        self.search_matches.clear();
        self.current_match = None;
    }

    /// Set the page as errored.
    pub fn set_error(&mut self, url: &str, error: String) {
        self.page_state = PageState::Error {
            url: url.to_string(),
            error,
        };
    }

    /// Recompute layout for the current document.
    pub fn compute_layout(&mut self, viewport_width: f32) {
        if let Some(ref doc) = self.document {
            let layout = LayoutTree::compute(doc, &self.stylesheets, viewport_width);
            self.layout = Some(layout);
        }
    }

    /// Get the total content height.
    #[must_use]
    pub fn content_height(&self) -> f32 {
        self.layout
            .as_ref()
            .map(|l| l.content_height())
            .unwrap_or(0.0)
    }

    /// Scroll down by a given amount, clamped to content bounds.
    pub fn scroll_down(&mut self, amount: f32, viewport_height: f32) {
        let max_scroll = (self.content_height() - viewport_height).max(0.0);
        self.scroll_y = (self.scroll_y + amount).min(max_scroll);
    }

    /// Scroll up by a given amount, clamped to zero.
    pub fn scroll_up(&mut self, amount: f32) {
        self.scroll_y = (self.scroll_y - amount).max(0.0);
    }

    /// Search for text on the current page.
    pub fn search(&mut self, query: &str) {
        self.search_matches.clear();
        self.current_match = None;

        if let Some(ref doc) = self.document {
            let text = doc.text_content();
            let query_lower = query.to_lowercase();
            let text_lower = text.to_lowercase();

            let mut offset = 0;
            while let Some(pos) = text_lower[offset..].find(&query_lower) {
                let abs_pos = offset + pos;
                self.search_matches.push(SearchMatch {
                    text: text[abs_pos..abs_pos + query.len()].to_string(),
                    offset: abs_pos,
                    y: 0.0, // Would need layout info for exact position.
                });
                offset = abs_pos + query.len();
            }

            if !self.search_matches.is_empty() {
                self.current_match = Some(0);
            }
        }
    }

    /// Move to next search match.
    pub fn next_match(&mut self) {
        if let Some(ref mut idx) = self.current_match {
            if *idx + 1 < self.search_matches.len() {
                *idx += 1;
            } else {
                *idx = 0; // Wrap around.
            }
        }
    }

    /// Move to previous search match.
    pub fn prev_match(&mut self) {
        if let Some(ref mut idx) = self.current_match {
            if *idx > 0 {
                *idx -= 1;
            } else {
                *idx = self.search_matches.len().saturating_sub(1);
            }
        }
    }

    /// Get search match count string (e.g., "3/15").
    #[must_use]
    pub fn match_count_str(&self) -> String {
        if self.search_matches.is_empty() {
            "No matches".to_string()
        } else if let Some(idx) = self.current_match {
            format!("{}/{}", idx + 1, self.search_matches.len())
        } else {
            format!("{} matches", self.search_matches.len())
        }
    }

    /// Check if the page is currently loading.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        matches!(self.page_state, PageState::Loading { .. })
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for multiple browser tabs.
pub struct TabManager {
    /// All open tabs.
    tabs: Vec<Tab>,
    /// Index of the currently active tab.
    active: usize,
}

impl TabManager {
    /// Create a new tab manager with one blank tab.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tabs: vec![Tab::new()],
            active: 0,
        }
    }

    /// Create a tab manager with an initial URL.
    #[must_use]
    pub fn with_url(url: &str) -> Self {
        Self {
            tabs: vec![Tab::with_url(url)],
            active: 0,
        }
    }

    /// Get the active tab.
    #[must_use]
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// Get the active tab mutably.
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// Get the active tab index.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Get the number of open tabs.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Get all tab titles and their active status.
    #[must_use]
    pub fn tab_titles(&self) -> Vec<(&str, bool)> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| (tab.title(), i == self.active))
            .collect()
    }

    /// Open a new tab (optionally with a URL).
    pub fn new_tab(&mut self, url: Option<&str>) -> usize {
        let tab = match url {
            Some(u) => Tab::with_url(u),
            None => Tab::new(),
        };
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.active
    }

    /// Close the active tab. If it's the last tab, creates a new blank one.
    pub fn close_tab(&mut self) {
        if self.tabs[self.active].pinned {
            return;
        }

        if self.tabs.len() == 1 {
            self.tabs[0] = Tab::new();
            return;
        }

        self.tabs.remove(self.active);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
    }

    /// Switch to the next tab.
    pub fn next_tab(&mut self) {
        self.active = (self.active + 1) % self.tabs.len();
    }

    /// Switch to the previous tab.
    pub fn prev_tab(&mut self) {
        self.active = if self.active == 0 {
            self.tabs.len() - 1
        } else {
            self.active - 1
        };
    }

    /// Switch to a specific tab by index.
    pub fn goto_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    /// Get a reference to a specific tab.
    #[must_use]
    pub fn get_tab(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }

    /// Get a mutable reference to a specific tab.
    pub fn get_tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tab_is_blank() {
        let tab = Tab::new();
        assert!(matches!(tab.page_state, PageState::Blank));
        assert_eq!(tab.title(), "New Tab");
        assert_eq!(tab.url(), "about:blank");
    }

    #[test]
    fn tab_with_url() {
        let tab = Tab::with_url("https://example.com");
        assert!(tab.is_loading());
        assert_eq!(tab.url(), "https://example.com");
    }

    #[test]
    fn tab_set_loaded() {
        let mut tab = Tab::new();
        tab.set_loaded(
            "https://example.com",
            "<html><head><title>Test</title></head><body>Hello</body></html>".to_string(),
        );
        assert_eq!(tab.title(), "Test");
        assert!(tab.document.is_some());
    }

    #[test]
    fn tab_scroll() {
        let mut tab = Tab::new();
        tab.set_loaded(
            "https://example.com",
            "<html><body><p>Content</p></body></html>".to_string(),
        );
        tab.compute_layout(800.0);

        tab.scroll_down(100.0, 600.0);
        // Scroll position depends on content height.
        assert!(tab.scroll_y >= 0.0);

        tab.scroll_up(50.0);
        assert!(tab.scroll_y >= 0.0);
    }

    #[test]
    fn tab_search() {
        let mut tab = Tab::new();
        tab.set_loaded(
            "https://example.com",
            "<html><body><p>Hello World. Hello again.</p></body></html>".to_string(),
        );
        tab.search("Hello");
        assert_eq!(tab.search_matches.len(), 2);
        assert_eq!(tab.current_match, Some(0));

        tab.next_match();
        assert_eq!(tab.current_match, Some(1));

        tab.next_match(); // Wraps around.
        assert_eq!(tab.current_match, Some(0));
    }

    #[test]
    fn tab_manager_basic() {
        let mut mgr = TabManager::new();
        assert_eq!(mgr.tab_count(), 1);

        mgr.new_tab(Some("https://example.com"));
        assert_eq!(mgr.tab_count(), 2);
        assert_eq!(mgr.active_index(), 1);

        mgr.prev_tab();
        assert_eq!(mgr.active_index(), 0);

        mgr.next_tab();
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn tab_manager_close() {
        let mut mgr = TabManager::new();
        mgr.new_tab(Some("https://a.com"));
        mgr.new_tab(Some("https://b.com"));
        assert_eq!(mgr.tab_count(), 3);

        mgr.close_tab();
        assert_eq!(mgr.tab_count(), 2);
    }

    #[test]
    fn tab_manager_close_last() {
        let mut mgr = TabManager::new();
        mgr.close_tab(); // Closing last tab should create a new blank one.
        assert_eq!(mgr.tab_count(), 1);
        assert!(matches!(mgr.active_tab().page_state, PageState::Blank));
    }

    #[test]
    fn tab_manager_titles() {
        let mut mgr = TabManager::new();
        mgr.new_tab(None);
        let titles = mgr.tab_titles();
        assert_eq!(titles.len(), 2);
        assert!(!titles[0].1); // First tab is not active.
        assert!(titles[1].1); // Second tab is active.
    }

    #[test]
    fn tab_match_count_str() {
        let mut tab = Tab::new();
        assert_eq!(tab.match_count_str(), "No matches");

        tab.set_loaded(
            "https://example.com",
            "<html><body>test test test</body></html>".to_string(),
        );
        tab.search("test");
        assert!(tab.match_count_str().contains('/'));
    }
}
