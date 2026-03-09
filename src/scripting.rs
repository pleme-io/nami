//! Rhai scripting plugin system.
//!
//! Loads user scripts from `~/.config/nami/scripts/*.rhai` and registers
//! app-specific functions for browser automation and extension.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use soushi::ScriptEngine;

/// Event hooks that scripts can define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptEvent {
    /// Fired when the browser starts.
    OnStart,
    /// Fired when the browser is quitting.
    OnQuit,
    /// Fired on key press with the key name.
    OnKey(String),
}

/// Manages the Rhai scripting engine with nami-specific functions.
pub struct NamiScriptEngine {
    engine: ScriptEngine,
    /// Compiled event hook ASTs (script name, hook name, AST).
    hooks: Vec<(String, String, soushi::rhai::AST)>,
    /// Shared state for script-triggered actions.
    pub pending_actions: Arc<Mutex<Vec<ScriptAction>>>,
}

/// Actions that scripts can trigger.
#[derive(Debug, Clone)]
pub enum ScriptAction {
    /// Navigate to a URL.
    Navigate(String),
    /// Add a bookmark.
    AddBookmark { url: String, title: String },
}

impl NamiScriptEngine {
    /// Create a new scripting engine with nami-specific functions registered.
    #[must_use]
    pub fn new() -> Self {
        let mut engine = ScriptEngine::new();
        engine.register_builtin_log();
        engine.register_builtin_env();
        engine.register_builtin_string();

        let pending = Arc::new(Mutex::new(Vec::<ScriptAction>::new()));

        // Register nami.navigate(url)
        let p = Arc::clone(&pending);
        engine.register_fn("nami_navigate", move |url: &str| {
            if let Ok(mut actions) = p.lock() {
                actions.push(ScriptAction::Navigate(url.to_string()));
            }
        });

        // Register nami.get_links() — returns empty array (placeholder for live DOM)
        engine.register_fn("nami_get_links", || -> soushi::rhai::Array {
            soushi::rhai::Array::new()
        });

        // Register nami.add_bookmark(url, title)
        let p = Arc::clone(&pending);
        engine.register_fn("nami_add_bookmark", move |url: &str, title: &str| {
            if let Ok(mut actions) = p.lock() {
                actions.push(ScriptAction::AddBookmark {
                    url: url.to_string(),
                    title: title.to_string(),
                });
            }
        });

        // Register nami.get_text() — returns empty string (placeholder for live DOM)
        engine.register_fn("nami_get_text", || -> String {
            String::new()
        });

        Self {
            engine,
            hooks: Vec::new(),
            pending_actions: pending,
        }
    }

    /// Load scripts from the default config directory.
    pub fn load_user_scripts(&mut self) {
        let scripts_dir = scripts_dir();
        if scripts_dir.is_dir() {
            match self.engine.load_scripts_dir(&scripts_dir) {
                Ok(names) => {
                    if !names.is_empty() {
                        tracing::info!(count = names.len(), "loaded nami scripts: {names:?}");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to load nami scripts");
                }
            }
        }
    }

    /// Fire an event hook. Scripts can register hooks by defining functions
    /// named `on_start`, `on_quit`, or `on_key(key)`.
    pub fn fire_event(&self, event: &ScriptEvent) {
        let hook_name = match event {
            ScriptEvent::OnStart => "on_start",
            ScriptEvent::OnQuit => "on_quit",
            ScriptEvent::OnKey(_) => "on_key",
        };

        let script = match event {
            ScriptEvent::OnKey(key) => format!("if is_def_fn(\"{hook_name}\", 1) {{ {hook_name}(\"{key}\"); }}"),
            _ => format!("if is_def_fn(\"{hook_name}\", 0) {{ {hook_name}(); }}"),
        };

        if let Err(e) = self.engine.eval(&script) {
            tracing::debug!(hook = hook_name, error = %e, "script hook not defined or failed");
        }
    }

    /// Drain any pending actions triggered by scripts.
    pub fn drain_actions(&self) -> Vec<ScriptAction> {
        if let Ok(mut actions) = self.pending_actions.lock() {
            actions.drain(..).collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for NamiScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Default scripts directory: `~/.config/nami/scripts/`.
fn scripts_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("nami")
        .join("scripts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_creation() {
        let _engine = NamiScriptEngine::new();
    }

    #[test]
    fn navigate_action() {
        let engine = NamiScriptEngine::new();
        engine
            .engine
            .eval(r#"nami_navigate("https://example.com")"#)
            .unwrap();
        let actions = engine.drain_actions();
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], ScriptAction::Navigate(url) if url == "https://example.com"));
    }

    #[test]
    fn add_bookmark_action() {
        let engine = NamiScriptEngine::new();
        engine
            .engine
            .eval(r#"nami_add_bookmark("https://rust-lang.org", "Rust")"#)
            .unwrap();
        let actions = engine.drain_actions();
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0], ScriptAction::AddBookmark { url, title } if url == "https://rust-lang.org" && title == "Rust")
        );
    }

    #[test]
    fn get_links_returns_array() {
        let engine = NamiScriptEngine::new();
        let result = engine.engine.eval("nami_get_links()").unwrap();
        assert!(result.is_array());
    }

    #[test]
    fn get_text_returns_string() {
        let engine = NamiScriptEngine::new();
        let result = engine.engine.eval("nami_get_text()").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn fire_event_does_not_panic() {
        let engine = NamiScriptEngine::new();
        engine.fire_event(&ScriptEvent::OnStart);
        engine.fire_event(&ScriptEvent::OnQuit);
        engine.fire_event(&ScriptEvent::OnKey("j".to_string()));
    }

    #[test]
    fn drain_actions_clears() {
        let engine = NamiScriptEngine::new();
        engine
            .engine
            .eval(r#"nami_navigate("https://a.com")"#)
            .unwrap();
        assert_eq!(engine.drain_actions().len(), 1);
        assert!(engine.drain_actions().is_empty());
    }
}
