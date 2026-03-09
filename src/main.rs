//! Nami (波) — GPU-rendered TUI browser.
//!
//! Full web rendering in a GPU-accelerated terminal interface:
//! - HTML5 parsing via html5ever
//! - CSS parsing via lightningcss
//! - Flexbox/grid layout via taffy
//! - GPU text and image rendering via garasu
//! - Widget-based UI (tabs, address bar, bookmarks) via egaku
//! - Rich text rendering via mojiban
//! - Hot-reloadable configuration via shikumi

mod config;
mod css;
mod dom;
mod fetch;
mod layout;
mod render;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "nami", version, about = "GPU-rendered TUI browser")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// URL to open
    url: Option<String>,

    /// Configuration file override
    #[arg(long, env = "NAMI_CONFIG")]
    config: Option<std::path::PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a URL
    Open {
        url: String,
    },
    /// Fetch and dump page source
    Source {
        url: String,
    },
    /// Fetch and render to plain text
    Text {
        url: String,
    },
    /// Manage bookmarks
    Bookmarks {
        #[command(subcommand)]
        action: Option<BookmarkAction>,
    },
}

#[derive(Subcommand)]
enum BookmarkAction {
    List,
    Add { url: String, title: Option<String> },
    Remove { url: String },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = config::load(&cli.config)?;

    match cli.command {
        None => {
            let url = cli.url.as_deref().unwrap_or("about:blank");
            tracing::info!("launching nami: {url}");
            // TODO: Initialize garasu GPU context
            // TODO: Create winit window with browser UI (egaku widgets)
            // TODO: Fetch, parse, layout, render page
        }
        Some(Commands::Open { url }) => {
            tracing::info!("opening: {url}");
            // TODO: Launch browser with URL
        }
        Some(Commands::Source { url }) => {
            // TODO: Fetch and print raw HTML
            tracing::info!("fetching source: {url}");
        }
        Some(Commands::Text { url }) => {
            // TODO: Fetch, parse, extract text content
            tracing::info!("fetching text: {url}");
        }
        Some(Commands::Bookmarks { action }) => {
            // TODO: Bookmark management
        }
    }

    Ok(())
}
