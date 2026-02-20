//! IMDB Service Client
//!
//! The IMDB data is managed by a separate microservice (pir9-imdb).
//! This module provides a client to interact with that service.
//!
//! Benefits of separate service:
//! - Large dataset (millions of rows) doesn't affect main app
//! - Independent scaling and deployment
//! - Can use optimized database (SQLite, PostgreSQL, or specialized)
//! - No impact on main app during bulk imports/syncs

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};

/// Configuration for the IMDB service
#[derive(Debug, Clone)]
pub struct ImdbServiceConfig {
    /// Base URL of the IMDB service (e.g., "http://pir9-imdb:8990")
    pub base_url: String,
    /// Whether the IMDB service is enabled
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

/// Client for the IMDB service
#[derive(Clone)]
pub struct ImdbClient {
    config: ImdbServiceConfig,
    client: reqwest::Client,
}

impl ImdbClient {
    /// Create a new IMDB service client
    pub fn new(config: ImdbServiceConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Create with default configuration
    pub fn from_env() -> Self {
        Self::new(ImdbServiceConfig::default())
    }

    /// Check if the IMDB service is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Look up a series by IMDB ID
    pub async fn get_series(&self, imdb_id: &str) -> Result<Option<ImdbSeries>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let url = format!("{}/api/series/{}", self.config.base_url, imdb_id);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.ok())
            }
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

    /// Search for series by title
    pub async fn search_series(&self, query: &str, limit: u32) -> Result<Vec<ImdbSeries>> {
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

    /// Get episodes for a series
    pub async fn get_episodes(&self, imdb_id: &str) -> Result<Vec<ImdbEpisode>> {
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

    /// Health check for the IMDB service
    pub async fn health_check(&self) -> Result<bool> {
        if !self.config.enabled {
            return Ok(false);
        }

        let url = format!("{}/health", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Get service statistics
    pub async fn get_stats(&self) -> Result<Option<ImdbStats>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let url = format!("{}/api/stats", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.ok())
            }
            Ok(_) | Err(_) => Ok(None),
        }
    }

    /// Start a full IMDB sync on the service
    pub async fn start_sync(&self) -> Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }

        let url = format!("{}/api/sync", self.config.base_url);
        match self.client.post(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response.json().await.unwrap_or_else(|_| {
                    serde_json::json!({ "error": "Failed to parse response" })
                });
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => {
                warn!("Failed to connect to IMDB service for sync: {}", e);
                Ok(ImdbProxyResponse {
                    status: 502,
                    body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
                })
            }
        }
    }

    /// Get sync status from the service
    pub async fn get_sync_status(&self) -> Result<ImdbProxyResponse> {
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
                let body: serde_json::Value = response.json().await.unwrap_or_else(|_| {
                    serde_json::json!({ "error": "Failed to parse response" })
                });
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => {
                warn!("Failed to get IMDB sync status: {}", e);
                Ok(ImdbProxyResponse {
                    status: 502,
                    body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
                })
            }
        }
    }

    /// Cancel running syncs on the service
    pub async fn cancel_sync(&self) -> Result<ImdbProxyResponse> {
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
                let body: serde_json::Value = response.json().await.unwrap_or_else(|_| {
                    serde_json::json!({ "error": "Failed to parse response" })
                });
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => {
                warn!("Failed to cancel IMDB sync: {}", e);
                Ok(ImdbProxyResponse {
                    status: 502,
                    body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
                })
            }
        }
    }
}

/// Response from proxied IMDB service calls
pub struct ImdbProxyResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

/// IMDB Series data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// IMDB Episode data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// IMDB Service statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImdbStats {
    pub series_count: i64,
    pub episode_count: i64,
    pub last_sync: Option<String>,
    pub db_size_bytes: Option<i64>,
}

// ============================================================================
// Legacy compatibility - these types/functions were used by other modules
// They now delegate to the client
// ============================================================================

/// Legacy database wrapper - now wraps the client
#[derive(Clone)]
pub struct ImdbDatabase {
    client: ImdbClient,
}

impl ImdbDatabase {
    /// Connect to the IMDB service (legacy API)
    pub async fn connect(_path: &str) -> Result<Self> {
        info!("IMDB data is now provided by external pir9-imdb service");
        Ok(Self {
            client: ImdbClient::from_env(),
        })
    }

    /// Connect for import - not supported with external service
    pub async fn connect_for_import(_path: &str) -> Result<Self> {
        info!("IMDB imports are handled by external pir9-imdb service");
        Ok(Self {
            client: ImdbClient::from_env(),
        })
    }

    /// Get stats from the IMDB service
    pub async fn get_stats(&self) -> Result<ImdbDbStats> {
        if let Some(stats) = self.client.get_stats().await? {
            Ok(ImdbDbStats {
                series_count: stats.series_count,
                episode_count: stats.episode_count,
                db_size_bytes: stats.db_size_bytes.unwrap_or(0),
            })
        } else {
            Ok(ImdbDbStats {
                series_count: 0,
                episode_count: 0,
                db_size_bytes: 0,
            })
        }
    }

    /// Optimize - no-op for external service
    pub async fn optimize(&self) -> Result<()> {
        Ok(())
    }
}

/// Legacy stats struct
#[derive(Debug)]
pub struct ImdbDbStats {
    pub series_count: i64,
    pub episode_count: i64,
    pub db_size_bytes: i64,
}

/// Default path constant (kept for config compatibility)
pub const DEFAULT_IMDB_DB_PATH: &str = "/config/imdb.db";
