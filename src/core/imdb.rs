//! IMDB service client
//!
//! HTTP client for the pir9-imdb microservice, which maintains a local
//! PostgreSQL database of IMDB data (series, episodes, ratings).
//! This client is shared via AppState so all layers can query IMDB.

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Configuration for the pir9-imdb microservice
#[derive(Debug, Clone)]
pub struct ImdbServiceConfig {
    pub base_url: String,
    pub enabled: bool,
}

impl Default for ImdbServiceConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var("PIR9_IMDB_SERVICE_URL")
                .unwrap_or_else(|_| "http://pir9-imdb:8990".to_string()),
            enabled: std::env::var("PIR9_IMDB_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }
    }
}

/// HTTP client for the pir9-imdb microservice
#[derive(Clone)]
pub struct ImdbClient {
    pub config: ImdbServiceConfig,
    client: reqwest::Client,
}

impl std::fmt::Debug for ImdbClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImdbClient")
            .field("config", &self.config)
            .finish()
    }
}

impl ImdbClient {
    pub fn from_env() -> Self {
        Self {
            config: ImdbServiceConfig::default(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub async fn get_series(&self, imdb_id: &str) -> anyhow::Result<Option<ImdbSeries>> {
        if !self.config.enabled {
            return Ok(None);
        }
        let url = format!("{}/api/series/{}", self.config.base_url, imdb_id);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(response) if response.status().as_u16() == 404 => Ok(None),
            Ok(response) => {
                warn!("IMDB service returned error: {}", response.status());
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to connect to IMDB service: {}", e);
                Ok(None)
            }
        }
    }

    pub async fn search_series(&self, query: &str, limit: u32) -> anyhow::Result<Vec<ImdbSeries>> {
        if !self.config.enabled {
            return Ok(vec![]);
        }
        let url = format!(
            "{}/api/series/search?q={}&limit={}",
            self.config.base_url,
            urlencoding::encode(query),
            limit
        );
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(_) | Err(_) => Ok(vec![]),
        }
    }

    pub async fn get_episodes(&self, imdb_id: &str) -> anyhow::Result<Vec<ImdbEpisode>> {
        if !self.config.enabled {
            return Ok(vec![]);
        }
        let url = format!("{}/api/series/{}/episodes", self.config.base_url, imdb_id);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(_) | Err(_) => Ok(vec![]),
        }
    }

    pub async fn get_movie(&self, imdb_id: &str) -> anyhow::Result<Option<ImdbMovie>> {
        if !self.config.enabled {
            return Ok(None);
        }
        let url = format!("{}/api/movies/{}", self.config.base_url, imdb_id);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(response) if response.status().as_u16() == 404 => Ok(None),
            Ok(response) => {
                warn!("IMDB service returned error: {}", response.status());
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to connect to IMDB service: {}", e);
                Ok(None)
            }
        }
    }

    pub async fn search_movies(&self, query: &str, limit: u32) -> anyhow::Result<Vec<ImdbMovie>> {
        if !self.config.enabled {
            return Ok(vec![]);
        }
        let url = format!(
            "{}/api/movies/search?q={}&limit={}",
            self.config.base_url,
            urlencoding::encode(query),
            limit
        );
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(_) | Err(_) => Ok(vec![]),
        }
    }

    pub async fn get_stats(&self) -> anyhow::Result<Option<ImdbStats>> {
        if !self.config.enabled {
            return Ok(None);
        }
        let url = format!("{}/api/stats", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(_) | Err(_) => Ok(None),
        }
    }

    pub async fn start_sync(&self) -> anyhow::Result<ImdbProxyResponse> {
        self.start_sync_selective(&[]).await
    }

    pub async fn start_sync_selective(&self, datasets: &[String]) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/sync", self.config.base_url);
        match self
            .client
            .post(&url)
            .json(&serde_json::json!({ "datasets": datasets }))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }

    pub async fn get_sync_status(&self) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/sync/status", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }

    pub async fn backfill_air_dates(&self, limit: u32) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/backfill-air-dates", self.config.base_url);
        match self
            .client
            .post(&url)
            .json(&serde_json::json!({ "limit": limit }))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }

    /// Get credits (cast/crew) for a title by IMDB ID
    pub async fn get_title_credits(&self, imdb_id: &str) -> anyhow::Result<Option<TitleCredits>> {
        if !self.config.enabled {
            return Ok(None);
        }
        let url = format!("{}/api/movies/{}/credits", self.config.base_url, imdb_id);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(response) if response.status().as_u16() == 404 => Ok(None),
            Ok(response) => {
                warn!("IMDB service returned error for credits: {}", response.status());
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to connect to IMDB service for credits: {}", e);
                Ok(None)
            }
        }
    }

    /// Get dataset metadata (sizes, cache status) from the IMDB service
    pub async fn get_datasets(&self) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/datasets", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }

    /// Download datasets to cache without processing
    pub async fn start_download(&self, datasets: &[String]) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/download", self.config.base_url);
        match self
            .client
            .post(&url)
            .json(&serde_json::json!({ "datasets": datasets }))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }

    /// Process cached datasets without re-downloading
    pub async fn start_process(&self, datasets: &[String]) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/process", self.config.base_url);
        match self
            .client
            .post(&url)
            .json(&serde_json::json!({ "datasets": datasets }))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }

    pub async fn cancel_sync(&self) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/sync/cancel", self.config.base_url);
        match self.client.post(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }
}

pub struct ImdbProxyResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbSeries {
    pub imdb_id: String,
    pub title: String,
    pub original_title: Option<String>,
    pub start_year: Option<i32>,
    pub end_year: Option<i32>,
    pub runtime_minutes: Option<i32>,
    pub genres: Vec<String>,
    pub is_adult: bool,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbEpisode {
    pub imdb_id: String,
    pub parent_imdb_id: String,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub title: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    pub air_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbMovie {
    pub imdb_id: String,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub runtime_minutes: Option<i32>,
    pub genres: Vec<String>,
    pub is_adult: bool,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    /// TMDB ID (cached by pir9-imdb service)
    #[serde(default)]
    pub tmdb_id: Option<i64>,
    /// Poster image URL (cached from TMDB)
    #[serde(default)]
    pub poster_url: Option<String>,
    /// Fanart/backdrop image URL (cached from TMDB)
    #[serde(default)]
    pub fanart_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbStats {
    pub series_count: i64,
    pub episode_count: i64,
    #[serde(default)]
    pub movie_count: i64,
    #[serde(default)]
    pub people_count: i64,
    #[serde(default)]
    pub credits_count: i64,
    pub last_sync: Option<String>,
    #[serde(default)]
    pub db_size_bytes: i64,
}

/// A single credit entry (cast/crew member) for a title
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbCredit {
    pub nconst: String,
    pub name: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters: Option<Vec<String>>,
    pub ordering: i16,
}

/// Credits for a title (movie or series)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleCredits {
    pub imdb_id: String,
    pub credits: Vec<ImdbCredit>,
}
