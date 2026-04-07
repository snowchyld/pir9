#![allow(dead_code, unused_imports)]
//! Music domain services
//! Business logic for managing artists, albums, and tracks

use anyhow::{Context, Result};
use chrono::Utc;
use std::sync::Arc;
use tracing::{info, warn};

use crate::core::datastore::Database;
use crate::core::messaging::{EventBus, Message};
use crate::core::music::{models::*, repositories::ArtistRepository, ArtistStatusType};

/// Service for managing music (artists, albums, tracks)
pub struct MusicService {
    db: Database,
    artist_repo: ArtistRepository,
    event_bus: Arc<EventBus>,
}

impl MusicService {
    pub fn new(db: Database, event_bus: Arc<EventBus>) -> Self {
        Self {
            artist_repo: ArtistRepository::new(db.clone()),
            db,
            event_bus,
        }
    }

    /// Add a new artist to the library
    pub async fn add_artist(&self, options: AddArtistOptions) -> Result<Artist> {
        info!("Adding artist: {}", options.name);

        // Check if artist already exists by MusicBrainz ID
        if let Some(ref mbid) = options.musicbrainz_id {
            if let Some(existing) = self.artist_repo.find_by_musicbrainz_id(mbid).await? {
                warn!("Artist already exists: {}", existing.name);
                anyhow::bail!("Artist already exists in library");
            }
        }

        // Create artist entity
        let mut artist = Artist::new(options.name.clone());
        artist.musicbrainz_id = options.musicbrainz_id;
        artist.quality_profile_id = options.quality_profile_id;
        artist.root_folder_path = options.root_folder_path.clone();
        artist.monitored = options.monitored;
        artist.tags = options.tags.into_iter().collect();
        artist.path = sanitize_filename::sanitize(&options.name).to_string();

        // Save artist to database
        let artist = self
            .artist_repo
            .insert(&artist)
            .await
            .context("Failed to insert artist into database")?;

        // Publish event
        self.event_bus
            .publish(Message::ArtistAdded {
                artist_id: artist.id,
                name: artist.name.clone(),
            })
            .await;

        info!(
            "Successfully added artist: {} (ID: {})",
            artist.name, artist.id
        );

        Ok(artist)
    }

    /// Refresh artist information from metadata sources
    pub async fn refresh_artist(&self, artist_id: i64, _force: bool) -> Result<()> {
        let mut artist = self
            .artist_repo
            .find_by_id(artist_id)
            .await?
            .context("Artist not found")?;

        info!("Refreshing artist: {}", artist.name);

        // Placeholder for future MusicBrainz integration
        artist.last_info_sync = Some(Utc::now());

        self.artist_repo.update(&artist).await?;

        self.event_bus
            .publish(Message::ArtistRefreshed {
                artist_id,
                name: artist.name,
            })
            .await;

        Ok(())
    }

    /// Get all artists
    pub async fn get_all_artists(&self) -> Result<Vec<Artist>> {
        self.artist_repo.find_all().await
    }

    /// Get artist by ID
    pub async fn get_artist(&self, id: i64) -> Result<Option<Artist>> {
        self.artist_repo.find_by_id(id).await
    }

    /// Update artist
    pub async fn update_artist(&self, artist: &Artist) -> Result<Artist> {
        let updated = self.artist_repo.update(artist).await?;

        self.event_bus
            .publish(Message::ArtistUpdated {
                artist_id: artist.id,
                name: artist.name.clone(),
            })
            .await;

        Ok(updated)
    }

    /// Delete artist
    pub async fn delete_artist(&self, id: i64, delete_files: bool) -> Result<()> {
        let artist = self
            .artist_repo
            .find_by_id(id)
            .await?
            .context("Artist not found")?;

        if delete_files {
            let path = artist.full_path();
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                tokio::fs::remove_dir_all(&path)
                    .await
                    .context("Failed to delete artist files")?;
            }
        }

        self.artist_repo.delete(id).await?;

        self.event_bus
            .publish(Message::ArtistDeleted {
                artist_id: id,
                name: artist.name,
            })
            .await;

        Ok(())
    }
}
