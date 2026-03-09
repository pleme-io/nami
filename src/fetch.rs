//! Network fetching -- HTTP client for page and resource loading.
//!
//! Uses reqwest for HTTP/HTTPS with cookie jar, compression,
//! redirect following, and configurable proxy support.
//! Will use todoku (shared HTTP library) once available.

use crate::config::NetworkConfig;
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
}

impl Fetcher {
    /// Create a new fetcher with the given network configuration.
    ///
    /// Configures the reqwest client with user-agent, timeout, redirect policy,
    /// and optional proxy from the config.
    #[must_use]
    pub fn new(config: &NetworkConfig) -> Self {
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
            "fetcher initialised"
        );

        Self {
            client,
            config: config.clone(),
        }
    }

    /// Fetch a URL and return the full response including headers.
    pub async fn fetch(&self, url: &str) -> Result<FetchResult> {
        tracing::info!(url, "fetching");

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| {
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

        let body = response.text().await.map_err(|e| {
            FetchError::Request(format!("failed to read response body: {e}"))
        })?;

        Ok(FetchResult {
            url: final_url,
            status,
            content_type,
            body,
            headers,
        })
    }

    /// Fetch a URL and return only the body text.
    ///
    /// This is a convenience wrapper around [`fetch`](Self::fetch) that discards
    /// headers and status information.
    pub async fn fetch_text(&self, url: &str) -> Result<String> {
        let result = self.fetch(url).await?;
        Ok(result.body)
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
        // Construction should not panic.
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
        // Should not panic, just warn and skip.
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
            headers: HashMap::from([
                ("content-type".to_string(), "text/html".to_string()),
            ]),
        };

        assert_eq!(result.status, 200);
        assert_eq!(result.content_type, "text/html");
        assert!(!result.body.is_empty());
        assert!(result.headers.contains_key("content-type"));
    }
}
