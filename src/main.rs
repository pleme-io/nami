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
mod scripting;
mod tabs;
mod transform;
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
    Open { url: String },
    /// Fetch and dump page source
    Source { url: String },
    /// Fetch and render to plain text
    Text { url: String },
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
    /// Dissect any page into its Lisp-space representation.
    ///
    /// Prints (in order):
    ///   1. Detected frameworks + evidence
    ///   2. Embedded JSON state blobs (Next.js / Remix / Nuxt / JSON-LD / etc.)
    ///   3. The DOM serialized as S-expressions (capped by --depth)
    Dissect {
        url: String,
        /// Max nesting depth in the DOM-to-Lisp dump.
        #[arg(long, default_value = "6")]
        depth: usize,
        /// Emit everything as one JSON object on stdout instead of the
        /// sectioned plain-text view. Good for piping to jq / MCP.
        #[arg(long)]
        json: bool,
        /// Skip the DOM dump (only frameworks + state). Handy for overview.
        #[arg(long)]
        no_dom: bool,
    },
    /// Scrape structured data from a page using a Lisp `(defscrape …)` file.
    ///
    /// Default config path: `$XDG_CONFIG_HOME/nami/scrapes.lisp`.
    /// Output: one JSON object per line (JSONL) unless --json-array is passed.
    Scrape {
        url: String,
        /// Path to a Lisp file of `(defscrape …)` forms.
        #[arg(long, short = 'c')]
        config: Option<std::path::PathBuf>,
        /// Inline Lisp source (appended to file contents if both given).
        #[arg(long, short = 'e')]
        expr: Option<String>,
        /// Wrap results in a JSON array instead of JSONL.
        #[arg(long)]
        json_array: bool,
    },
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
            let url = cli.url.clone().unwrap_or_else(|| cfg.homepage.clone());
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
        Some(Commands::Dissect {
            url,
            depth,
            json,
            no_dom,
        }) => {
            rt.block_on(async {
                let fetcher = fetch::Fetcher::new(&cfg.network);
                match fetcher.fetch(&url).await {
                    Ok(result) => {
                        let doc = nami_core::dom::Document::parse(&result.body);
                        let frameworks = nami_core::framework::detect(&doc);
                        let state = nami_core::state::extract(&doc);
                        let dom_sexp = if no_dom {
                            None
                        } else {
                            Some(nami_core::lisp::dom_to_sexp_with(
                                &doc,
                                &nami_core::lisp::SexpOptions {
                                    depth_cap: Some(depth),
                                    pretty: true,
                                    trim_whitespace: true,
                                },
                            ))
                        };

                        if json {
                            let obj = serde_json::json!({
                                "url": result.url,
                                "bytes": result.body.len(),
                                "frameworks": frameworks,
                                "state": state,
                                "dom_sexp": dom_sexp,
                            });
                            println!("{}", serde_json::to_string_pretty(&obj).unwrap());
                        } else {
                            println!("════════════════════════════════════════");
                            println!(" nami dissect  {}", result.url);
                            println!(" {} bytes", result.body.len());
                            println!("════════════════════════════════════════");
                            println!();
                            println!("── frameworks ──");
                            if frameworks.is_empty() {
                                println!("  (none detected — likely plain HTML)");
                            } else {
                                for f in &frameworks {
                                    println!("  {:<14}  confidence {:.2}", f.name, f.confidence);
                                    for e in &f.evidence {
                                        println!("    · {e}");
                                    }
                                }
                            }
                            println!();
                            println!("── embedded state ──");
                            if state.is_empty() {
                                println!("  (no embedded JSON state found)");
                            } else {
                                for s in &state {
                                    let id =
                                        s.id.as_deref()
                                            .map(|i| format!(" id={i}"))
                                            .unwrap_or_default();
                                    let ok = if s.value.is_some() { "✓" } else { "✗" };
                                    println!("  {ok} {:?}{id}  {} bytes", s.kind, s.bytes);
                                    if let Some(v) = &s.value {
                                        let preview = serde_json::to_string(v).unwrap();
                                        let preview = if preview.len() > 160 {
                                            format!("{}…", &preview[..160])
                                        } else {
                                            preview
                                        };
                                        println!("      {preview}");
                                    }
                                }
                            }
                            if let Some(sexp) = dom_sexp {
                                println!();
                                println!("── dom as lisp (depth cap = {depth}) ──");
                                println!("{sexp}");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("nami: fetch: {e}");
                        std::process::exit(2);
                    }
                }
            });
        }
        Some(Commands::Scrape {
            url,
            config: scrape_config,
            expr,
            json_array,
        }) => {
            rt.block_on(async {
                let fetcher = fetch::Fetcher::new(&cfg.network);
                let scrapes_path = scrape_config
                    .clone()
                    .unwrap_or_else(config::default_scrapes_path);

                let mut lisp_src = String::new();
                if scrapes_path.exists() {
                    match std::fs::read_to_string(&scrapes_path) {
                        Ok(s) => lisp_src.push_str(&s),
                        Err(e) => {
                            eprintln!("nami: read {scrapes_path:?}: {e}");
                            std::process::exit(2);
                        }
                    }
                }
                if let Some(extra) = expr.as_deref() {
                    if !lisp_src.is_empty() {
                        lisp_src.push('\n');
                    }
                    lisp_src.push_str(extra);
                }
                if lisp_src.trim().is_empty() {
                    eprintln!(
                        "nami: no (defscrape …) forms — pass -c <file> or -e '<lisp>', or put them at {scrapes_path:?}"
                    );
                    std::process::exit(2);
                }

                let specs = match nami_core::scrape::compile(&lisp_src) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("nami: parse scrapes: {e}");
                        std::process::exit(2);
                    }
                };

                match fetcher.fetch(&url).await {
                    Ok(result) => {
                        // Re-parse with nami-core's Document so scrape runs.
                        let core_doc = nami_core::dom::Document::parse(&result.body);

                        // Expand framework aliases if the user has any.
                        let aliases_path = cfg
                            .aliases_file
                            .clone()
                            .unwrap_or_else(config::default_aliases_path);
                        let aliases = transform::AliasSet::load(&aliases_path)
                            .unwrap_or_default();
                        let specs = if aliases.is_empty() {
                            specs
                        } else {
                            let detections = nami_core::framework::detect(&core_doc);
                            let registry = aliases.registry();
                            registry.expand_scrapes(&specs, &detections)
                        };

                        let hits = nami_core::scrape::scrape(&core_doc, &specs);
                        if json_array {
                            println!("{}", serde_json::to_string_pretty(&hits).unwrap());
                        } else {
                            for hit in &hits {
                                println!("{}", serde_json::to_string(hit).unwrap());
                            }
                        }
                        eprintln!(
                            "[scrape] {} hits across {} spec(s) against {}",
                            hits.len(),
                            specs.len(),
                            result.url
                        );
                    }
                    Err(e) => {
                        eprintln!("nami: fetch: {e}");
                        std::process::exit(2);
                    }
                }
            });
        }
        Some(Commands::Render { url, width }) => {
            rt.block_on(async {
                let fetcher = fetch::Fetcher::new(&cfg.network);
                let transforms_path = cfg
                    .transforms_file
                    .clone()
                    .unwrap_or_else(config::default_transforms_path);
                let transforms =
                    transform::TransformSet::load(&transforms_path).unwrap_or_default();
                let aliases_path = cfg
                    .aliases_file
                    .clone()
                    .unwrap_or_else(config::default_aliases_path);
                let aliases = transform::AliasSet::load(&aliases_path).unwrap_or_default();

                match fetcher.fetch_page_with_css(&url).await {
                    Ok((result, css_texts)) => {
                        let mut doc = dom::Document::parse(&result.body);

                        if !transforms.is_empty() {
                            let report = if aliases.is_empty() {
                                transforms.apply(&mut doc)
                            } else {
                                let core_doc = nami_core::dom::Document::parse(&result.body);
                                let detections = nami_core::framework::detect(&core_doc);
                                let registry = aliases.registry();
                                transforms.apply_with_aliases(&mut doc, &registry, &detections)
                            };
                            eprintln!(
                                "[transforms] {} applied to {}",
                                report.applied.len(),
                                result.url
                            );
                        }

                        let mut stylesheets = Vec::new();
                        for css_text in &css_texts {
                            stylesheets.push(css::Stylesheet::parse(css_text));
                        }
                        for inline_css in doc.inline_styles() {
                            stylesheets.push(css::Stylesheet::parse(&inline_css));
                        }
                        let layout_tree =
                            layout::LayoutTree::compute(&doc, &stylesheets, width as f32 * 8.0);
                        let page = render::render_document(&doc, Some(&layout_tree), width);
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
