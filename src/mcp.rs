//! MCP server for Nami (Aranami) TUI browser via kaname.
//!
//! Exposes browser tools over the Model Context Protocol (stdio transport),
//! allowing AI assistants to navigate pages, extract content, and manage bookmarks.

use kaname::ToolResponse;
use kaname::rmcp;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::bookmarks::Bookmarks;
use crate::config;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct NavigateRequest {
    /// URL to navigate to.
    url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetDomRequest {
    /// CSS selector to narrow the DOM subtree. Omit for the full document.
    selector: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetTextRequest {
    /// CSS selector to extract text from. Omit for the full page text.
    selector: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AddBookmarkRequest {
    /// URL to bookmark.
    url: String,
    /// Bookmark title. Defaults to the URL.
    title: Option<String>,
    /// Tags for the bookmark.
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ConfigGetRequest {
    /// Config key (dot-separated path).
    key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ConfigSetRequest {
    /// Config key.
    key: String,
    /// Value to set (as string).
    value: String,
}

// ---------------------------------------------------------------------------
// MCP Service
// ---------------------------------------------------------------------------

/// Nami browser MCP server.
pub struct NamiMcpServer {
    tool_router: ToolRouter<Self>,
    config: config::NamiConfig,
}

impl std::fmt::Debug for NamiMcpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamiMcpServer").finish()
    }
}

#[tool_router]
impl NamiMcpServer {
    pub fn new(config: config::NamiConfig) -> Self {
        Self {
            tool_router: Self::tool_router(),
            config,
        }
    }

    // -- Standard tools --

    #[tool(description = "Get Nami browser status.")]
    async fn status(&self) -> Result<CallToolResult, McpError> {
        let bookmarks_path = self
            .config
            .bookmarks_file
            .clone()
            .unwrap_or_else(config::default_bookmarks_path);
        let bm = Bookmarks::load(&bookmarks_path);
        Ok(ToolResponse::success(&serde_json::json!({
            "status": "running",
            "homepage": self.config.homepage,
            "bookmarks_count": bm.all().len(),
        })))
    }

    #[tool(description = "Get the Nami version.")]
    async fn version(&self) -> Result<CallToolResult, McpError> {
        Ok(ToolResponse::success(&serde_json::json!({
            "name": "nami",
            "crate": "aranami",
            "version": env!("CARGO_PKG_VERSION"),
        })))
    }

    #[tool(description = "Get a configuration value by key.")]
    async fn config_get(
        &self,
        Parameters(req): Parameters<ConfigGetRequest>,
    ) -> Result<CallToolResult, McpError> {
        let json = serde_json::to_value(&self.config).unwrap_or_default();
        let value = req
            .key
            .split('.')
            .fold(Some(&json), |v, k| v.and_then(|v| v.get(k)));
        match value {
            Some(v) => Ok(ToolResponse::success(v)),
            None => Ok(ToolResponse::error(&format!("Key '{}' not found", req.key))),
        }
    }

    #[tool(description = "Set a configuration value (runtime only, not persisted).")]
    async fn config_set(
        &self,
        Parameters(req): Parameters<ConfigSetRequest>,
    ) -> Result<CallToolResult, McpError> {
        Ok(ToolResponse::text(&format!(
            "Config key '{}' would be set to '{}'. Runtime config mutation not yet supported; \
             edit ~/.config/nami/nami.yaml instead.",
            req.key, req.value
        )))
    }

    // -- App-specific tools --

    #[tool(description = "Navigate to a URL and return the page content as plain text.")]
    async fn navigate(
        &self,
        Parameters(req): Parameters<NavigateRequest>,
    ) -> Result<CallToolResult, McpError> {
        let fetcher = crate::fetch::Fetcher::new(&self.config.network);
        match fetcher.fetch(&req.url).await {
            Ok(result) => {
                let doc = crate::dom::Document::parse(&result.body);
                let text = crate::dom::node_to_text(&doc.root, 0);
                let truncated = if text.len() > 8000 {
                    format!("{}... [truncated]", &text[..8000])
                } else {
                    text
                };
                Ok(ToolResponse::success(&serde_json::json!({
                    "url": req.url,
                    "status": result.status,
                    "content_type": result.content_type,
                    "text": truncated,
                })))
            }
            Err(e) => Ok(ToolResponse::error(&format!("Navigation failed: {e}"))),
        }
    }

    #[tool(
        description = "Get the DOM structure of a fetched page. Returns a simplified JSON representation."
    )]
    async fn get_dom(
        &self,
        Parameters(req): Parameters<GetDomRequest>,
    ) -> Result<CallToolResult, McpError> {
        // DOM inspection requires an active page; provide a helpful message
        Ok(ToolResponse::text(&format!(
            "DOM inspection for selector {:?} requires an active browsing session. \
             Use the 'navigate' tool first to load a page, then use 'get_text' to extract content.",
            req.selector.as_deref().unwrap_or("*")
        )))
    }

    #[tool(description = "Get all links on the current page. Requires a prior navigate call.")]
    async fn get_links(&self) -> Result<CallToolResult, McpError> {
        Ok(ToolResponse::text(
            "Link extraction requires an active browsing session. \
             Use 'navigate' to load a page first. In the current MCP server mode, \
             use 'navigate' and parse the returned text for links.",
        ))
    }

    #[tool(description = "Extract text content from a page. Optionally filter by CSS selector.")]
    async fn get_text(
        &self,
        Parameters(req): Parameters<GetTextRequest>,
    ) -> Result<CallToolResult, McpError> {
        Ok(ToolResponse::text(&format!(
            "Text extraction for selector {:?} requires an active page. \
             Use the 'navigate' tool which returns page text content directly.",
            req.selector.as_deref().unwrap_or("body")
        )))
    }

    #[tool(
        description = "Take a screenshot of the current viewport. Returns status since screenshot requires GPU."
    )]
    async fn screenshot(&self) -> Result<CallToolResult, McpError> {
        Ok(ToolResponse::text(
            "Screenshot requires the GPU rendering pipeline. Use 'navigate' to get page content as text instead.",
        ))
    }

    #[tool(description = "Add a URL to bookmarks.")]
    async fn add_bookmark(
        &self,
        Parameters(req): Parameters<AddBookmarkRequest>,
    ) -> Result<CallToolResult, McpError> {
        let bookmarks_path = self
            .config
            .bookmarks_file
            .clone()
            .unwrap_or_else(config::default_bookmarks_path);
        let mut bm = Bookmarks::load(&bookmarks_path);
        let title = req.title.unwrap_or_else(|| req.url.clone());
        let tags = req.tags.unwrap_or_default();
        if bm.add(&req.url, &title, tags.clone()) {
            match bm.save() {
                Ok(()) => Ok(ToolResponse::success(&serde_json::json!({
                    "added": true,
                    "url": req.url,
                    "title": title,
                    "tags": tags,
                }))),
                Err(e) => Ok(ToolResponse::error(&format!("Save failed: {e}"))),
            }
        } else {
            Ok(ToolResponse::text(&format!(
                "Already bookmarked: {}",
                req.url
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// ServerHandler
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for NamiMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: rmcp::model::Implementation {
                name: "nami".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: None,
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Nami TUI browser MCP server. Navigate web pages, extract content, \
                 and manage bookmarks."
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the MCP server on stdio.
pub async fn run(config: config::NamiConfig) -> Result<(), Box<dyn std::error::Error>> {
    use rmcp::{ServiceExt, transport::stdio};

    let service = NamiMcpServer::new(config);
    let server = service.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
