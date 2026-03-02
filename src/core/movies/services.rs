#![allow(dead_code, unused_imports)]
//! Movie domain services
//! Business logic for managing movies

use anyhow::{Context, Result};
use chrono::Utc;
use std::sync::Arc;
use tracing::{info, warn};

use crate::core::datastore::Database;
use crate::core::messaging::{EventBus, Message};
use crate::core::movies::{models::*, repositories::MovieRepository, MovieStatusType};

/// Service for managing movies
pub struct MovieService {
    db: Database,
    movie_repo: MovieRepository,
    event_bus: Arc<EventBus>,
}

impl MovieService {
    pub fn new(db: Database, event_bus: Arc<EventBus>) -> Self {
        Self {
            movie_repo: MovieRepository::new(db.clone()),
            db,
            event_bus,
        }
    }

    /// Add a new movie to the library
    pub async fn add_movie(&self, options: AddMovieOptions) -> Result<Movie> {
        info!(
            "Adding movie: {} (TMDB: {})",
            options.title, options.tmdb_id
        );

        // Check if movie already exists
        if let Some(existing) = self.movie_repo.find_by_tmdb_id(options.tmdb_id).await? {
            warn!("Movie already exists: {}", existing.title);
            anyhow::bail!("Movie already exists in library");
        }

        // Create movie entity
        let mut movie = Movie::new(options.title.clone(), options.tmdb_id);
        movie.imdb_id = options.imdb_id;
        movie.quality_profile_id = options.quality_profile_id;
        movie.root_folder_path = options.root_folder_path.clone();
        movie.monitored = options.monitored;
        movie.tags = options.tags.into_iter().collect();
        movie.path = sanitize_filename::sanitize(&options.title).to_string();

        // Save movie to database
        let movie = self
            .movie_repo
            .insert(&movie)
            .await
            .context("Failed to insert movie into database")?;

        // Publish event
        self.event_bus
            .publish(Message::MovieAdded {
                movie_id: movie.id,
                title: movie.title.clone(),
            })
            .await;

        info!(
            "Successfully added movie: {} (ID: {})",
            movie.title, movie.id
        );

        Ok(movie)
    }

    /// Refresh movie information from metadata sources.
    /// Uses IMDB service first for core metadata, then TMDB/Radarr cascade for images.
    pub async fn refresh_movie(&self, movie_id: i64, _force: bool) -> Result<()> {
        let mut movie = self
            .movie_repo
            .find_by_id(movie_id)
            .await?
            .context("Movie not found")?;

        info!("Refreshing movie: {}", movie.title);

        if let Some(ref imdb_id) = movie.imdb_id {
            // Step 1: Fetch images + enrichment via cascade
            if let Some(enrichment) =
                crate::api::v5::movies::fetch_movie_images_and_tmdb_id(imdb_id).await
            {
                if enrichment.tmdb_id > 0 && movie.tmdb_id == 0 {
                    movie.tmdb_id = enrichment.tmdb_id;
                }
                if !enrichment.images.is_empty() {
                    movie.images = enrichment.images
                        .into_iter()
                        .filter_map(|img| {
                            let cover_type = match img.cover_type.as_str() {
                                "poster" => crate::core::tv::CoverType::Poster,
                                "fanart" => crate::core::tv::CoverType::Fanart,
                                "banner" => crate::core::tv::CoverType::Banner,
                                _ => return None,
                            };
                            Some(crate::core::tv::MediaCover {
                                cover_type,
                                url: img.url,
                                remote_url: img.remote_url,
                            })
                        })
                        .collect();
                }
                info!("Updated metadata for movie: {}", movie.title);
            } else {
                warn!(
                    "No image metadata found for movie: {} (IMDB: {})",
                    movie.title, imdb_id
                );
            }
        } else {
            warn!("Cannot refresh movie '{}': no IMDB ID", movie.title);
        }

        movie.last_info_sync = Some(Utc::now());

        self.movie_repo.update(&movie).await?;

        self.event_bus
            .publish(Message::MovieRefreshed {
                movie_id,
                title: movie.title,
            })
            .await;

        Ok(())
    }

    /// Get all movies
    pub async fn get_all_movies(&self) -> Result<Vec<Movie>> {
        self.movie_repo.find_all().await
    }

    /// Get movie by ID
    pub async fn get_movie(&self, id: i64) -> Result<Option<Movie>> {
        self.movie_repo.find_by_id(id).await
    }

    /// Update movie
    pub async fn update_movie(&self, movie: &Movie) -> Result<Movie> {
        let updated = self.movie_repo.update(movie).await?;

        self.event_bus
            .publish(Message::MovieUpdated {
                movie_id: movie.id,
                title: movie.title.clone(),
            })
            .await;

        Ok(updated)
    }

    /// Delete movie
    pub async fn delete_movie(&self, id: i64, delete_files: bool) -> Result<()> {
        let movie = self
            .movie_repo
            .find_by_id(id)
            .await?
            .context("Movie not found")?;

        if delete_files {
            let path = movie.full_path();
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                tokio::fs::remove_dir_all(&path)
                    .await
                    .context("Failed to delete movie files")?;
            }
        }

        self.movie_repo.delete(id).await?;

        self.event_bus
            .publish(Message::MovieDeleted {
                movie_id: id,
                title: movie.title,
            })
            .await;

        Ok(())
    }
}
