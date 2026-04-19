//! Blocking implementation of `nami_core::query::Fetcher` via reqwest.
//!
//! `(defquery …)` specs are synchronous by contract — they're
//! triggered during the navigate pipeline alongside predicates +
//! effects, all of which run on a single thread. Using reqwest's
//! blocking client sidesteps the async/sync mismatch between
//! nami-core's sync Fetcher trait and our tokio browser runtime.
//!
//! Built once per invocation (queries are infrequent; per-call client
//! allocation is dwarfed by TLS handshakes) so users get a cold-start
//! behaviour that's easy to reason about + honours cookie-free
//! per-query semantics. The browser's main `fetch::Fetcher` remains
//! the async hot path for actual page fetches.

use nami_core::query::{Fetcher, HeaderPair};
use reqwest::blocking::Client;
use std::time::Duration;

/// Thin reqwest-blocking shim implementing the nami-core Fetcher
/// trait. Defaults: 15-second timeout, rustls TLS, gzip + brotli
/// transparent decoding, redirect following.
pub struct BlockingFetcher {
    client: Client,
    user_agent: String,
}

impl BlockingFetcher {
    /// Build with nami's network config for consistent UA + timeout.
    #[must_use]
    pub fn new(cfg: &crate::config::NetworkConfig) -> Self {
        let client = Client::builder()
            .user_agent(&cfg.user_agent)
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("failed to build blocking fetcher client: {e}; using default");
                Client::new()
            });
        Self {
            client,
            user_agent: cfg.user_agent.clone(),
        }
    }
}

impl Fetcher for BlockingFetcher {
    fn fetch(
        &self,
        url: &str,
        method: &str,
        body: Option<&str>,
        headers: &[HeaderPair],
    ) -> Result<String, String> {
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|e| format!("invalid method {method:?}: {e}"))?;

        let mut req = self.client.request(method, url);
        req = req.header("user-agent", &self.user_agent);
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if let Some(b) = body {
            req = req.body(b.to_string());
        }

        let resp = req.send().map_err(|e| format!("send: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            tracing::debug!("query fetch non-2xx status={status}");
        }
        resp.text().map_err(|e| format!("read body: {e}"))
    }
}
