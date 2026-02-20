//! TMDB (The Movie Database) API client
//!
//! Direct HTTP client for the TMDB API. Used to fetch movie poster and
//! backdrop images given an IMDB ID. Auto-enables when PIR9_TMDB_API_KEY
//! is set; all methods return None gracefully when disabled.

use serde::Deserialize;
use tracing::{debug, warn};

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p";

/// Configuration for the TMDB API
#[derive(Debug, Clone)]
pub struct TmdbConfig {
    pub api_key: String,
    pub enabled: bool,
}

impl Default for TmdbConfig {
    fn default() -> Self {
        let api_key = std::env::var("PIR9_TMDB_API_KEY").unwrap_or_default();
        let enabled = !api_key.is_empty();
        Self { api_key, enabled }
    }
}

/// HTTP client for the TMDB API
#[derive(Clone)]
pub struct TmdbClient {
    config: TmdbConfig,
    client: reqwest::Client,
}

impl std::fmt::Debug for TmdbClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TmdbClient")
            .field("enabled", &self.config.enabled)
            .finish()
    }
}

/// Images resolved from TMDB for a movie
#[derive(Debug, Clone)]
pub struct TmdbMovieImages {
    pub tmdb_id: i64,
    pub poster_url: Option<String>,
    pub fanart_url: Option<String>,
}

// TMDB API response types (internal)

#[derive(Debug, Deserialize)]
struct FindResponse {
    movie_results: Vec<FindMovieResult>,
}

#[derive(Debug, Deserialize)]
struct FindMovieResult {
    id: i64,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
}

impl TmdbClient {
    pub fn from_env() -> Self {
        let config = TmdbConfig::default();
        if config.enabled {
            tracing::info!("TMDB integration enabled");
        }
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Look up a movie on TMDB by its IMDB ID and return poster/backdrop URLs.
    ///
    /// Returns `Ok(None)` when disabled or on any error (graceful degradation).
    pub async fn find_movie_by_imdb_id(&self, imdb_id: &str) -> anyhow::Result<Option<TmdbMovieImages>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let url = format!(
            "{}/find/{}?api_key={}&external_source=imdb_id",
            TMDB_BASE_URL, imdb_id, self.config.api_key
        );

        debug!("TMDB find request: imdb_id={}", imdb_id);

        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                let find: FindResponse = match response.json().await {
                    Ok(f) => f,
                    Err(e) => {
                        warn!("Failed to parse TMDB response for {}: {}", imdb_id, e);
                        return Ok(None);
                    }
                };

                if let Some(movie) = find.movie_results.into_iter().next() {
                    let poster_url = movie.poster_path.map(|p| format!("{}/w500{}", TMDB_IMAGE_BASE, p));
                    let fanart_url = movie.backdrop_path.map(|p| format!("{}/w1280{}", TMDB_IMAGE_BASE, p));

                    debug!(
                        "TMDB found movie: tmdb_id={}, poster={}, fanart={}",
                        movie.id,
                        poster_url.is_some(),
                        fanart_url.is_some()
                    );

                    Ok(Some(TmdbMovieImages {
                        tmdb_id: movie.id,
                        poster_url,
                        fanart_url,
                    }))
                } else {
                    debug!("TMDB returned no movie results for {}", imdb_id);
                    Ok(None)
                }
            }
            Ok(response) => {
                warn!("TMDB API returned error for {}: {}", imdb_id, response.status());
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to connect to TMDB API: {}", e);
                Ok(None)
            }
        }
    }
}
