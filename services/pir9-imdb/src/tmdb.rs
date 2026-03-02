//! TMDB API client for on-demand movie metadata enrichment
//!
//! Fetches TMDB ID + image URLs via the `/find/{imdb_id}` endpoint.
//! Supports both v3 API keys and v4 Bearer tokens (auto-detected).

use serde::Deserialize;
use tracing::{debug, warn};

/// Cached TMDB data for a movie
pub struct TmdbMovieData {
    pub tmdb_id: i64,
    pub poster_url: Option<String>,
    pub fanart_url: Option<String>,
}

/// TMDB API client
pub struct TmdbClient {
    client: reqwest::Client,
    api_key: String,
    /// true = Bearer token (v4), false = query param (v3)
    is_bearer: bool,
}

impl TmdbClient {
    /// Create from `PIR9_TMDB_API_KEY` env var. Returns None if absent or empty.
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("PIR9_TMDB_API_KEY").ok().filter(|s| !s.is_empty())?;
        let is_bearer = key.starts_with("eyJ");

        let client = reqwest::Client::builder()
            .user_agent("pir9-imdb/0.5.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .ok()?;

        Some(Self {
            client,
            api_key: key,
            is_bearer,
        })
    }

    /// Look up a movie on TMDB by its IMDB ID.
    /// Returns None if no match found or API fails.
    pub async fn find_movie_by_imdb_id(&self, imdb_id: &str) -> Option<TmdbMovieData> {
        let url = format!(
            "https://api.themoviedb.org/3/find/{}?external_source=imdb_id",
            imdb_id
        );

        let req = if self.is_bearer {
            self.client.get(&url).bearer_auth(&self.api_key)
        } else {
            self.client.get(&url).query(&[("api_key", &self.api_key)])
        };

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                warn!("TMDB request failed for {}: {}", imdb_id, e);
                return None;
            }
        };

        if !resp.status().is_success() {
            debug!("TMDB returned {} for {}", resp.status(), imdb_id);
            return None;
        }

        let find: TmdbFindResponse = resp.json().await.ok()?;
        let movie = find.movie_results.into_iter().next()?;

        let poster_url = movie
            .poster_path
            .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p));
        let fanart_url = movie
            .backdrop_path
            .map(|p| format!("https://image.tmdb.org/t/p/w1280{}", p));

        Some(TmdbMovieData {
            tmdb_id: movie.id,
            poster_url,
            fanart_url,
        })
    }
}

#[derive(Deserialize)]
struct TmdbFindResponse {
    movie_results: Vec<TmdbMovieResult>,
}

#[derive(Deserialize)]
struct TmdbMovieResult {
    id: i64,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
}
