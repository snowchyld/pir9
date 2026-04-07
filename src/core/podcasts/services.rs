#![allow(dead_code, unused_imports)]
//! Podcast domain services
//! Business logic for managing podcasts and episodes

use anyhow::{Context, Result};
use chrono::Utc;
use std::sync::Arc;
use tracing::{info, warn};

use crate::core::datastore::Database;
use crate::core::messaging::{EventBus, Message};
use crate::core::podcasts::{
    feed,
    models::*,
    repositories::{PodcastEpisodeRepository, PodcastRepository},
};
use crate::core::tv::{CoverType, MediaCover};

/// Service for managing podcasts
pub struct PodcastService {
    db: Database,
    podcast_repo: PodcastRepository,
    episode_repo: PodcastEpisodeRepository,
    event_bus: Arc<EventBus>,
}

impl PodcastService {
    pub fn new(db: Database, event_bus: Arc<EventBus>) -> Self {
        Self {
            podcast_repo: PodcastRepository::new(db.clone()),
            episode_repo: PodcastEpisodeRepository::new(db.clone()),
            db,
            event_bus,
        }
    }

    /// Add a new podcast to the library
    pub async fn add_podcast(&self, options: AddPodcastOptions) -> Result<Podcast> {
        info!("Adding podcast from feed: {}", options.feed_url);

        // Check if podcast already exists by feed URL
        if let Some(existing) = self
            .podcast_repo
            .find_by_feed_url(&options.feed_url)
            .await?
        {
            warn!("Podcast already exists: {}", existing.title);
            anyhow::bail!("Podcast already exists in library");
        }

        // Fetch the RSS feed to get metadata
        let feed_metadata = feed::fetch_feed(&options.feed_url).await?;

        let title = options.title.unwrap_or(feed_metadata.title.clone());

        // Create podcast entity
        let mut podcast = Podcast::new(title.clone(), options.feed_url.clone());
        podcast.overview = feed_metadata.description;
        podcast.author = feed_metadata.author;
        podcast.website_url = feed_metadata.link;
        podcast.quality_profile_id = options.quality_profile_id;
        podcast.root_folder_path = options.root_folder_path.clone();
        podcast.monitored = options.monitored;
        podcast.tags = options.tags.into_iter().collect();
        podcast.path = sanitize_filename::sanitize(&title).to_string();
        podcast.genres = feed_metadata.categories;

        // Set image from feed
        if let Some(image_url) = feed_metadata.image_url {
            podcast.images.push(MediaCover {
                cover_type: CoverType::Poster,
                url: image_url.clone(),
                remote_url: Some(image_url),
            });
        }

        // Save podcast to database
        let podcast = self
            .podcast_repo
            .insert(&podcast)
            .await
            .context("Failed to insert podcast into database")?;

        // Create episodes from feed items
        let episodes_added = self
            .sync_feed_episodes(podcast.id, &feed_metadata.episodes)
            .await?;

        // Publish event
        self.event_bus
            .publish(Message::PodcastAdded {
                podcast_id: podcast.id,
                title: podcast.title.clone(),
            })
            .await;

        info!(
            "Successfully added podcast: {} (ID: {}, {} episodes)",
            podcast.title, podcast.id, episodes_added
        );

        Ok(podcast)
    }

    /// Refresh podcast by re-fetching the RSS feed
    pub async fn refresh_podcast(&self, podcast_id: i64) -> Result<()> {
        let mut podcast = self
            .podcast_repo
            .find_by_id(podcast_id)
            .await?
            .context("Podcast not found")?;

        info!("Refreshing podcast: {}", podcast.title);

        // Fetch the RSS feed
        let feed_metadata = feed::fetch_feed(&podcast.feed_url).await?;

        // Update podcast metadata from feed
        if let Some(desc) = feed_metadata.description {
            podcast.overview = Some(desc);
        }
        if let Some(author) = feed_metadata.author {
            podcast.author = Some(author);
        }
        if let Some(link) = feed_metadata.link {
            podcast.website_url = Some(link);
        }
        if let Some(image_url) = feed_metadata.image_url {
            podcast.images = vec![MediaCover {
                cover_type: CoverType::Poster,
                url: image_url.clone(),
                remote_url: Some(image_url),
            }];
        }
        if !feed_metadata.categories.is_empty() {
            podcast.genres = feed_metadata.categories;
        }

        podcast.last_info_sync = Some(Utc::now());

        // Save updates
        self.podcast_repo.update(&podcast).await?;

        // Sync episodes (add new ones, update existing by guid)
        let episodes_added = self
            .sync_feed_episodes(podcast_id, &feed_metadata.episodes)
            .await?;

        // Publish event
        self.event_bus
            .publish(Message::PodcastRefreshed {
                podcast_id,
                title: podcast.title,
            })
            .await;

        info!(
            "Refreshed podcast id={}, {} new episodes added",
            podcast_id, episodes_added
        );

        Ok(())
    }

    /// Sync episodes from feed items. Uses guid for deduplication.
    /// Returns the number of new episodes added.
    async fn sync_feed_episodes(
        &self,
        podcast_id: i64,
        feed_episodes: &[feed::FeedEpisode],
    ) -> Result<i32> {
        let mut added = 0i32;

        for fe in feed_episodes.iter() {
            // Check for existing episode by guid
            let existing = if let Some(ref guid) = fe.guid {
                self.episode_repo.find_by_guid(podcast_id, guid).await?
            } else {
                None
            };

            match existing {
                Some(mut episode) => {
                    // Update existing episode metadata
                    episode.title = fe.title.clone();
                    episode.overview = fe.description.clone();
                    episode.air_date_utc = fe.pub_date;
                    episode.duration_ms = fe.duration_ms;
                    episode.download_url = fe.download_url.clone();
                    episode.file_size = fe.file_size;
                    if let Some(ep_num) = fe.episode_number {
                        episode.episode_number = Some(ep_num);
                    }
                    if let Some(season_num) = fe.season_number {
                        episode.season_number = season_num;
                    }

                    self.episode_repo.update(&episode).await?;
                }
                None => {
                    // Create new episode
                    let mut episode = PodcastEpisode::new(podcast_id, fe.title.clone());
                    episode.overview = fe.description.clone();
                    episode.guid = fe.guid.clone();
                    episode.air_date_utc = fe.pub_date;
                    episode.duration_ms = fe.duration_ms;
                    episode.download_url = fe.download_url.clone();
                    episode.file_size = fe.file_size;
                    episode.episode_number = fe.episode_number;
                    episode.season_number = fe.season_number.unwrap_or(1);

                    self.episode_repo.insert(&episode).await?;
                    added += 1;
                }
            }
        }

        Ok(added)
    }

    /// Get all podcasts
    pub async fn get_all_podcasts(&self) -> Result<Vec<Podcast>> {
        self.podcast_repo.find_all().await
    }

    /// Get podcast by ID
    pub async fn get_podcast(&self, id: i64) -> Result<Option<Podcast>> {
        self.podcast_repo.find_by_id(id).await
    }

    /// Update podcast
    pub async fn update_podcast(&self, podcast: &Podcast) -> Result<Podcast> {
        let updated = self.podcast_repo.update(podcast).await?;

        self.event_bus
            .publish(Message::PodcastUpdated {
                podcast_id: podcast.id,
                title: podcast.title.clone(),
            })
            .await;

        Ok(updated)
    }

    /// Delete podcast
    pub async fn delete_podcast(&self, id: i64, delete_files: bool) -> Result<()> {
        let podcast = self
            .podcast_repo
            .find_by_id(id)
            .await?
            .context("Podcast not found")?;

        if delete_files {
            let path = podcast.full_path();
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                tokio::fs::remove_dir_all(&path)
                    .await
                    .context("Failed to delete podcast files")?;
            }
        }

        self.podcast_repo.delete(id).await?;

        self.event_bus
            .publish(Message::PodcastDeleted {
                podcast_id: id,
                title: podcast.title,
            })
            .await;

        Ok(())
    }

    /// Get episodes for a podcast
    pub async fn get_episodes(&self, podcast_id: i64) -> Result<Vec<PodcastEpisode>> {
        self.episode_repo.find_by_podcast_id(podcast_id).await
    }
}
