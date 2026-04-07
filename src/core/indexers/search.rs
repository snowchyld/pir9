//! Indexer search functionality

use anyhow::{Context, Result};

use super::clients::{create_client_from_model, SearchQuery};
use super::{ReleaseInfo, SearchCriteria};
use crate::core::datastore::models::IndexerDbModel;

/// Search service for indexers
pub struct IndexerSearchService {
    indexers: Vec<IndexerDbModel>,
}

impl IndexerSearchService {
    pub fn new(indexers: Vec<IndexerDbModel>) -> Self {
        Self { indexers }
    }

    /// Search for episodes across all enabled indexers
    pub async fn search(&self, criteria: &SearchCriteria) -> Result<Vec<ReleaseInfo>> {
        let mut all_releases = Vec::new();

        for indexer in &self.indexers {
            if !indexer.enable_automatic_search {
                continue;
            }

            match self.search_indexer(indexer, criteria).await {
                Ok(releases) => {
                    tracing::debug!(
                        "Indexer {} returned {} releases",
                        indexer.name,
                        releases.len()
                    );
                    all_releases.extend(releases);
                }
                Err(e) => {
                    tracing::warn!("Indexer {} search failed: {}", indexer.name, e);
                }
            }
        }

        // Sort by quality and other criteria
        all_releases.sort_by(|a, b| b.quality.quality.weight().cmp(&a.quality.quality.weight()));

        Ok(all_releases)
    }

    /// Search a specific indexer
    pub async fn search_indexer(
        &self,
        indexer: &IndexerDbModel,
        criteria: &SearchCriteria,
    ) -> Result<Vec<ReleaseInfo>> {
        let client =
            create_client_from_model(indexer).context("Failed to create indexer client")?;

        // Sanitize series title for search (remove special characters)
        let search_term = sanitize_title_for_search(&criteria.series_title);

        // Build search query from criteria
        // Use series title as the primary search term (works with all indexers)
        // Also include TVDB ID for indexers that support it
        let query = SearchQuery {
            query: Some(search_term.clone()),
            tvdb_id: Some(criteria.series_id),
            season: criteria.season_number,
            episode: criteria.episode_numbers.first().copied(),
            limit: Some(100),
            categories: vec![5000], // TV parent category - indexers will search subcategories
            ..Default::default()
        };

        tracing::info!(
            "Searching indexer {} for Term: [{}] | TVDbId: [{}] | S{:02}E{:02}",
            indexer.name,
            search_term,
            criteria.series_id,
            criteria.season_number.unwrap_or(0),
            criteria.episode_numbers.first().copied().unwrap_or(0)
        );

        let mut releases = client.search(&query).await?;

        // Set the indexer ID on all releases
        for release in &mut releases {
            release.indexer_id = indexer.id;
            release.indexer = indexer.name.clone();
        }

        // Apply minimum seeders filter
        let min_seeders = get_minimum_seeders(indexer);
        let releases = apply_minimum_seeders_filter(releases, min_seeders);

        Ok(releases)
    }

    /// Interactive search with more results
    pub async fn interactive_search(&self, criteria: &SearchCriteria) -> Result<Vec<ReleaseInfo>> {
        let mut all_releases = Vec::new();

        for indexer in &self.indexers {
            if !indexer.enable_interactive_search {
                continue;
            }

            match self.search_indexer(indexer, criteria).await {
                Ok(releases) => {
                    tracing::debug!(
                        "Indexer {} returned {} releases for interactive search",
                        indexer.name,
                        releases.len()
                    );
                    all_releases.extend(releases);
                }
                Err(e) => {
                    tracing::warn!("Indexer {} search failed: {}", indexer.name, e);
                }
            }
        }

        // Sort by quality
        all_releases.sort_by(|a, b| b.quality.quality.weight().cmp(&a.quality.quality.weight()));

        Ok(all_releases)
    }

    /// Search a specific indexer for movies
    pub async fn search_movie_indexer(
        &self,
        indexer: &IndexerDbModel,
        title: &str,
        year: Option<i32>,
        imdb_id: Option<&str>,
    ) -> Result<Vec<ReleaseInfo>> {
        let client =
            create_client_from_model(indexer).context("Failed to create indexer client")?;

        let search_term = sanitize_title_for_search(title);
        // Append year for better text-search precision (skip if title already ends with the year)
        let query_text = if let Some(y) = year {
            let year_str = y.to_string();
            if search_term.ends_with(&year_str) {
                search_term.clone()
            } else {
                format!("{} {}", search_term, y)
            }
        } else {
            search_term.clone()
        };

        let query = SearchQuery {
            query: Some(query_text.clone()),
            imdb_id: imdb_id.map(String::from),
            limit: Some(100),
            categories: get_movie_categories(),
            is_movie_search: true,
            ..Default::default()
        };

        tracing::info!(
            "Searching indexer {} for movie: Term: [{}] | IMDB: [{}]",
            indexer.name,
            query_text,
            imdb_id.unwrap_or("none"),
        );

        let mut releases = client.search(&query).await?;

        for release in &mut releases {
            release.indexer_id = indexer.id;
            release.indexer = indexer.name.clone();
        }

        // Apply minimum seeders filter
        let min_seeders = get_minimum_seeders(indexer);
        let releases = apply_minimum_seeders_filter(releases, min_seeders);

        Ok(releases)
    }

    /// Automatic movie search (uses enable_automatic_search filter)
    pub async fn movie_search(
        &self,
        title: &str,
        year: Option<i32>,
        imdb_id: Option<&str>,
    ) -> Result<Vec<ReleaseInfo>> {
        let mut all_releases = Vec::new();

        for indexer in &self.indexers {
            if !indexer.enable_automatic_search {
                continue;
            }

            match self.search_movie_indexer(indexer, title, year, imdb_id).await {
                Ok(releases) => {
                    tracing::debug!(
                        "Indexer {} returned {} movie releases",
                        indexer.name,
                        releases.len()
                    );
                    all_releases.extend(releases);
                }
                Err(e) => {
                    tracing::warn!("Indexer {} movie search failed: {:#}", indexer.name, e);
                }
            }
        }

        all_releases.sort_by(|a, b| b.quality.quality.weight().cmp(&a.quality.quality.weight()));
        Ok(all_releases)
    }

    /// Interactive movie search (uses enable_interactive_search filter)
    pub async fn interactive_movie_search(
        &self,
        title: &str,
        year: Option<i32>,
        imdb_id: Option<&str>,
    ) -> Result<Vec<ReleaseInfo>> {
        let mut all_releases = Vec::new();

        for indexer in &self.indexers {
            if !indexer.enable_interactive_search {
                continue;
            }

            match self.search_movie_indexer(indexer, title, year, imdb_id).await {
                Ok(releases) => {
                    tracing::debug!(
                        "Indexer {} returned {} releases for interactive movie search",
                        indexer.name,
                        releases.len()
                    );
                    all_releases.extend(releases);
                }
                Err(e) => {
                    tracing::warn!("Indexer {} movie search failed: {:#}", indexer.name, e);
                }
            }
        }

        all_releases.sort_by(|a, b| b.quality.quality.weight().cmp(&a.quality.quality.weight()));
        Ok(all_releases)
    }

    /// Search by query text with optional category override.
    /// Pass `None` for categories to use TV defaults (backwards compat).
    pub async fn search_by_query(
        &self,
        series_title: &str,
        season: Option<i32>,
        episode: Option<i32>,
    ) -> Result<Vec<ReleaseInfo>> {
        self.search_by_query_with_categories(series_title, season, episode, None).await
    }

    /// Search by query text with explicit categories.
    pub async fn search_by_query_with_categories(
        &self,
        query_text: &str,
        season: Option<i32>,
        episode: Option<i32>,
        categories: Option<Vec<i32>>,
    ) -> Result<Vec<ReleaseInfo>> {
        let mut all_releases = Vec::new();

        for indexer in &self.indexers {
            if !indexer.enable_automatic_search && !indexer.enable_interactive_search {
                continue;
            }

            match create_client_from_model(indexer) {
                Ok(client) => {
                    let cats = categories.clone()
                        .unwrap_or_else(|| get_tv_categories(indexer.protocol));
                    let query = SearchQuery {
                        query: Some(query_text.to_string()),
                        season,
                        episode,
                        limit: Some(100),
                        categories: cats,
                        ..Default::default()
                    };

                    match client.search(&query).await {
                        Ok(mut releases) => {
                            for release in &mut releases {
                                release.indexer_id = indexer.id;
                                release.indexer = indexer.name.clone();
                            }
                            let min_seeders = get_minimum_seeders(indexer);
                            let releases =
                                apply_minimum_seeders_filter(releases, min_seeders);
                            all_releases.extend(releases);
                        }
                        Err(e) => {
                            tracing::warn!("Indexer {} search failed: {}", indexer.name, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to create client for {}: {}", indexer.name, e);
                }
            }
        }

        // Sort by quality
        all_releases.sort_by(|a, b| b.quality.quality.weight().cmp(&a.quality.quality.weight()));

        Ok(all_releases)
    }
}

/// Parse minimumSeeders from an indexer's settings JSON.
/// Handles both numeric and string-encoded values.
pub(crate) fn get_minimum_seeders(indexer: &IndexerDbModel) -> i32 {
    let settings: serde_json::Value =
        serde_json::from_str(&indexer.settings).unwrap_or(serde_json::json!({}));
    settings
        .get("minimumSeeders")
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
        .unwrap_or(1) as i32
}

/// Filter releases below minimum seeders threshold.
/// Only applies to torrent releases that report a seeders count.
pub(crate) fn apply_minimum_seeders_filter(releases: Vec<ReleaseInfo>, min_seeders: i32) -> Vec<ReleaseInfo> {
    if min_seeders <= 1 {
        return releases;
    }

    let before = releases.len();
    let filtered: Vec<ReleaseInfo> = releases
        .into_iter()
        .filter(|r| {
            // Only filter torrent releases that report seeders
            match r.seeders {
                Some(s) => s >= min_seeders,
                None => true, // Keep releases without seeder info
            }
        })
        .collect();

    let removed = before - filtered.len();
    if removed > 0 {
        tracing::debug!(
            "Filtered {} releases below minimum seeders threshold ({})",
            removed,
            min_seeders
        );
    }

    filtered
}

/// Sanitize title for search queries
/// Removes special characters that can break indexer searches
pub fn sanitize_title_for_search(title: &str) -> String {
    // Remove special characters that indexers often can't handle
    // Keep alphanumeric, spaces, and common punctuation
    let sanitized: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '\'' {
                c
            } else {
                ' ' // Replace special chars with space
            }
        })
        .collect();

    // Collapse multiple spaces into one
    let mut result = String::new();
    let mut last_was_space = false;
    for c in sanitized.chars() {
        if c == ' ' {
            if !last_was_space {
                result.push(c);
            }
            last_was_space = true;
        } else {
            result.push(c);
            last_was_space = false;
        }
    }

    result.trim().to_string()
}

/// Get movie categories for Newznab indexers
/// 2000: Movies, 2010: Foreign, 2020: Other, 2030: SD, 2040: HD, 2045: UHD, 2050: BluRay
fn get_movie_categories() -> Vec<i32> {
    vec![2000, 2010, 2020, 2030, 2040, 2045, 2050]
}

/// Get TV categories based on protocol
fn get_tv_categories(_protocol: i32) -> Vec<i32> {
    // Standard Newznab TV categories
    // 5000: TV, 5010: WEB-DL, 5020: Foreign, 5030: SD, 5040: HD, 5045: UHD, 5050: Other, 5060: Sport, 5070: Anime, 5080: Documentary
    vec![5000, 5010, 5020, 5030, 5040, 5045, 5050, 5060, 5070, 5080]
}

/// Get Audio/Music categories (Newznab 3000 range)
/// 3000: Audio, 3010: MP3, 3020: Video, 3030: Audiobook, 3040: Lossless, 3050: Podcast, 3060: Other
pub fn get_music_categories() -> Vec<i32> {
    vec![3000, 3010, 3020, 3040, 3060]
}

/// Get Audiobook categories
pub fn get_audiobook_categories() -> Vec<i32> {
    vec![3030]
}

/// Get Podcast categories
pub fn get_podcast_categories() -> Vec<i32> {
    vec![3050]
}
