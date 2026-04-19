//! Network fetching -- HTTP client for page and resource loading.
//!
//! Uses reqwest for HTTP/HTTPS with cookie jar, compression,
//! redirect following, and configurable proxy support.

use crate::config::NetworkConfig;
use crate::url_util;
use std::collections::HashMap;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum FetchError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("timeout after {0}s")]
    Timeout(u64),
    #[error("status {status}: {url}")]
    HttpStatus { status: u16, url: String },
    #[error("HTTPS required but got HTTP url: {0}")]
    HttpsRequired(String),
    #[error("blocked by content filter: {0}")]
    Blocked(String),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, FetchError>;

/// Result of an HTTP fetch operation.
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// The final URL after any redirects.
    pub url: String,
    /// HTTP status code.
    pub status: u16,
    /// Content-Type header value.
    pub content_type: String,
    /// Response body as a string.
    pub body: String,
    /// All response headers.
    pub headers: HashMap<String, String>,
}

/// HTTP fetcher for web pages and resources.
pub struct Fetcher {
    client: reqwest::Client,
    config: NetworkConfig,
    /// Whether to enforce HTTPS-only mode.
    https_only: bool,
}

impl Fetcher {
    /// Create a new fetcher with the given network configuration.
    #[must_use]
    pub fn new(config: &NetworkConfig) -> Self {
        Self::with_https(config, false)
    }

    /// Create a new fetcher with HTTPS-only mode control.
    #[must_use]
    pub fn with_https(config: &NetworkConfig, https_only: bool) -> Self {
        let mut builder = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.timeout_secs))
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            })
            .gzip(true)
            .brotli(true)
            .cookie_store(true);

        if let Some(ref proxy_url) = config.proxy {
            match reqwest::Proxy::all(proxy_url) {
                Ok(proxy) => {
                    tracing::info!(proxy = %proxy_url, "using proxy");
                    builder = builder.proxy(proxy);
                }
                Err(e) => {
                    tracing::warn!(proxy = %proxy_url, error = %e, "invalid proxy URL, ignoring");
                }
            }
        }

        let client = builder.build().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to build custom client, using default");
            reqwest::Client::new()
        });

        tracing::debug!(
            user_agent = %config.user_agent,
            timeout = config.timeout_secs,
            follow_redirects = config.follow_redirects,
            https_only,
            "fetcher initialised"
        );

        Self {
            client,
            config: config.clone(),
            https_only,
        }
    }

    /// Fetch a URL and return the full response including headers.
    pub async fn fetch(&self, url: &str) -> Result<FetchResult> {
        // HTTPS-only enforcement.
        if self.https_only && url.starts_with("http://") {
            return Err(FetchError::HttpsRequired(url.to_string()));
        }

        tracing::info!(url, "fetching");

        let response = self.client.get(url).send().await.map_err(|e| {
            if e.is_timeout() {
                FetchError::Timeout(self.config.timeout_secs)
            } else {
                FetchError::Request(e.to_string())
            }
        })?;

        let final_url = response.url().to_string();
        let status = response.status().as_u16();

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|v| (name.as_str().to_string(), v.to_string()))
            })
            .collect();

        tracing::debug!(
            url = %final_url,
            status,
            content_type = %content_type,
            "response received"
        );

        let body = response
            .text()
            .await
            .map_err(|e| FetchError::Request(format!("failed to read response body: {e}")))?;

        Ok(FetchResult {
            url: final_url,
            status,
            content_type,
            body,
            headers,
        })
    }

    /// Fetch a URL and return only the body text.
    pub async fn fetch_text(&self, url: &str) -> Result<String> {
        let result = self.fetch(url).await?;
        Ok(result.body)
    }

    /// Fetch a page and its linked CSS stylesheets.
    ///
    /// Returns the HTML body and a list of CSS stylesheet texts.
    pub async fn fetch_page_with_css(&self, url: &str) -> Result<(FetchResult, Vec<String>)> {
        let page = self.fetch(url).await?;

        // Parse the HTML to find linked stylesheets.
        let doc = crate::dom::Document::parse(&page.body);
        let css_links = doc.stylesheet_links();

        let mut css_texts = Vec::new();
        for css_href in css_links {
            let css_url = url_util::resolve_url(&page.url, &css_href);
            match self.fetch_text(&css_url).await {
                Ok(css_text) => {
                    tracing::debug!(url = %css_url, "fetched stylesheet");
                    css_texts.push(css_text);
                }
                Err(e) => {
                    tracing::warn!(url = %css_url, error = %e, "failed to fetch stylesheet");
                }
            }
        }

        Ok((page, css_texts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> NetworkConfig {
        NetworkConfig {
            user_agent: "nami-test/0.1".to_string(),
            timeout_secs: 10,
            follow_redirects: true,
            proxy: None,
        }
    }

    #[test]
    fn fetcher_creates_with_defaults() {
        let config = test_config();
        let _fetcher = Fetcher::new(&config);
    }

    #[test]
    fn fetcher_creates_with_proxy() {
        let mut config = test_config();
        config.proxy = Some("http://localhost:8080".to_string());
        let _fetcher = Fetcher::new(&config);
    }

    #[test]
    fn fetcher_creates_with_invalid_proxy() {
        let mut config = test_config();
        config.proxy = Some("not-a-valid-proxy-url".to_string());
        let _fetcher = Fetcher::new(&config);
    }

    #[test]
    fn fetcher_creates_no_redirects() {
        let mut config = test_config();
        config.follow_redirects = false;
        let _fetcher = Fetcher::new(&config);
    }

    #[test]
    fn fetch_result_fields() {
        let result = FetchResult {
            url: "https://example.com".to_string(),
            status: 200,
            content_type: "text/html".to_string(),
            body: "<html></html>".to_string(),
            headers: HashMap::from([("content-type".to_string(), "text/html".to_string())]),
        };

        assert_eq!(result.status, 200);
        assert_eq!(result.content_type, "text/html");
        assert!(!result.body.is_empty());
        assert!(result.headers.contains_key("content-type"));
    }

    #[test]
    fn fetcher_with_https_only() {
        let config = test_config();
        let fetcher = Fetcher::with_https(&config, true);
        assert!(fetcher.https_only);
    }
}
