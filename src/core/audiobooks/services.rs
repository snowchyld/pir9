#![allow(dead_code, unused_imports)]
//! Audiobook domain services
//! Business logic for managing audiobooks and chapters

use anyhow::{Context, Result};
use chrono::Utc;
use std::sync::Arc;
use tracing::{info, warn};

use crate::core::audiobooks::{
    models::*,
    repositories::{AudiobookChapterRepository, AudiobookRepository},
};
use crate::core::datastore::Database;
use crate::core::messaging::{EventBus, Message};

/// Service for managing audiobooks
pub struct AudiobookService {
    db: Database,
    audiobook_repo: AudiobookRepository,
    chapter_repo: AudiobookChapterRepository,
    event_bus: Arc<EventBus>,
}

impl AudiobookService {
    pub fn new(db: Database, event_bus: Arc<EventBus>) -> Self {
        Self {
            audiobook_repo: AudiobookRepository::new(db.clone()),
            chapter_repo: AudiobookChapterRepository::new(db.clone()),
            db,
            event_bus,
        }
    }

    /// Add a new audiobook to the library
    pub async fn add_audiobook(&self, options: AddAudiobookOptions) -> Result<Audiobook> {
        info!("Adding audiobook: {}", options.title);

        // Check if audiobook already exists by ISBN
        if let Some(ref isbn) = options.isbn {
            if let Some(existing) = self.audiobook_repo.find_by_isbn(isbn).await? {
                warn!("Audiobook already exists: {}", existing.title);
                anyhow::bail!("Audiobook already exists in library");
            }
        }

        // Check if audiobook already exists by ASIN
        if let Some(ref asin) = options.asin {
            if let Some(existing) = self.audiobook_repo.find_by_asin(asin).await? {
                warn!("Audiobook already exists: {}", existing.title);
                anyhow::bail!("Audiobook already exists in library");
            }
        }

        // Create audiobook entity
        let mut audiobook = Audiobook::new(options.title.clone());
        audiobook.author = options.author;
        audiobook.narrator = options.narrator;
        audiobook.isbn = options.isbn;
        audiobook.asin = options.asin;
        audiobook.quality_profile_id = options.quality_profile_id;
        audiobook.root_folder_path = options.root_folder_path.clone();
        audiobook.monitored = options.monitored;
        audiobook.tags = options.tags.into_iter().collect();
        audiobook.path = sanitize_filename::sanitize(&options.title).to_string();

        // Save audiobook to database
        let audiobook = self
            .audiobook_repo
            .insert(&audiobook)
            .await
            .context("Failed to insert audiobook into database")?;

        // Publish event
        self.event_bus
            .publish(Message::AudiobookAdded {
                audiobook_id: audiobook.id,
                title: audiobook.title.clone(),
            })
            .await;

        info!(
            "Successfully added audiobook: {} (ID: {})",
            audiobook.title, audiobook.id
        );

        Ok(audiobook)
    }

    /// Refresh audiobook information from metadata sources
    pub async fn refresh_audiobook(&self, audiobook_id: i64) -> Result<()> {
        let mut audiobook = self
            .audiobook_repo
            .find_by_id(audiobook_id)
            .await?
            .context("Audiobook not found")?;

        info!("Refreshing audiobook: {}", audiobook.title);

        // Placeholder for future metadata integration (OpenLibrary, Audible, etc.)
        audiobook.last_info_sync = Some(Utc::now());

        self.audiobook_repo.update(&audiobook).await?;

        self.event_bus
            .publish(Message::AudiobookUpdated {
                audiobook_id,
                title: audiobook.title,
            })
            .await;

        Ok(())
    }

    /// Get all audiobooks
    pub async fn get_all_audiobooks(&self) -> Result<Vec<Audiobook>> {
        self.audiobook_repo.find_all().await
    }

    /// Get audiobook by ID
    pub async fn get_audiobook(&self, id: i64) -> Result<Option<Audiobook>> {
        self.audiobook_repo.find_by_id(id).await
    }

    /// Update audiobook
    pub async fn update_audiobook(&self, audiobook: &Audiobook) -> Result<Audiobook> {
        let updated = self.audiobook_repo.update(audiobook).await?;

        self.event_bus
            .publish(Message::AudiobookUpdated {
                audiobook_id: audiobook.id,
                title: audiobook.title.clone(),
            })
            .await;

        Ok(updated)
    }

    /// Delete audiobook
    pub async fn delete_audiobook(&self, id: i64, delete_files: bool) -> Result<()> {
        let audiobook = self
            .audiobook_repo
            .find_by_id(id)
            .await?
            .context("Audiobook not found")?;

        if delete_files {
            let path = audiobook.full_path();
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                tokio::fs::remove_dir_all(&path)
                    .await
                    .context("Failed to delete audiobook files")?;
            }
        }

        self.audiobook_repo.delete(id).await?;

        self.event_bus
            .publish(Message::AudiobookDeleted {
                audiobook_id: id,
                title: audiobook.title,
            })
            .await;

        Ok(())
    }

    /// Get chapters for an audiobook
    pub async fn get_chapters(&self, audiobook_id: i64) -> Result<Vec<AudiobookChapter>> {
        self.chapter_repo.find_by_audiobook_id(audiobook_id).await
    }
}
