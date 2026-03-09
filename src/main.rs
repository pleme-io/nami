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

mod bookmarks;
mod browser;
mod config;
mod content_blocking;
mod css;
mod dom;
mod fetch;
mod history;
mod input;
mod layout;
mod mcp;
mod render;
mod tabs;
mod url_util;

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
    /// Show browsing history
    History,
    /// Render a page and print ANSI output (non-interactive)
    Render {
        url: String,
        /// Viewport width in characters
        #[arg(long, default_value = "80")]
        width: u32,
    },
    /// Start the MCP server (stdio transport).
    Mcp,
}

#[derive(Subcommand)]
enum BookmarkAction {
    List,
    Add { url: String, title: Option<String> },
    Remove { url: String },
    Search { query: String },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let cfg = config::load(&cli.config)?;

    let rt = tokio::runtime::Runtime::new()?;

    match cli.command {
        None => {
            let url = cli
                .url
                .clone()
                .unwrap_or_else(|| cfg.homepage.clone());
            tracing::info!("launching nami: {url}");
            rt.block_on(run_browser(cfg, &url))?;
        }
        Some(Commands::Open { url }) => {
            tracing::info!("opening: {url}");
            rt.block_on(run_browser(cfg, &url))?;
        }
        Some(Commands::Source { url }) => {
            rt.block_on(async {
                let fetcher = fetch::Fetcher::new(&cfg.network);
                match fetcher.fetch_text(&url).await {
                    Ok(body) => println!("{body}"),
                    Err(e) => eprintln!("Error: {e}"),
                }
            });
        }
        Some(Commands::Text { url }) => {
            rt.block_on(async {
                let fetcher = fetch::Fetcher::new(&cfg.network);
                match fetcher.fetch(&url).await {
                    Ok(result) => {
                        let doc = dom::Document::parse(&result.body);
                        let text = dom::node_to_text(&doc.root, 0);
                        println!("{text}");
                    }
                    Err(e) => eprintln!("Error: {e}"),
                }
            });
        }
        Some(Commands::Bookmarks { action }) => {
            handle_bookmarks(&cfg, action)?;
        }
        Some(Commands::History) => {
            let history_path = cfg
                .history_file
                .clone()
                .unwrap_or_else(config::default_history_path);
            let history = history::BrowsingHistory::load(&history_path);
            for entry in history.entries().iter().take(50) {
                println!("{} - {}", entry.url, entry.title);
            }
            if history.is_empty() {
                println!("No history yet.");
            }
        }
        Some(Commands::Mcp) => {
            rt.block_on(async {
                if let Err(e) = mcp::run(cfg).await {
                    eprintln!("MCP server error: {e}");
                    std::process::exit(1);
                }
            });
        }
        Some(Commands::Render { url, width }) => {
            rt.block_on(async {
                let fetcher = fetch::Fetcher::new(&cfg.network);
                match fetcher.fetch_page_with_css(&url).await {
                    Ok((result, css_texts)) => {
                        let doc = dom::Document::parse(&result.body);
                        let mut stylesheets = Vec::new();
                        for css_text in &css_texts {
                            stylesheets.push(css::Stylesheet::parse(css_text));
                        }
                        for inline_css in doc.inline_styles() {
                            stylesheets.push(css::Stylesheet::parse(&inline_css));
                        }
                        let layout_tree =
                            layout::LayoutTree::compute(&doc, &stylesheets, width as f32 * 8.0);
                        let page =
                            render::render_document(&doc, Some(&layout_tree), width);
                        let output = render::to_ansi_text(&page);
                        print!("{output}");
                    }
                    Err(e) => eprintln!("Error: {e}"),
                }
            });
        }
    }

    Ok(())
}

/// Run the browser in interactive (TUI) mode.
async fn run_browser(cfg: config::NamiConfig, url: &str) -> anyhow::Result<()> {
    let mut browser_state = browser::Browser::new(cfg, Some(url));

    // Navigate to the initial URL if not blank.
    if url != "about:blank" {
        browser_state.navigate(url).await;
    }

    // In non-GPU mode (until garasu/winit event loop is integrated), render once
    // and print the page. This enables `nami <url>` to work as a CLI page renderer.
    let output = browser_state.render_full();
    print!("{output}");

    // Save state on exit.
    browser_state.save_state();

    Ok(())
}

/// Handle bookmark subcommands.
fn handle_bookmarks(
    cfg: &config::NamiConfig,
    action: Option<BookmarkAction>,
) -> anyhow::Result<()> {
    let bookmarks_path = cfg
        .bookmarks_file
        .clone()
        .unwrap_or_else(config::default_bookmarks_path);
    let mut bm = bookmarks::Bookmarks::load(&bookmarks_path);

    match action {
        None | Some(BookmarkAction::List) => {
            for bookmark in bm.all() {
                let tags = if bookmark.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", bookmark.tags.join(", "))
                };
                println!("{} - {}{tags}", bookmark.url, bookmark.title);
            }
            if bm.is_empty() {
                println!("No bookmarks yet.");
            }
        }
        Some(BookmarkAction::Add { url, title }) => {
            let title = title.unwrap_or_else(|| url.clone());
            if bm.add(&url, &title, vec![]) {
                bm.save()?;
                println!("Added: {url}");
            } else {
                println!("Already bookmarked: {url}");
            }
        }
        Some(BookmarkAction::Remove { url }) => {
            if bm.remove(&url) {
                bm.save()?;
                println!("Removed: {url}");
            } else {
                println!("Not found: {url}");
            }
        }
        Some(BookmarkAction::Search { query }) => {
            let results = bm.search(&query);
            for bookmark in results {
                println!("{} - {}", bookmark.url, bookmark.title);
            }
        }
    }

    Ok(())
}
