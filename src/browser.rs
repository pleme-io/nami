//! Browser state -- orchestrates all browser components.
//!
//! The `Browser` struct owns tabs, history, bookmarks, content blocker,
//! and fetcher. It provides the high-level API for the UI to interact with.

use crate::bookmarks::Bookmarks;
use crate::config::NamiConfig;
use crate::content_blocking::ContentBlocker;
use crate::css::Stylesheet;
use crate::dom::Document;
use crate::fetch::Fetcher;
use crate::history::BrowsingHistory;
use crate::input::{BrowserAction, InputHandler, Mode};
use crate::render;
use crate::substrate::Substrate;
use crate::tabs::{PageState, TabManager};
use crate::transform::{AliasSet, TransformSet};
use crate::url_util;
use nami_core::store::StateStore;

/// The main browser state.
pub struct Browser {
    /// Tab manager.
    pub tabs: TabManager,
    /// Global browsing history.
    pub history: BrowsingHistory,
    /// Bookmarks.
    pub bookmarks: Bookmarks,
    /// Content blocker.
    pub content_blocker: ContentBlocker,
    /// Input handler (keyboard mode).
    pub input: InputHandler,
    /// Configuration.
    pub config: NamiConfig,
    /// HTTP fetcher.
    pub fetcher: Fetcher,
    /// Viewport width in characters.
    pub viewport_width: u32,
    /// Viewport height in lines.
    pub viewport_height: u32,
    /// Status message displayed at the bottom.
    pub status_message: Option<String>,
    /// Whether the browser should quit.
    pub should_quit: bool,
    /// Address bar content (when editing).
    pub address_bar: String,
    /// Whether the address bar is being edited.
    pub editing_address: bool,
    /// Lisp-authored DOM transforms applied after parse, before layout.
    pub transforms: TransformSet,
    /// Framework-aware selector aliases resolved per-page.
    pub aliases: AliasSet,
    /// State cells + effects + predicates + plans + agents — the full
    /// Lisp substrate loaded from extensions.lisp.
    pub substrate: Substrate,
    /// Runtime state cells owned by this browser session. Persists
    /// across navigations (memory-only for V1 — persistent cells land
    /// in a later commit).
    pub state_store: StateStore,
}

impl Browser {
    /// Create a new browser with the given config and initial URL.
    #[must_use]
    pub fn new(config: NamiConfig, initial_url: Option<&str>) -> Self {
        let history_path = config
            .history_file
            .clone()
            .unwrap_or_else(crate::config::default_history_path);
        let bookmarks_path = config
            .bookmarks_file
            .clone()
            .unwrap_or_else(crate::config::default_bookmarks_path);

        let fetcher = Fetcher::with_https(&config.network, config.privacy.https_only);
        let content_blocker = ContentBlocker::new(&config.content_blocking);
        let history = BrowsingHistory::load(&history_path);
        let bookmarks = Bookmarks::load(&bookmarks_path);

        let transforms_path = config
            .transforms_file
            .clone()
            .unwrap_or_else(crate::config::default_transforms_path);
        let transforms = match TransformSet::load(&transforms_path) {
            Ok(set) => {
                if !set.is_empty() {
                    tracing::info!(
                        "loaded {} Lisp DOM transforms from {transforms_path:?}",
                        set.len()
                    );
                }
                set
            }
            Err(e) => {
                tracing::warn!("failed to load transforms {transforms_path:?}: {e}");
                TransformSet::default()
            }
        };

        let aliases_path = config
            .aliases_file
            .clone()
            .unwrap_or_else(crate::config::default_aliases_path);
        let aliases = match AliasSet::load(&aliases_path) {
            Ok(set) => {
                if !set.is_empty() {
                    tracing::info!(
                        "loaded {} framework aliases from {aliases_path:?}",
                        set.len()
                    );
                }
                set
            }
            Err(e) => {
                tracing::warn!("failed to load aliases {aliases_path:?}: {e}");
                AliasSet::default()
            }
        };

        let extensions_path = config
            .extensions_file
            .clone()
            .unwrap_or_else(crate::config::default_extensions_path);
        let substrate = match Substrate::load(&extensions_path) {
            Ok(s) => {
                if !s.is_empty() {
                    tracing::info!("loaded substrate from {extensions_path:?}: {}", s.summary());
                }
                s
            }
            Err(e) => {
                tracing::warn!("failed to load extensions {extensions_path:?}: {e}");
                Substrate::default()
            }
        };
        let state_store = substrate.build_state_store();

        let tabs = match initial_url {
            Some(url) => {
                let normalized = url_util::normalize_input(url, &config.search_engine);
                TabManager::with_url(&normalized)
            }
            None => {
                if config.homepage == "about:blank" {
                    TabManager::new()
                } else {
                    TabManager::with_url(&config.homepage)
                }
            }
        };

        Self {
            tabs,
            history,
            bookmarks,
            content_blocker,
            input: InputHandler::new(),
            config,
            fetcher,
            viewport_width: 80,
            viewport_height: 24,
            status_message: None,
            should_quit: false,
            address_bar: String::new(),
            editing_address: false,
            transforms,
            aliases,
            substrate,
            state_store,
        }
    }

    /// Navigate the active tab to a URL.
    pub async fn navigate(&mut self, url: &str) {
        let normalized = url_util::normalize_input(url, &self.config.search_engine);

        // Check content blocker.
        if self.content_blocker.should_block(&normalized) {
            self.tabs
                .active_tab_mut()
                .set_error(&normalized, "Blocked by content filter".to_string());
            self.status_message = Some(format!("Blocked: {normalized}"));
            return;
        }

        // Handle internal pages.
        if normalized == "about:blank" {
            self.tabs.active_tab_mut().page_state = PageState::Blank;
            self.tabs.active_tab_mut().document = None;
            self.tabs.active_tab_mut().layout = None;
            return;
        }

        // Set loading state.
        self.tabs.active_tab_mut().page_state = PageState::Loading {
            url: normalized.clone(),
        };
        self.tabs.active_tab_mut().nav_stack.navigate(&normalized);

        // Fetch the page.
        match self.fetcher.fetch_page_with_css(&normalized).await {
            Ok((result, css_texts)) => {
                // Parse CSS.
                let mut stylesheets = Vec::new();
                for css in &css_texts {
                    stylesheets.push(Stylesheet::parse(css));
                }

                // Parse HTML and extract inline styles.
                let mut doc = Document::parse(&result.body);
                for inline_css in doc.inline_styles() {
                    stylesheets.push(Stylesheet::parse(&inline_css));
                }

                self.run_substrate_pipeline(&normalized, &result.body, &mut doc);

                let title = doc.title().unwrap_or("Untitled").to_string();

                // Record in history.
                self.history.add(&result.url, &title);

                // Set page state.
                let tab = self.tabs.active_tab_mut();
                tab.page_state = PageState::Loaded {
                    url: result.url.clone(),
                    title,
                    html: result.body,
                };
                tab.document = Some(doc);
                tab.stylesheets = stylesheets;
                tab.scroll_y = 0.0;
                tab.scroll_x = 0.0;
                tab.search_matches.clear();
                tab.current_match = None;

                // Compute layout.
                tab.compute_layout(self.viewport_width as f32 * 8.0);

                self.status_message = Some(format!("Loaded: {}", result.url));
            }
            Err(e) => {
                self.tabs
                    .active_tab_mut()
                    .set_error(&normalized, e.to_string());
                self.status_message = Some(format!("Error: {e}"));
            }
        }
    }

    /// Handle a browser action (from keyboard input).
    pub async fn handle_action(&mut self, action: BrowserAction) {
        match action {
            BrowserAction::ScrollDown(n) => {
                let line_height = 20.0;
                self.tabs.active_tab_mut().scroll_down(
                    n as f32 * line_height,
                    self.viewport_height as f32 * line_height,
                );
            }
            BrowserAction::ScrollUp(n) => {
                let line_height = 20.0;
                self.tabs.active_tab_mut().scroll_up(n as f32 * line_height);
            }
            BrowserAction::HalfPageDown => {
                let amount = self.viewport_height as f32 * 10.0;
                self.tabs
                    .active_tab_mut()
                    .scroll_down(amount, self.viewport_height as f32 * 20.0);
            }
            BrowserAction::HalfPageUp => {
                let amount = self.viewport_height as f32 * 10.0;
                self.tabs.active_tab_mut().scroll_up(amount);
            }
            BrowserAction::PageDown => {
                let amount = self.viewport_height as f32 * 20.0;
                self.tabs
                    .active_tab_mut()
                    .scroll_down(amount, self.viewport_height as f32 * 20.0);
            }
            BrowserAction::PageUp => {
                let amount = self.viewport_height as f32 * 20.0;
                self.tabs.active_tab_mut().scroll_up(amount);
            }
            BrowserAction::ScrollToTop => {
                self.tabs.active_tab_mut().scroll_y = 0.0;
            }
            BrowserAction::ScrollToBottom => {
                let height = self.tabs.active_tab().content_height();
                self.tabs.active_tab_mut().scroll_y = height;
            }
            BrowserAction::GoBack => {
                if let Some(url) = self.tabs.active_tab_mut().nav_stack.go_back() {
                    let url = url.clone();
                    self.navigate(&url).await;
                } else {
                    self.status_message = Some("No previous page".to_string());
                }
            }
            BrowserAction::GoForward => {
                if let Some(url) = self.tabs.active_tab_mut().nav_stack.go_forward() {
                    let url = url.clone();
                    self.navigate(&url).await;
                } else {
                    self.status_message = Some("No next page".to_string());
                }
            }
            BrowserAction::Reload => {
                let url = self.tabs.active_tab().url().to_string();
                self.navigate(&url).await;
            }
            BrowserAction::OpenUrl => {
                self.editing_address = true;
                self.address_bar.clear();
            }
            BrowserAction::OpenUrlNewTab => {
                self.tabs.new_tab(None);
                self.editing_address = true;
                self.address_bar.clear();
            }
            BrowserAction::NewTab => {
                self.tabs.new_tab(None);
            }
            BrowserAction::CloseTab => {
                self.tabs.close_tab();
            }
            BrowserAction::NextTab => {
                self.tabs.next_tab();
            }
            BrowserAction::PrevTab => {
                self.tabs.prev_tab();
            }
            BrowserAction::GotoTab(idx) => {
                self.tabs.goto_tab(idx);
            }
            BrowserAction::CopyUrl => {
                let url = self.tabs.active_tab().url().to_string();
                self.status_message = Some(format!("Copied: {url}"));
                // Clipboard integration would go here via hasami.
            }
            BrowserAction::SearchForward | BrowserAction::SearchBackward => {
                // Mode switch is handled by InputHandler.
            }
            BrowserAction::NextMatch => {
                self.tabs.active_tab_mut().next_match();
                let info = self.tabs.active_tab().match_count_str();
                self.status_message = Some(info);
            }
            BrowserAction::PrevMatch => {
                self.tabs.active_tab_mut().prev_match();
                let info = self.tabs.active_tab().match_count_str();
                self.status_message = Some(info);
            }
            BrowserAction::ToggleBookmark => {
                let url = self.tabs.active_tab().url().to_string();
                let title = self.tabs.active_tab().title().to_string();
                if self.bookmarks.is_bookmarked(&url) {
                    self.bookmarks.remove(&url);
                    self.status_message = Some("Bookmark removed".to_string());
                } else {
                    self.bookmarks.add(&url, &title, vec![]);
                    self.status_message = Some("Bookmark added".to_string());
                }
                let _ = self.bookmarks.save();
            }
            BrowserAction::ShowBookmarks => {
                let bm_html = render_bookmarks_page(&self.bookmarks);
                self.tabs
                    .active_tab_mut()
                    .set_loaded("about:bookmarks", bm_html);
                self.tabs
                    .active_tab_mut()
                    .compute_layout(self.viewport_width as f32 * 8.0);
            }
            BrowserAction::ShowHistory => {
                let hist_html = render_history_page(&self.history);
                self.tabs
                    .active_tab_mut()
                    .set_loaded("about:history", hist_html);
                self.tabs
                    .active_tab_mut()
                    .compute_layout(self.viewport_width as f32 * 8.0);
            }
            BrowserAction::ExecuteCommand(cmd) => {
                self.execute_command(&cmd).await;
            }
            BrowserAction::SubmitInput => {
                if self.editing_address {
                    let url = self.address_bar.clone();
                    self.editing_address = false;
                    self.navigate(&url).await;
                }
            }
            BrowserAction::CancelInput => {
                self.editing_address = false;
            }
            BrowserAction::Quit => {
                self.should_quit = true;
            }
            BrowserAction::Help => {
                // Show help page.
                self.status_message = Some("Press :help for commands, Esc to dismiss".to_string());
            }
            _ => {}
        }
    }

    /// Execute a : command.
    async fn execute_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        let command = parts[0];
        let arg = parts.get(1).copied().unwrap_or("");

        match command {
            "open" | "o" => {
                if !arg.is_empty() {
                    self.navigate(arg).await;
                }
            }
            "tabopen" | "topen" | "to" => {
                if !arg.is_empty() {
                    self.tabs.new_tab(None);
                    self.navigate(arg).await;
                }
            }
            "bookmark" | "bm" => {
                let url = self.tabs.active_tab().url().to_string();
                let title = self.tabs.active_tab().title().to_string();
                let tags: Vec<String> = if arg.is_empty() {
                    vec![]
                } else {
                    arg.split_whitespace().map(String::from).collect()
                };
                self.bookmarks.add(&url, &title, tags);
                let _ = self.bookmarks.save();
                self.status_message = Some("Bookmarked".to_string());
            }
            "bookmarks" | "bmarks" => {
                let html = render_bookmarks_page(&self.bookmarks);
                self.tabs
                    .active_tab_mut()
                    .set_loaded("about:bookmarks", html);
            }
            "history" | "hist" => {
                let html = render_history_page(&self.history);
                self.tabs.active_tab_mut().set_loaded("about:history", html);
            }
            "quit" | "q" => {
                self.should_quit = true;
            }
            "qa" | "qall" => {
                self.should_quit = true;
            }
            "reload" | "r" => {
                let url = self.tabs.active_tab().url().to_string();
                self.navigate(&url).await;
            }
            "back" => {
                if let Some(url) = self.tabs.active_tab_mut().nav_stack.go_back() {
                    let url = url.clone();
                    self.navigate(&url).await;
                }
            }
            "forward" => {
                if let Some(url) = self.tabs.active_tab_mut().nav_stack.go_forward() {
                    let url = url.clone();
                    self.navigate(&url).await;
                }
            }
            "source" | "src" => {
                // Show raw HTML source.
                if let PageState::Loaded { html, .. } = &self.tabs.active_tab().page_state {
                    let escaped = html
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;");
                    let source_html = format!("<html><body><pre>{escaped}</pre></body></html>");
                    self.tabs
                        .active_tab_mut()
                        .set_loaded("about:source", source_html);
                }
            }
            cmd if cmd.starts_with('/') || cmd.starts_with('?') => {
                let query = &cmd[1..];
                if !query.is_empty() {
                    self.tabs.active_tab_mut().search(query);
                    let info = self.tabs.active_tab().match_count_str();
                    self.status_message = Some(info);
                }
            }
            _ => {
                self.status_message = Some(format!("Unknown command: {command}"));
            }
        }
    }

    /// Render the current page as styled text.
    #[must_use]
    pub fn render_page(&self) -> render::RenderedPage {
        let tab = self.tabs.active_tab();
        match &tab.page_state {
            PageState::Blank => render::render_blank_page(),
            PageState::Loading { url } => {
                let html = format!("<html><body><p>Loading {url}...</p></body></html>");
                let doc = Document::parse(&html);
                render::render_document(&doc, None, self.viewport_width)
            }
            PageState::Loaded { .. } => {
                if let Some(ref doc) = tab.document {
                    render::render_document(doc, tab.layout.as_ref(), self.viewport_width)
                } else {
                    render::render_blank_page()
                }
            }
            PageState::Error { url, error } => {
                let html = format!(
                    "<html><body><h1>Error</h1><p>Failed to load {url}</p><p>{error}</p></body></html>"
                );
                let doc = Document::parse(&html);
                render::render_document(&doc, None, self.viewport_width)
            }
        }
    }

    /// Get the full display output as ANSI text.
    #[must_use]
    pub fn render_full(&self) -> String {
        let page = self.render_page();
        let tab_bar = render::render_tab_bar(&self.tabs.tab_titles());
        let status = render::render_status_bar(
            self.tabs.active_tab().url(),
            self.input.mode_indicator(),
            &format!("{}/{}", self.tabs.active_index() + 1, self.tabs.tab_count()),
            self.content_blocker.stats().blocked,
            self.tabs.active_tab().is_loading(),
        );

        let mut output = String::new();
        output.push_str(&tab_bar);
        output.push('\n');
        output.push_str(&render::to_ansi_text(&page));
        output.push_str(&status);
        output.push('\n');

        if let Some(ref msg) = self.status_message {
            output.push_str(msg);
            output.push('\n');
        }

        if self.input.mode() == Mode::Command || self.input.mode() == Mode::Search {
            let prefix = if self.input.mode() == Mode::Command {
                ":"
            } else {
                "/"
            };
            output.push_str(&format!("{prefix}{}", self.input.command_buffer()));
            output.push('\n');
        }

        output
    }

    /// Save state (history, bookmarks) before exit.
    pub fn save_state(&self) {
        if let Err(e) = self.history.save() {
            tracing::warn!(error = %e, "failed to save history");
        }
        if let Err(e) = self.bookmarks.save() {
            tracing::warn!(error = %e, "failed to save bookmarks");
        }
    }

    /// Post-parse, pre-layout pipeline: framework detection + effects
    /// + agent decisions + transform application. Runs in place on
    /// nami's `doc`; mutates `self.state_store` via effects.
    ///
    /// Phases:
    ///   1. Re-parse body into nami-core's DOM (only if substrate or
    ///      aliases are loaded — otherwise the old direct-transforms
    ///      fast path stays).
    ///   2. Run page-load effects against the state store.
    ///   3. Run page-load agents → list of transform names to apply.
    ///   4. If no agent fired, fall back to applying every transform
    ///      authored in transforms.lisp.
    ///   5. Apply the selected set through the transform engine,
    ///      expanding framework aliases when applicable.
    fn run_substrate_pipeline(&self, url: &str, body: &str, doc: &mut Document) {
        let substrate_live = !self.substrate.is_empty();
        let have_transforms = !self.transforms.is_empty();
        if !substrate_live && !have_transforms {
            return;
        }

        // Phase 0 — route matching. Write extracted params into the
        // state store BEFORE effects run so `(defeffect …)` bodies
        // can read them via bound symbols. Route-matched on-match
        // names merge into the transform-selection pass below.
        let mut route_on_match: Vec<String> = Vec::new();
        if !self.substrate.routes.is_empty() {
            if let Some(m) = self.substrate.routes.match_url(url) {
                // Write :bind pairs → state store.
                for (cell, param) in &m.bindings {
                    if let Some(val) = m.params.get(param) {
                        self.state_store
                            .set(cell, serde_json::Value::String(val.clone()));
                    }
                }
                tracing::info!(
                    "route '{}' matched {url} ({} params, {} on-match)",
                    m.route,
                    m.params.len(),
                    m.on_match.len()
                );
                route_on_match = m.on_match;
            }
        }

        // Phase 0.5 — dispatch any on-match names that are QUERIES
        // rather than transforms. Queries populate state cells; they
        // fire before effects so effect bodies can see fresh data.
        // Non-query names fall through to the transform pass below.
        let mut remaining_route_on_match: Vec<String> = Vec::new();
        if !self.substrate.queries.is_empty() {
            let fetcher = crate::query_fetcher::BlockingFetcher::new(&self.config.network);
            for name in &route_on_match {
                if self.substrate.queries.get(name).is_some() {
                    match self
                        .substrate
                        .queries
                        .run(name, &fetcher, &self.state_store)
                    {
                        Ok(r) => tracing::info!(
                            "query '{}' → state cell '{}' ({} bytes{})",
                            r.query,
                            r.into,
                            r.bytes,
                            if r.parsed_json { ", json" } else { "" }
                        ),
                        Err(e) => tracing::warn!("query '{name}' failed: {e}"),
                    }
                } else {
                    remaining_route_on_match.push(name.clone());
                }
            }
        } else {
            remaining_route_on_match = route_on_match;
        }
        let route_on_match = remaining_route_on_match;

        // We need nami-core's doc only when aliases, predicates, or
        // agents need to read it. Skip the second parse otherwise.
        let need_core = substrate_live || !self.aliases.is_empty();
        let (detections, agent_decided) = if need_core {
            let core_doc = nami_core::dom::Document::parse(body);
            let detections = nami_core::framework::detect(&core_doc);
            let page_state = nami_core::state::extract(&core_doc);

            if !self.substrate.effects.is_empty() {
                let cx = nami_core::predicate::EvalContext {
                    doc: &core_doc,
                    detections: &detections,
                    state: &page_state,
                };
                // Use the derived-aware variant so effect bodies can
                // read `(defderived …)` values alongside state cells.
                // Back-compat: when the derived registry is empty the
                // variant is semantically identical to run_page_load.
                let (_decisions, reports) = nami_core::effect::run_page_load_with_derived(
                    &self.state_store,
                    &self.substrate.effects,
                    &self.substrate.derived,
                    &self.substrate.predicates,
                    &cx,
                );
                let fired = reports.iter().filter(|r| r.ok).count();
                if fired > 0 {
                    tracing::info!("ran {fired} Lisp effects on {url}");
                }
            }

            let decided: Vec<String> = if self.substrate.agents.is_empty() {
                Vec::new()
            } else {
                let cx = nami_core::predicate::EvalContext {
                    doc: &core_doc,
                    detections: &detections,
                    state: &page_state,
                };
                nami_core::agent::decide(
                    &self.substrate.agents,
                    "page-load",
                    &self.substrate.predicates,
                    &self.substrate.plans,
                    &cx,
                )
                .into_iter()
                .filter(|d| d.fired)
                .flat_map(|d| d.transforms)
                .collect()
            };

            (detections, decided)
        } else {
            (Vec::new(), Vec::new())
        };

        // Merge agent-decided + route-matched on-match names. If neither
        // produced anything, fall back to every transform in
        // transforms.lisp (preserves prior behaviour).
        let mut decided_names: Vec<String> = route_on_match;
        decided_names.extend(agent_decided);
        let selected: Vec<_> = if decided_names.is_empty() {
            self.transforms.specs.clone()
        } else {
            decided_names
                .iter()
                .filter_map(|name| {
                    self.transforms
                        .specs
                        .iter()
                        .find(|t| &t.name == name)
                        .cloned()
                })
                .collect()
        };

        if selected.is_empty() {
            return;
        }

        let set = crate::transform::TransformSet { specs: selected };
        let report = if self.aliases.is_empty() {
            set.apply(doc)
        } else {
            set.apply_with_aliases(doc, &self.aliases.registry(), &detections)
        };
        if !report.applied.is_empty() {
            tracing::info!("applied {} Lisp transforms to {url}", report.applied.len());
        }
    }
}

/// Generate HTML for the bookmarks page.
fn render_bookmarks_page(bookmarks: &Bookmarks) -> String {
    let mut html =
        String::from("<html><head><title>Bookmarks</title></head><body><h1>Bookmarks</h1><ul>");

    for bm in bookmarks.all() {
        let tags = if bm.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", bm.tags.join(", "))
        };
        html.push_str(&format!(
            r#"<li><a href="{}">{}</a>{}</li>"#,
            bm.url, bm.title, tags
        ));
    }

    if bookmarks.is_empty() {
        html.push_str("<li>No bookmarks yet. Press B to bookmark a page.</li>");
    }

    html.push_str("</ul></body></html>");
    html
}

/// Generate HTML for the history page.
fn render_history_page(history: &BrowsingHistory) -> String {
    let mut html = String::from(
        "<html><head><title>History</title></head><body><h1>Browsing History</h1><ul>",
    );

    for entry in history.entries().iter().take(100) {
        html.push_str(&format!(
            r#"<li><a href="{}">{}</a></li>"#,
            entry.url, entry.title
        ));
    }

    if history.is_empty() {
        html.push_str("<li>No history yet.</li>");
    }

    html.push_str("</ul></body></html>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> NamiConfig {
        NamiConfig::default()
    }

    #[test]
    fn browser_creates() {
        let browser = Browser::new(test_config(), None);
        assert!(!browser.should_quit);
        assert_eq!(browser.tabs.tab_count(), 1);
    }

    #[test]
    fn browser_creates_with_url() {
        let browser = Browser::new(test_config(), Some("https://example.com"));
        assert_eq!(browser.tabs.active_tab().url(), "https://example.com");
    }

    #[test]
    fn browser_render_blank() {
        let browser = Browser::new(test_config(), None);
        let page = browser.render_page();
        let text = render::to_plain_text(&page);
        assert!(text.contains("Nami Browser"));
    }

    #[test]
    fn browser_render_full() {
        let browser = Browser::new(test_config(), None);
        let output = browser.render_full();
        assert!(output.contains("NORMAL"));
    }

    #[test]
    fn bookmarks_page_rendering() {
        let mut bookmarks = Bookmarks::load(&std::path::PathBuf::from("/tmp/test-bm.json"));
        bookmarks.add("https://example.com", "Example", vec!["test".into()]);
        let html = render_bookmarks_page(&bookmarks);
        assert!(html.contains("Example"));
        assert!(html.contains("example.com"));
    }

    #[test]
    fn history_page_rendering() {
        let mut history = BrowsingHistory::load(&std::path::PathBuf::from("/tmp/test-hist.json"));
        history.add("https://example.com", "Example");
        let html = render_history_page(&history);
        assert!(html.contains("Example"));
    }
}
