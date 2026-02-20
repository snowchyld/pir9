//! TVMaze API client
//!
//! Conditional fallback for network metadata. When Skyhook returns no network
//! for a series, TVMaze is queried to fill it in. TVMaze is a free API
//! (no key required) and has broad coverage of TV networks and streaming services.

use serde::Deserialize;
use tracing::debug;

/// HTTP client for the TVMaze public API
#[derive(Clone)]
pub struct TvMazeClient {
    client: reqwest::Client,
}

impl std::fmt::Debug for TvMazeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TvMazeClient").finish()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TvMazeShow {
    network: Option<TvMazeNetwork>,
    web_channel: Option<TvMazeNetwork>,
}

#[derive(Deserialize)]
struct TvMazeNetwork {
    name: String,
}

impl TvMazeClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent(format!("pir9/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Look up a show by TVDB ID. Returns `None` on any error or 404.
    async fn lookup_by_tvdb(&self, tvdb_id: i64) -> Option<TvMazeShow> {
        let url = format!("https://api.tvmaze.com/lookup/shows?thetvdb={}", tvdb_id);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => resp.json().await.ok(),
            Ok(resp) => {
                debug!("TVMaze TVDB lookup returned {}", resp.status());
                None
            }
            Err(e) => {
                debug!("TVMaze TVDB lookup failed: {}", e);
                None
            }
        }
    }

    /// Look up a show by IMDB ID. Returns `None` on any error or 404.
    async fn lookup_by_imdb(&self, imdb_id: &str) -> Option<TvMazeShow> {
        let url = format!("https://api.tvmaze.com/lookup/shows?imdb={}", imdb_id);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => resp.json().await.ok(),
            Ok(resp) => {
                debug!("TVMaze IMDB lookup returned {}", resp.status());
                None
            }
            Err(e) => {
                debug!("TVMaze IMDB lookup failed: {}", e);
                None
            }
        }
    }

    /// Get network name for a show, trying TVDB ID first, then IMDB ID.
    ///
    /// Returns the `network.name` for broadcast shows, or `webChannel.name`
    /// for streaming services (e.g., Netflix, Apple TV+).
    pub async fn get_network_name(&self, tvdb_id: i64, imdb_id: Option<&str>) -> Option<String> {
        // Try TVDB lookup first (most reliable cross-reference)
        if let Some(show) = self.lookup_by_tvdb(tvdb_id).await {
            if let Some(name) = extract_network(&show) {
                debug!("TVMaze network for tvdb_id={}: {}", tvdb_id, name);
                return Some(name);
            }
        }

        // Fall back to IMDB lookup
        if let Some(imdb_id) = imdb_id {
            if let Some(show) = self.lookup_by_imdb(imdb_id).await {
                if let Some(name) = extract_network(&show) {
                    debug!("TVMaze network for imdb_id={}: {}", imdb_id, name);
                    return Some(name);
                }
            }
        }

        None
    }
}

/// Extract network name from a TVMaze show response.
/// Prefers `network` (broadcast) over `webChannel` (streaming).
fn extract_network(show: &TvMazeShow) -> Option<String> {
    show.network
        .as_ref()
        .map(|n| n.name.clone())
        .or_else(|| show.web_channel.as_ref().map(|n| n.name.clone()))
}
