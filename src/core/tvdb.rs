//! TVDB v4 API client
//!
//! Fetches episode data with alternate orderings (DVD, Absolute, etc.)
//! directly from the TVDB v4 API. Used when a series has a non-default
//! `episode_ordering` to remap episode numbers after the Skyhook sync.
//!
//! Authentication: JWT token via `POST /v4/login` with API key from
//! `PIR9_TVDB_API_KEY` environment variable. Token cached for 25 days.

use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

const TVDB_BASE_URL: &str = "https://api4.thetvdb.com/v4";

/// Cached JWT token with expiry timestamp
struct CachedToken {
    token: String,
    /// When the token was obtained — we re-login after 25 days
    obtained_at: DateTime<Utc>,
}

impl CachedToken {
    /// Tokens are valid for ~28 days; refresh after 25 to be safe
    fn is_expired(&self) -> bool {
        let age = Utc::now() - self.obtained_at;
        age.num_days() >= 25
    }
}

/// HTTP client for the TVDB v4 API
#[derive(Clone)]
pub struct TvdbClient {
    client: reqwest::Client,
    api_key: Option<String>,
    token: Arc<RwLock<Option<CachedToken>>>,
}

impl std::fmt::Debug for TvdbClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TvdbClient")
            .field("enabled", &self.api_key.is_some())
            .finish()
    }
}

/// A single episode from the TVDB v4 episodes endpoint
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TvdbEpisode {
    /// TVDB episode ID (stable across orderings)
    pub id: i64,
    /// Season number in this ordering
    #[serde(default)]
    pub season_number: i32,
    /// Episode number within the season in this ordering
    #[serde(default, alias = "number")]
    pub episode_number: i32,
}

/// Login request body
#[derive(Serialize)]
struct LoginRequest {
    apikey: String,
}

/// Login response
#[derive(Deserialize)]
struct LoginResponse {
    data: Option<LoginData>,
}

#[derive(Deserialize)]
struct LoginData {
    token: Option<String>,
}

#[derive(Deserialize)]
struct EpisodesData {
    episodes: Option<Vec<TvdbEpisode>>,
}

/// Response wrapper with pagination links
#[derive(Deserialize)]
struct PaginatedResponse {
    data: Option<EpisodesData>,
    links: Option<PaginationLinks>,
}

#[derive(Deserialize)]
struct PaginationLinks {
    next: Option<String>,
}

/// Valid TVDB season types for episode ordering
pub const VALID_ORDERINGS: &[&str] = &["aired", "dvd", "absolute", "alternate", "regional", "altdvd"];

/// Map our ordering names to TVDB v4 season type slugs
fn ordering_to_season_type(ordering: &str) -> &str {
    match ordering {
        "aired" => "official",
        "dvd" => "dvd",
        "absolute" => "absolute",
        "alternate" => "alternate",
        "regional" => "regional",
        "altdvd" => "altdvd",
        _ => "official",
    }
}

impl TvdbClient {
    /// Create a client from the `PIR9_TVDB_API_KEY` environment variable.
    /// Returns a client with `is_enabled() == false` if the key is not set.
    pub fn from_env() -> Self {
        let api_key = std::env::var("PIR9_TVDB_API_KEY").ok().filter(|k| !k.is_empty());
        if api_key.is_some() {
            info!("TVDB v4 client enabled");
        } else {
            debug!("TVDB v4 client disabled (PIR9_TVDB_API_KEY not set)");
        }
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent(format!("pir9/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("Failed to create HTTP client"),
            api_key,
            token: Arc::new(RwLock::new(None)),
        }
    }

    /// Whether the TVDB client has an API key configured
    pub fn is_enabled(&self) -> bool {
        self.api_key.is_some()
    }

    /// Ensure we have a valid JWT token, logging in if needed.
    /// Returns the Bearer token string.
    async fn ensure_token(&self) -> Result<String> {
        // Fast path: check if cached token is still valid
        {
            let guard = self.token.read().await;
            if let Some(ref cached) = *guard {
                if !cached.is_expired() {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Slow path: need to login
        let api_key = self
            .api_key
            .as_ref()
            .context("TVDB API key not configured")?;

        debug!("TVDB: logging in to obtain JWT token");

        let resp = self
            .client
            .post(format!("{}/login", TVDB_BASE_URL))
            .json(&LoginRequest {
                apikey: api_key.clone(),
            })
            .send()
            .await
            .context("TVDB login request failed")?;

        if !resp.status().is_success() {
            anyhow::bail!("TVDB login failed with status: {}", resp.status());
        }

        let login: LoginResponse = resp.json().await.context("Failed to parse TVDB login response")?;

        let token = login
            .data
            .and_then(|d| d.token)
            .context("TVDB login response missing token")?;

        // Cache the token
        let mut guard = self.token.write().await;
        *guard = Some(CachedToken {
            token: token.clone(),
            obtained_at: Utc::now(),
        });

        info!("TVDB: obtained JWT token");
        Ok(token)
    }

    /// Fetch all episodes for a series in a specific ordering.
    ///
    /// Handles TVDB v4 pagination automatically.
    /// Returns `Ok(None)` if the API key is not configured or the ordering
    /// is "aired" (no remapping needed).
    pub async fn get_episodes_by_ordering(
        &self,
        tvdb_series_id: i64,
        ordering: &str,
    ) -> Result<Option<Vec<TvdbEpisode>>> {
        if !self.is_enabled() {
            return Ok(None);
        }

        if ordering == "aired" {
            return Ok(None);
        }

        let season_type = ordering_to_season_type(ordering);
        let token = self.ensure_token().await?;

        let mut all_episodes = Vec::new();
        let mut page = 0;

        loop {
            let url = format!(
                "{}/series/{}/episodes/{}?page={}",
                TVDB_BASE_URL, tvdb_series_id, season_type, page
            );

            debug!("TVDB: fetching {}", url);

            let resp = self
                .client
                .get(&url)
                .bearer_auth(&token)
                .send()
                .await
                .context("TVDB episodes request failed")?;

            if !resp.status().is_success() {
                if resp.status().as_u16() == 404 {
                    warn!(
                        "TVDB: no {} ordering found for series {}",
                        season_type, tvdb_series_id
                    );
                    return Ok(None);
                }
                anyhow::bail!(
                    "TVDB episodes request failed with status: {}",
                    resp.status()
                );
            }

            let paginated: PaginatedResponse =
                resp.json().await.context("Failed to parse TVDB episodes response")?;

            if let Some(data) = paginated.data {
                if let Some(episodes) = data.episodes {
                    all_episodes.extend(episodes);
                }
            }

            // Check if there's a next page
            match paginated.links.and_then(|l| l.next) {
                Some(next) if !next.is_empty() && next != "null" => {
                    page += 1;
                }
                _ => break,
            }
        }

        info!(
            "TVDB: fetched {} episodes in {} ordering for series {}",
            all_episodes.len(),
            season_type,
            tvdb_series_id
        );

        Ok(Some(all_episodes))
    }
}
