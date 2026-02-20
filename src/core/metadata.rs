//! Unified metadata service
//!
//! Orchestrates metadata fetching from IMDB (preferred) and Skyhook (fallback).
//! Defines Skyhook response types once (eliminating duplicates across the codebase)
//! and merges data from both sources using an IMDB-first strategy.

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::info;

use crate::core::imdb::ImdbClient;

/// Base URL for Skyhook metadata service
const SKYHOOK_BASE_URL: &str = "http://skyhook.sonarr.tv/v1/tvdb";

/// HTTP client for fetching series metadata from IMDB + Skyhook
#[derive(Clone)]
pub struct MetadataService {
    imdb_client: ImdbClient,
    http_client: reqwest::Client,
}

impl std::fmt::Debug for MetadataService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetadataService")
            .field("imdb_enabled", &self.imdb_client.is_enabled())
            .finish()
    }
}

impl MetadataService {
    pub fn new(imdb_client: ImdbClient) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent(format!("pir9/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            imdb_client,
            http_client,
        }
    }

    /// Fetch series metadata, merging IMDB + Skyhook data.
    ///
    /// If IMDB is enabled and an imdb_id is provided, fetches from both
    /// sources in parallel and merges (IMDB wins for ratings/genres/runtime/year,
    /// Skyhook wins for overview/images/episodes/network/status).
    pub async fn fetch_series_metadata(
        &self,
        tvdb_id: i64,
        imdb_id: Option<&str>,
    ) -> Result<MergedSeriesMetadata> {
        let skyhook_fut = self.fetch_from_skyhook(tvdb_id);

        // If IMDB is enabled and we have an imdb_id, fetch in parallel
        if self.imdb_client.is_enabled() {
            if let Some(imdb_id) = imdb_id {
                let imdb_fut = self.imdb_client.get_series(imdb_id);
                let (skyhook_result, imdb_result) = tokio::join!(skyhook_fut, imdb_fut);

                let skyhook = skyhook_result?;
                let imdb_series = imdb_result.ok().flatten();

                if let Some(ref imdb) = imdb_series {
                    info!(
                        "Fetched IMDB ratings for {} ({}): {}/10 ({} votes)",
                        skyhook.title,
                        imdb.imdb_id,
                        imdb.rating.unwrap_or(0.0),
                        imdb.votes.unwrap_or(0)
                    );
                }

                return Ok(Self::merge_metadata(skyhook, imdb_series));
            }
        }

        // Skyhook only
        let skyhook = skyhook_fut.await?;
        Ok(Self::merge_metadata(skyhook, None))
    }

    /// Enrich local episodes with IMDB ratings data.
    ///
    /// Fetches episodes from the IMDB service and matches them by
    /// (season_number, episode_number). Returns the number of episodes enriched.
    pub async fn enrich_episodes_with_imdb(
        &self,
        imdb_id: &str,
        episodes: &mut [EpisodeEnrichment],
    ) -> Result<usize> {
        if !self.imdb_client.is_enabled() {
            return Ok(0);
        }

        let imdb_episodes = self.imdb_client.get_episodes(imdb_id).await?;
        if imdb_episodes.is_empty() {
            return Ok(0);
        }

        // Build a lookup map: (season, episode) -> IMDB data
        let mut lookup: std::collections::HashMap<(i32, i32), &crate::core::imdb::ImdbEpisode> =
            std::collections::HashMap::new();
        for ep in &imdb_episodes {
            if let (Some(s), Some(e)) = (ep.season_number, ep.episode_number) {
                lookup.insert((s, e), ep);
            }
        }

        let mut enriched = 0;
        for ep in episodes.iter_mut() {
            if let Some(imdb_ep) = lookup.get(&(ep.season_number, ep.episode_number)) {
                ep.imdb_id = Some(imdb_ep.imdb_id.clone());
                ep.imdb_rating = imdb_ep.rating.map(|r| r as f32);
                ep.imdb_votes = imdb_ep.votes.map(|v| v as i32);
                enriched += 1;
            }
        }

        if enriched > 0 {
            info!(
                "Enriched {}/{} episodes with IMDB ratings for {}",
                enriched,
                episodes.len(),
                imdb_id
            );
        }

        Ok(enriched)
    }

    /// Search for series by title via Skyhook.
    ///
    /// Skyhook search is still needed because it returns tvdb_ids which we use
    /// for episode matching. The IMDB search can supplement results later.
    #[allow(dead_code)] // API layers use local SkyhookSearchResult types; kept for service-layer callers
    pub async fn search_series(&self, term: &str) -> Result<Vec<SkyhookSearchResult>> {
        let url = format!(
            "{}/search/en/?term={}",
            SKYHOOK_BASE_URL,
            urlencoding::encode(term)
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to search Skyhook")?;

        if !response.status().is_success() {
            anyhow::bail!("Skyhook search returned status: {}", response.status());
        }

        response
            .json()
            .await
            .context("Failed to parse Skyhook search response")
    }

    /// Fetch Skyhook-only metadata (no IMDB enrichment).
    ///
    /// Static helper for callers that don't have a `MetadataService` instance
    /// (e.g., scheduler fallback). Creates a temporary HTTP client.
    pub async fn fetch_skyhook_only(tvdb_id: i64) -> Result<MergedSeriesMetadata> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent(format!("pir9/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Failed to create HTTP client")?;

        let url = format!("{}/shows/en/{}", SKYHOOK_BASE_URL, tvdb_id);
        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch from Skyhook")?;

        if !response.status().is_success() {
            anyhow::bail!("Skyhook returned status: {}", response.status());
        }

        let skyhook: SkyhookShowResponse = response
            .json()
            .await
            .context("Failed to parse Skyhook response")?;

        Ok(Self::merge_metadata(skyhook, None))
    }

    /// Fetch a single show's details from Skyhook by TVDB ID.
    pub async fn fetch_from_skyhook(&self, tvdb_id: i64) -> Result<SkyhookShowResponse> {
        let url = format!("{}/shows/en/{}", SKYHOOK_BASE_URL, tvdb_id);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch from Skyhook")?;

        if !response.status().is_success() {
            anyhow::bail!("Skyhook returned status: {}", response.status());
        }

        response
            .json()
            .await
            .context("Failed to parse Skyhook response")
    }

    /// Merge Skyhook + optional IMDB data into a unified result.
    ///
    /// Strategy: IMDB wins for ratings, genres, runtime, year.
    /// Skyhook wins for overview, images, episodes, network, status, certification.
    fn merge_metadata(
        skyhook: SkyhookShowResponse,
        imdb: Option<crate::core::imdb::ImdbSeries>,
    ) -> MergedSeriesMetadata {
        let (imdb_rating, imdb_votes, imdb_id_from_source) = match &imdb {
            Some(i) => (
                i.rating.map(|r| r as f32),
                i.votes.map(|v| v as i32),
                Some(i.imdb_id.clone()),
            ),
            None => (None, None, None),
        };

        // Prefer IMDB genres if available, fall back to Skyhook
        let genres = match &imdb {
            Some(i) if !i.genres.is_empty() => i.genres.clone(),
            _ => skyhook.genres.clone().unwrap_or_default(),
        };

        // Prefer IMDB runtime if available
        let runtime = match &imdb {
            Some(i) if i.runtime_minutes.is_some() => i.runtime_minutes,
            _ => skyhook.runtime,
        };

        // Prefer IMDB year range if available
        let year = match &imdb {
            Some(i) if i.start_year.is_some() => i.start_year,
            _ => skyhook.year,
        };

        // Skyhook ratings as fallback
        let skyhook_rating = skyhook.rating.as_ref().or(skyhook.ratings.as_ref());
        let (skyhook_rating_value, skyhook_rating_votes) = match skyhook_rating {
            Some(r) => (r.value, r.votes),
            None => (None, None),
        };

        MergedSeriesMetadata {
            tvdb_id: skyhook.tvdb_id,
            title: skyhook.title,
            overview: skyhook.overview,
            status: skyhook.status,
            year,
            first_aired: skyhook.first_aired,
            runtime,
            network: skyhook.network,
            certification: skyhook.content_rating.or(skyhook.certification),
            genres,
            images: skyhook.images.unwrap_or_default(),
            seasons: skyhook.seasons.unwrap_or_default(),
            episodes: skyhook.episodes.unwrap_or_default(),
            // IMDB ratings (preferred, higher quality)
            imdb_rating,
            imdb_votes,
            imdb_id: imdb_id_from_source.or(skyhook.imdb_id),
            // Skyhook ratings (fallback)
            skyhook_rating_value,
            skyhook_rating_votes,
        }
    }
}

// ========== Unified Skyhook Response Types ==========
//
// These replace the duplicate definitions scattered across:
//   - src/api/v3/serieslookup.rs
//   - src/api/v3/mediacover.rs
//   - src/api/v5/series.rs
//   - src/api/v5/command.rs
//   - src/core/tv/services.rs
//   - src/main.rs

/// Full show response from Skyhook API
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields populated via JSON deserialization; not all read yet
pub struct SkyhookShowResponse {
    pub tvdb_id: i64,
    pub title: String,
    pub overview: Option<String>,
    pub status: Option<String>,
    pub year: Option<i32>,
    pub first_aired: Option<String>,
    pub runtime: Option<i32>,
    pub network: Option<String>,
    pub content_rating: Option<String>,
    #[serde(default)]
    pub certification: Option<String>,
    pub genres: Option<Vec<String>>,
    pub images: Option<Vec<SkyhookImage>>,
    pub seasons: Option<Vec<SkyhookSeason>>,
    pub episodes: Option<Vec<SkyhookEpisode>>,
    /// Some Skyhook responses use "rating" (singular)
    pub rating: Option<SkyhookRatings>,
    /// Some Skyhook responses use "ratings" (plural)
    pub ratings: Option<SkyhookRatings>,
    /// IMDB ID when returned by Skyhook (often present in search results)
    pub imdb_id: Option<String>,
    pub tvrage_id: Option<i64>,
    pub sort_title: Option<String>,
}

/// Search result from Skyhook search API
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Used by MetadataService::search_series
pub struct SkyhookSearchResult {
    pub tvdb_id: i64,
    pub title: String,
    pub sort_title: Option<String>,
    pub overview: Option<String>,
    pub year: Option<i32>,
    pub status: Option<String>,
    pub images: Option<Vec<SkyhookImage>>,
    pub seasons: Option<Vec<SkyhookSeason>>,
    pub ratings: Option<SkyhookRatings>,
    pub genres: Option<Vec<String>>,
    pub network: Option<String>,
    pub runtime: Option<i32>,
    pub tvrage_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub certification: Option<String>,
    pub first_aired: Option<String>,
}

/// Image metadata from Skyhook
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields populated via JSON deserialization
pub struct SkyhookImage {
    pub cover_type: String,
    pub url: String,
}

/// Season metadata from Skyhook
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields populated via JSON deserialization
pub struct SkyhookSeason {
    pub season_number: i32,
}

/// Episode metadata from Skyhook
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkyhookEpisode {
    pub tvdb_id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub absolute_episode_number: Option<i32>,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub air_date_utc: Option<String>,
    pub runtime: Option<i32>,
}

/// Ratings data from Skyhook (handles both "votes"/"count" field names)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkyhookRatings {
    /// Vote count — Skyhook uses "votes" in search results, "count" in show details
    #[serde(alias = "count")]
    pub votes: Option<i64>,
    /// Rating value — Skyhook sometimes returns this as a string ("3.9") or number (3.9)
    #[serde(default, deserialize_with = "deserialize_f64_or_string")]
    pub value: Option<f64>,
}

/// Deserialize a value that may be either a number or a string representation of a number.
fn deserialize_f64_or_string<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct F64OrString;

    impl<'de> de::Visitor<'de> for F64OrString {
        type Value = Option<f64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a number or string")
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v as f64))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v as f64))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            v.parse::<f64>().map(Some).map_err(de::Error::custom)
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2: serde::Deserializer<'de>>(self, deserializer: D2) -> Result<Self::Value, D2::Error> {
            deserializer.deserialize_any(Self)
        }
    }

    deserializer.deserialize_any(F64OrString)
}

// ========== Merged Metadata Types ==========

/// Result of merging IMDB + Skyhook metadata for a series
#[allow(dead_code)] // Fields populated during metadata merging, consumed incrementally as features are wired
pub struct MergedSeriesMetadata {
    pub tvdb_id: i64,
    pub title: String,
    pub overview: Option<String>,
    pub status: Option<String>,
    pub year: Option<i32>,
    pub first_aired: Option<String>,
    pub runtime: Option<i32>,
    pub network: Option<String>,
    pub certification: Option<String>,
    pub genres: Vec<String>,
    pub images: Vec<SkyhookImage>,
    pub seasons: Vec<SkyhookSeason>,
    pub episodes: Vec<SkyhookEpisode>,
    /// IMDB rating (preferred, higher quality than Skyhook)
    pub imdb_rating: Option<f32>,
    /// IMDB vote count
    pub imdb_votes: Option<i32>,
    /// IMDB ID (from IMDB directly or captured from Skyhook)
    pub imdb_id: Option<String>,
    /// Skyhook/TVDB rating (fallback)
    pub skyhook_rating_value: Option<f64>,
    /// Skyhook/TVDB vote count (fallback)
    pub skyhook_rating_votes: Option<i64>,
}

/// Episode enrichment data for batch IMDB update
pub struct EpisodeEnrichment {
    pub season_number: i32,
    pub episode_number: i32,
    pub imdb_id: Option<String>,
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<i32>,
}
