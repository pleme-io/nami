use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamiConfig {
    #[serde(default = "default_homepage")]
    pub homepage: String,
    #[serde(default)]
    pub appearance: AppearanceConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
}

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
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_true")]
    pub show_images: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    #[serde(default = "default_true")]
    pub block_trackers: bool,
    #[serde(default)]
    pub clear_on_exit: bool,
    #[serde(default = "default_true")]
    pub https_only: bool,
}

impl Default for NamiConfig {
    fn default() -> Self {
        Self {
            homepage: default_homepage(),
            appearance: AppearanceConfig::default(),
            network: NetworkConfig::default(),
            privacy: PrivacyConfig::default(),
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
            font_size: default_font_size(),
            show_images: true,
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
        Self { block_trackers: true, clear_on_exit: false, https_only: true }
    }
}

fn default_homepage() -> String { "about:blank".into() }
fn default_bg() -> String { "#2e3440".into() }
fn default_fg() -> String { "#eceff4".into() }
fn default_accent() -> String { "#88c0d0".into() }
fn default_link_color() -> String { "#5e81ac".into() }
fn default_font_size() -> f32 { 14.0 }
fn default_true() -> bool { true }
fn default_user_agent() -> String { format!("Nami/0.1.0") }
fn default_timeout() -> u64 { 30 }

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
