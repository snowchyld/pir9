#![allow(dead_code, unused_imports)]
//! TV Show domain services
//! Business logic for managing series and episodes

use anyhow::{Context, Result};
use chrono::Utc;
use std::sync::Arc;
use tracing::{info, warn};

use crate::core::tv::{
    models::*,
    repositories::{EpisodeRepository, SeriesRepository},
    AddOptions, NewItemMonitorType, SeriesStatusType,
};
use crate::core::datastore::Database;
use crate::core::messaging::{EventBus, Message};

/// Service for managing TV series
pub struct SeriesService {
    db: Database,
    series_repo: SeriesRepository,
    episode_repo: EpisodeRepository,
    event_bus: Arc<EventBus>,
}

impl SeriesService {
    pub fn new(db: Database, event_bus: Arc<EventBus>) -> Self {
        Self {
            series_repo: SeriesRepository::new(db.clone()),
            episode_repo: EpisodeRepository::new(db.clone()),
            db,
            event_bus,
        }
    }
    
    /// Add a new series to the library
    pub async fn add_series(&self, options: AddSeriesOptions) -> Result<Series> {
        info!("Adding series: {} (TVDB: {})", options.title, options.tvdb_id);
        
        // Check if series already exists
        if let Some(existing) = self.series_repo.find_by_tvdb_id(options.tvdb_id).await? {
            warn!("Series already exists: {}", existing.title);
            anyhow::bail!("Series already exists in library");
        }
        
        // Create series entity
        let mut series = Series::new(options.title.clone(), options.tvdb_id);
        series.quality_profile_id = options.quality_profile_id;
        series.language_profile_id = options.language_profile_id;
        series.root_folder_path = options.root_folder_path.clone();
        series.series_type = options.series_type;
        series.monitored = options.monitored;
        series.season_folder = options.season_folder;
        series.tags = options.tags.into_iter().collect();
        series.path = sanitize_filename::sanitize(&options.title).to_string();
        
        // Save series to database
        let series = self.series_repo.insert(&series).await
            .context("Failed to insert series into database")?;
        
        // Fetch episode info from metadata source
        self.refresh_series(series.id, false).await?;
        
        // Apply monitoring options
        self.apply_monitoring_options(series.id, &options.add_options).await?;
        
        // Search for episodes if requested
        if options.add_options.search_for_missing_episodes {
            self.search_for_missing(series.id).await?;
        }
        
        // Publish event
        self.event_bus.publish(Message::SeriesAdded { 
            series_id: series.id,
            title: series.title.clone(),
        }).await;
        
        info!("Successfully added series: {} (ID: {})", series.title, series.id);
        
        Ok(series)
    }
    
    /// Refresh series information from metadata source
    pub async fn refresh_series(&self, series_id: i64, force: bool) -> Result<()> {
        let mut series = self.series_repo.find_by_id(series_id).await?
            .context("Series not found")?;
        
        // Check if refresh is needed
        if !force && !series.needs_refresh() {
            info!("Series {} is up to date, skipping refresh", series.title);
            return Ok(());
        }
        
        info!("Refreshing series: {}", series.title);
        
        // Fetch updated info from TVDB/TMDB
        // This would integrate with metadata sources
        let metadata = self.fetch_series_metadata(series.tvdb_id).await?;
        
        // Update series fields
        series.overview = metadata.overview;
        series.status = metadata.status;
        series.year = metadata.year;
        series.first_aired = metadata.first_aired;
        series.last_aired = metadata.last_aired;
        series.runtime = metadata.runtime;
        series.network = metadata.network;
        series.certification = metadata.certification;
        series.genres = metadata.genres;
        series.actors = metadata.actors;
        series.images = metadata.images;
        series.ratings = metadata.ratings;
        series.last_info_sync = Some(Utc::now());
        
        // Update seasons
        series.seasons = metadata.seasons;
        
        // Save updates
        self.series_repo.update(&series).await?;
        
        // Refresh episodes
        self.refresh_episodes(series_id, &metadata.episodes).await?;
        
        // Publish event
        self.event_bus.publish(Message::SeriesRefreshed { 
            series_id,
            title: series.title,
        }).await;
        
        Ok(())
    }
    
    /// Refresh episodes for a series
    async fn refresh_episodes(&self, series_id: i64, episodes: &[EpisodeMetadata]) -> Result<()> {
        for ep_metadata in episodes {
            // Check if episode already exists
            let existing = self.episode_repo
                .find_by_series_season_episode(series_id, ep_metadata.season_number, ep_metadata.episode_number)
                .await?;
            
            match existing {
                Some(mut episode) => {
                    // Update existing episode
                    episode.title = ep_metadata.title.clone();
                    episode.overview = ep_metadata.overview.clone();
                    episode.air_date = ep_metadata.air_date;
                    episode.air_date_utc = ep_metadata.air_date_utc;
                    episode.runtime = ep_metadata.runtime;
                    episode.absolute_episode_number = ep_metadata.absolute_episode_number;
                    
                    self.episode_repo.update(&episode).await?;
                }
                None => {
                    // Create new episode
                    let mut episode = Episode::new(
                        series_id,
                        ep_metadata.season_number,
                        ep_metadata.episode_number,
                    );
                    episode.tvdb_id = ep_metadata.tvdb_id;
                    episode.title = ep_metadata.title.clone();
                    episode.overview = ep_metadata.overview.clone();
                    episode.air_date = ep_metadata.air_date;
                    episode.air_date_utc = ep_metadata.air_date_utc;
                    episode.runtime = ep_metadata.runtime;
                    episode.absolute_episode_number = ep_metadata.absolute_episode_number;
                    
                    self.episode_repo.insert(&episode).await?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Apply monitoring options to a series
    async fn apply_monitoring_options(&self, series_id: i64, options: &AddOptions) -> Result<()> {
        let episodes = self.episode_repo.find_by_series_id(series_id).await?;
        
        for mut episode in episodes {
            let should_monitor = match options.monitor {
                NewItemMonitorType::All => true,
                NewItemMonitorType::None => false,
                NewItemMonitorType::Future => {
                    episode.air_date_utc.map(|d| d > Utc::now()).unwrap_or(false)
                }
                NewItemMonitorType::Missing => !episode.has_file,
                NewItemMonitorType::Existing => episode.has_file,
                NewItemMonitorType::FirstSeason => episode.season_number == 1,
                NewItemMonitorType::LastSeason => {
                    // Would need to get max season number
                    true
                }
                NewItemMonitorType::Pilot => {
                    episode.season_number == 1 && episode.episode_number == 1
                }
            };
            
            if options.ignore_episodes_with_files && episode.has_file {
                continue;
            }
            
            if options.ignore_episodes_without_files && !episode.has_file {
                continue;
            }
            
            if episode.monitored != should_monitor {
                episode.monitored = should_monitor;
                self.episode_repo.update(&episode).await?;
            }
        }
        
        Ok(())
    }
    
    /// Search for missing episodes
    async fn search_for_missing(&self, series_id: i64) -> Result<()> {
        let episodes = self.episode_repo.find_missing_by_series(series_id).await?;
        
        for episode in episodes {
            self.event_bus.publish(Message::EpisodeSearchRequested {
                episode_id: episode.id,
                series_id,
                season_number: episode.season_number,
                episode_number: episode.episode_number,
            }).await;
        }
        
        Ok(())
    }
    
    /// Get all series
    pub async fn get_all_series(&self) -> Result<Vec<Series>> {
        self.series_repo.find_all().await
    }
    
    /// Get series by ID
    pub async fn get_series(&self, id: i64) -> Result<Option<Series>> {
        self.series_repo.find_by_id(id).await
    }
    
    /// Update series
    pub async fn update_series(&self, series: &Series) -> Result<Series> {
        let updated = self.series_repo.update(series).await?;
        
        self.event_bus.publish(Message::SeriesUpdated {
            series_id: series.id,
            title: series.title.clone(),
        }).await;
        
        Ok(updated)
    }
    
    /// Delete series
    pub async fn delete_series(&self, id: i64, delete_files: bool) -> Result<()> {
        let series = self.series_repo.find_by_id(id).await?
            .context("Series not found")?;
        
        if delete_files {
            // Delete series files
            tokio::fs::remove_dir_all(&series.full_path()).await
                .context("Failed to delete series files")?;
        }
        
        self.series_repo.delete(id).await?;
        
        self.event_bus.publish(Message::SeriesDeleted {
            series_id: id,
            title: series.title,
        }).await;
        
        Ok(())
    }
    
    /// Fetch series metadata from Skyhook (Sonarr's metadata service)
    async fn fetch_series_metadata(&self, tvdb_id: i64) -> Result<SeriesMetadata> {
        info!("Fetching metadata from Skyhook for TVDB ID: {}", tvdb_id);

        let url = format!("http://skyhook.sonarr.tv/v1/tvdb/shows/en/{}", tvdb_id);

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", "pir9/0.1.0")
            .send()
            .await
            .context("Failed to fetch from Skyhook")?;

        if !response.status().is_success() {
            anyhow::bail!("Skyhook returned status: {}", response.status());
        }

        let skyhook: SkyhookSeriesResponse = response
            .json()
            .await
            .context("Failed to parse Skyhook response")?;

        // Parse status (Skyhook returns capitalized values like "Ended")
        let status = match skyhook.status.as_deref().map(|s| s.to_lowercase()).as_deref() {
            Some("continuing") => SeriesStatusType::Continuing,
            Some("ended") => SeriesStatusType::Ended,
            Some("upcoming") => SeriesStatusType::Upcoming,
            _ => SeriesStatusType::Continuing,
        };

        // Parse first_aired date
        let first_aired = skyhook.first_aired.as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        // Convert images
        let images: Vec<crate::core::tv::MediaCover> = skyhook.images.unwrap_or_default()
            .into_iter()
            .filter_map(|img| {
                let cover_type = match img.cover_type.as_str() {
                    "poster" => crate::core::tv::CoverType::Poster,
                    "fanart" => crate::core::tv::CoverType::Fanart,
                    "banner" => crate::core::tv::CoverType::Banner,
                    _ => return None, // skip unknown cover types
                };
                Some(crate::core::tv::MediaCover {
                    cover_type,
                    url: img.url.clone(),
                    remote_url: Some(img.url),
                })
            })
            .collect();

        // Convert seasons
        let seasons: Vec<crate::core::tv::Season> = skyhook.seasons.unwrap_or_default()
            .into_iter()
            .map(|s| crate::core::tv::Season {
                season_number: s.season_number,
                monitored: true,
                statistics: None,
            })
            .collect();

        // Convert episodes
        let episodes: Vec<EpisodeMetadata> = skyhook.episodes.unwrap_or_default()
            .into_iter()
            .map(|ep| {
                let air_date = ep.air_date.as_ref()
                    .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
                let air_date_utc = ep.air_date_utc.as_ref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc));

                EpisodeMetadata {
                    tvdb_id: ep.tvdb_id,
                    season_number: ep.season_number,
                    episode_number: ep.episode_number,
                    absolute_episode_number: ep.absolute_episode_number,
                    title: ep.title.unwrap_or_else(|| format!("Episode {}", ep.episode_number)),
                    overview: ep.overview,
                    air_date,
                    air_date_utc,
                    runtime: ep.runtime.unwrap_or(0),
                }
            })
            .collect();

        info!("Fetched {} seasons and {} episodes from Skyhook", seasons.len(), episodes.len());

        Ok(SeriesMetadata {
            overview: skyhook.overview,
            status,
            year: skyhook.year.unwrap_or(0),
            first_aired,
            last_aired: None,
            runtime: skyhook.runtime.unwrap_or(0),
            network: skyhook.network,
            certification: skyhook.content_rating,
            genres: skyhook.genres.unwrap_or_default(),
            actors: Vec::new(),
            images,
            ratings: skyhook.rating.map(|r| crate::core::tv::Ratings {
                votes: r.count.unwrap_or(0) as i32,
                value: r.value.unwrap_or(0.0),
                source: "tvdb".to_string(),
            }),
            seasons,
            episodes,
        })
    }
}

/// Skyhook API response structures
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookSeriesResponse {
    tvdb_id: i64,
    title: String,
    overview: Option<String>,
    status: Option<String>,
    year: Option<i32>,
    first_aired: Option<String>,
    runtime: Option<i32>,
    network: Option<String>,
    content_rating: Option<String>,
    genres: Option<Vec<String>>,
    images: Option<Vec<SkyhookImage>>,
    seasons: Option<Vec<SkyhookSeason>>,
    episodes: Option<Vec<SkyhookEpisode>>,
    rating: Option<SkyhookRating>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookImage {
    cover_type: String,
    url: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookSeason {
    season_number: i32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookEpisode {
    tvdb_id: i64,
    season_number: i32,
    episode_number: i32,
    absolute_episode_number: Option<i32>,
    title: Option<String>,
    overview: Option<String>,
    air_date: Option<String>,
    air_date_utc: Option<String>,
    runtime: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookRating {
    count: Option<i64>,
    value: Option<f64>,
}

/// Metadata fetched from external sources
struct SeriesMetadata {
    overview: Option<String>,
    status: SeriesStatusType,
    year: i32,
    first_aired: Option<chrono::NaiveDate>,
    last_aired: Option<chrono::NaiveDate>,
    runtime: i32,
    network: Option<String>,
    certification: Option<String>,
    genres: Vec<String>,
    actors: Vec<crate::core::tv::Actor>,
    images: Vec<crate::core::tv::MediaCover>,
    ratings: Option<crate::core::tv::Ratings>,
    seasons: Vec<crate::core::tv::Season>,
    episodes: Vec<EpisodeMetadata>,
}

/// Episode metadata from external source
struct EpisodeMetadata {
    tvdb_id: i64,
    season_number: i32,
    episode_number: i32,
    absolute_episode_number: Option<i32>,
    title: String,
    overview: Option<String>,
    air_date: Option<chrono::NaiveDate>,
    air_date_utc: Option<chrono::DateTime<Utc>>,
    runtime: i32,
}
