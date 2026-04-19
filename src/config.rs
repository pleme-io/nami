//! Configuration -- shikumi hot-reload config store.
//!
//! Manages `~/.config/nami/nami.yaml` with env override `NAMI_CONFIG`
//! and env prefix `NAMI_`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level nami configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamiConfig {
    #[serde(default = "default_homepage")]
    pub homepage: String,
    #[serde(default = "default_search_engine")]
    pub search_engine: String,
    #[serde(default)]
    pub appearance: AppearanceConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub content_blocking: ContentBlockingConfig,
    #[serde(default)]
    pub bookmarks_file: Option<PathBuf>,
    #[serde(default)]
    pub history_file: Option<PathBuf>,
    /// Path to a tatara-lisp file containing `(defdom-transform …)` forms.
    /// Defaults to `~/.config/nami/transforms.lisp` when absent.
    #[serde(default)]
    pub transforms_file: Option<PathBuf>,
    /// Path to a tatara-lisp file containing `(defframework-alias …)` forms.
    /// Defaults to `~/.config/nami/aliases.lisp` when absent.
    #[serde(default)]
    pub aliases_file: Option<PathBuf>,
}

/// Appearance settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    #[serde(default = "default_bg")]
    pub background: String,
    #[serde(default = "default_fg")]
    pub foreground: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_link_color")]
    pub link_color: String,
    #[serde(default = "default_visited_color")]
    pub visited_link_color: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_true")]
    pub show_images: bool,
    #[serde(default = "default_true")]
    pub dark_mode: bool,
}

/// Network / HTTP settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_true")]
    pub follow_redirects: bool,
    pub proxy: Option<String>,
}

/// Privacy settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    #[serde(default = "default_true")]
    pub block_trackers: bool,
    #[serde(default)]
    pub clear_on_exit: bool,
    #[serde(default)]
    pub https_only: bool,
    #[serde(default = "default_true")]
    pub do_not_track: bool,
}

/// Content blocking settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub block_trackers: bool,
    #[serde(default)]
    pub block_ads: bool,
    #[serde(default)]
    pub filter_lists: Vec<PathBuf>,
    #[serde(default)]
    pub blocked_domains: Vec<String>,
}

// --- Defaults ---

impl Default for NamiConfig {
    fn default() -> Self {
        Self {
            homepage: default_homepage(),
            search_engine: default_search_engine(),
            appearance: AppearanceConfig::default(),
            network: NetworkConfig::default(),
            privacy: PrivacyConfig::default(),
            content_blocking: ContentBlockingConfig::default(),
            bookmarks_file: None,
            history_file: None,
            transforms_file: None,
            aliases_file: None,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            background: default_bg(),
            foreground: default_fg(),
            accent: default_accent(),
            link_color: default_link_color(),
            visited_link_color: default_visited_color(),
            font_size: default_font_size(),
            show_images: true,
            dark_mode: true,
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            user_agent: default_user_agent(),
            timeout_secs: default_timeout(),
            follow_redirects: true,
            proxy: None,
        }
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            block_trackers: true,
            clear_on_exit: false,
            https_only: false,
            do_not_track: true,
        }
    }
}

impl Default for ContentBlockingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            block_trackers: true,
            block_ads: false,
            filter_lists: Vec::new(),
            blocked_domains: Vec::new(),
        }
    }
}

fn default_homepage() -> String {
    "about:blank".into()
}
fn default_search_engine() -> String {
    "https://www.google.com/search?q=%s".into()
}
fn default_bg() -> String {
    "#2e3440".into()
}
fn default_fg() -> String {
    "#eceff4".into()
}
fn default_accent() -> String {
    "#88c0d0".into()
}
fn default_link_color() -> String {
    "#5e81ac".into()
}
fn default_visited_color() -> String {
    "#b48ead".into()
}
fn default_font_size() -> f32 {
    14.0
}
fn default_true() -> bool {
    true
}
fn default_user_agent() -> String {
    "Nami/0.1.0".to_string()
}
fn default_timeout() -> u64 {
    30
}

/// Load configuration from disk or defaults.
pub fn load(override_path: &Option<PathBuf>) -> anyhow::Result<NamiConfig> {
    let path = match override_path {
        Some(p) => p.clone(),
        None => match shikumi::ConfigDiscovery::new("nami")
            .env_override("NAMI_CONFIG")
            .discover()
        {
            Ok(p) => p,
            Err(_) => {
                tracing::info!("no config file found, using defaults");
                return Ok(NamiConfig::default());
            }
        },
    };

    let store = shikumi::ConfigStore::<NamiConfig>::load(&path, "NAMI_")?;
    Ok(NamiConfig::clone(&store.get()))
}

/// Get the default bookmarks file path.
#[must_use]
pub fn default_bookmarks_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nami")
        .join("bookmarks.json")
}

/// Get the default history file path.
#[must_use]
pub fn default_history_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nami")
        .join("history.json")
}

/// Default tatara-lisp transforms file path: `$XDG_CONFIG_HOME/nami/transforms.lisp`
/// or `$HOME/.config/nami/transforms.lisp`. Matches shikumi's config discovery
/// convention rather than macOS's Application Support default from `dirs::config_dir`.
#[must_use]
pub fn default_transforms_path() -> PathBuf {
    config_file("transforms.lisp")
}

/// Default tatara-lisp aliases file path — same rules as
/// [`default_transforms_path`].
#[must_use]
pub fn default_aliases_path() -> PathBuf {
    config_file("aliases.lisp")
}

/// Default tatara-lisp scrapes file path — same discovery rules as
/// [`default_transforms_path`].
#[must_use]
pub fn default_scrapes_path() -> PathBuf {
    config_file("scrapes.lisp")
}

fn config_file(name: &str) -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".config"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("nami").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = NamiConfig::default();
        assert_eq!(cfg.homepage, "about:blank");
        assert_eq!(cfg.appearance.foreground, "#eceff4");
        assert_eq!(cfg.network.timeout_secs, 30);
        assert!(cfg.privacy.do_not_track);
        assert!(cfg.content_blocking.enabled);
    }

    #[test]
    fn default_paths_exist() {
        let bm = default_bookmarks_path();
        assert!(bm.to_str().unwrap().contains("nami"));
        let hist = default_history_path();
        assert!(hist.to_str().unwrap().contains("nami"));
    }
}
