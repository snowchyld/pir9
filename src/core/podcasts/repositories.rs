#![allow(dead_code, unused_imports)]
//! Podcast data repositories
//! Database access layer for Podcast entities

use anyhow::{Context, Result};
use sqlx::Row;
use std::collections::HashSet;

use crate::core::datastore::Database;
use crate::core::podcasts::models::{Podcast, PodcastEpisode, PodcastFile};
use crate::core::tv::MediaCover;

/// Repository for Podcast entities
#[derive(Debug, Clone)]
pub struct PodcastRepository {
    db: Database,
}

impl PodcastRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find podcast by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<Podcast>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM podcasts WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_podcast(&row)?)),
            None => Ok(None),
        }
    }

    /// Find podcast by feed URL
    pub async fn find_by_feed_url(&self, feed_url: &str) -> Result<Option<Podcast>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM podcasts WHERE feed_url = $1")
            .bind(feed_url)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_podcast(&row)?)),
            None => Ok(None),
        }
    }

    /// Find all podcasts
    pub async fn find_all(&self) -> Result<Vec<Podcast>> {
        let pool = self.db.pool();
        let rows = sqlx::query("SELECT * FROM podcasts ORDER BY sort_title")
            .fetch_all(pool)
            .await?;

        let mut podcasts = Vec::new();
        for row in rows {
            podcasts.push(self.row_to_podcast(&row)?);
        }
        Ok(podcasts)
    }

    /// Insert a new podcast
    pub async fn insert(&self, podcast: &Podcast) -> Result<Podcast> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&podcast.genres)?;
        let tags_json = serde_json::to_string(&podcast.tags)?;
        let images_json = serde_json::to_string(&podcast.images)?;

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO podcasts (
                title, clean_title, sort_title, overview, author,
                feed_url, website_url, genres, images, tags,
                path, root_folder_path, quality_profile_id, monitored,
                added, last_info_sync, title_slug
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17
            )
            RETURNING id
            "#,
        )
        .bind(&podcast.title)
        .bind(&podcast.clean_title)
        .bind(&podcast.sort_title)
        .bind(&podcast.overview)
        .bind(&podcast.author)
        .bind(&podcast.feed_url)
        .bind(&podcast.website_url)
        .bind(&genres_json)
        .bind(&images_json)
        .bind(&tags_json)
        .bind(&podcast.path)
        .bind(&podcast.root_folder_path)
        .bind(podcast.quality_profile_id)
        .bind(podcast.monitored)
        .bind(podcast.added)
        .bind(podcast.last_info_sync)
        .bind(&podcast.title_slug)
        .fetch_one(pool)
        .await?;

        self.find_by_id(row.0)
            .await?
            .context("Failed to retrieve inserted podcast")
    }

    /// Update an existing podcast
    pub async fn update(&self, podcast: &Podcast) -> Result<Podcast> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&podcast.genres)?;
        let tags_json = serde_json::to_string(&podcast.tags)?;
        let images_json = serde_json::to_string(&podcast.images)?;

        sqlx::query(
            r#"
            UPDATE podcasts SET
                title = $1, clean_title = $2, sort_title = $3, overview = $4,
                author = $5, feed_url = $6, website_url = $7, genres = $8,
                images = $9, tags = $10, path = $11, root_folder_path = $12,
                quality_profile_id = $13, monitored = $14, last_info_sync = $15,
                title_slug = $16
            WHERE id = $17
            "#,
        )
        .bind(&podcast.title)
        .bind(&podcast.clean_title)
        .bind(&podcast.sort_title)
        .bind(&podcast.overview)
        .bind(&podcast.author)
        .bind(&podcast.feed_url)
        .bind(&podcast.website_url)
        .bind(&genres_json)
        .bind(&images_json)
        .bind(&tags_json)
        .bind(&podcast.path)
        .bind(&podcast.root_folder_path)
        .bind(podcast.quality_profile_id)
        .bind(podcast.monitored)
        .bind(podcast.last_info_sync)
        .bind(&podcast.title_slug)
        .bind(podcast.id)
        .execute(pool)
        .await?;

        self.find_by_id(podcast.id)
            .await?
            .context("Failed to retrieve updated podcast")
    }

    /// Delete a podcast
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        // podcast_episodes cascade on delete, but be explicit
        sqlx::query("DELETE FROM podcast_episodes WHERE podcast_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM podcast_files WHERE podcast_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM podcasts WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Convert PostgreSQL row to Podcast entity
    fn row_to_podcast(&self, row: &sqlx::postgres::PgRow) -> Result<Podcast> {
        let genres_str: String = row.try_get("genres")?;
        let tags_str: String = row.try_get("tags")?;
        let images_str: String = row.try_get("images")?;

        let genres: Vec<String> = serde_json::from_str(&genres_str).unwrap_or_default();
        let tags: HashSet<i64> = serde_json::from_str(&tags_str).unwrap_or_default();
        let images: Vec<MediaCover> = serde_json::from_str(&images_str).unwrap_or_default();

        Ok(Podcast {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            clean_title: row.try_get("clean_title")?,
            sort_title: row.try_get("sort_title")?,
            overview: row.try_get("overview")?,
            author: row.try_get("author")?,
            feed_url: row.try_get("feed_url")?,
            website_url: row.try_get("website_url")?,
            genres,
            images,
            tags,
            path: row.try_get("path")?,
            root_folder_path: row.try_get("root_folder_path")?,
            quality_profile_id: row.try_get("quality_profile_id")?,
            title_slug: row.try_get("title_slug")?,
            monitored: row.try_get("monitored")?,
            added: row.try_get("added")?,
            last_info_sync: row.try_get("last_info_sync")?,
        })
    }
}

/// Repository for PodcastEpisode entities
#[derive(Debug, Clone)]
pub struct PodcastEpisodeRepository {
    db: Database,
}

impl PodcastEpisodeRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find episode by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<PodcastEpisode>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM podcast_episodes WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_episode(&row)?)),
            None => Ok(None),
        }
    }

    /// Find episodes by podcast ID
    pub async fn find_by_podcast_id(&self, podcast_id: i64) -> Result<Vec<PodcastEpisode>> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            "SELECT * FROM podcast_episodes WHERE podcast_id = $1 ORDER BY season_number, COALESCE(episode_number, 0)",
        )
        .bind(podcast_id)
        .fetch_all(pool)
        .await?;

        let mut episodes = Vec::new();
        for row in rows {
            episodes.push(self.row_to_episode(&row)?);
        }
        Ok(episodes)
    }

    /// Find episode by podcast ID and guid (for deduplication)
    pub async fn find_by_guid(
        &self,
        podcast_id: i64,
        guid: &str,
    ) -> Result<Option<PodcastEpisode>> {
        let pool = self.db.pool();
        let row = sqlx::query(
            "SELECT * FROM podcast_episodes WHERE podcast_id = $1 AND guid = $2",
        )
        .bind(podcast_id)
        .bind(guid)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_episode(&row)?)),
            None => Ok(None),
        }
    }

    /// Insert a new episode
    pub async fn insert(&self, episode: &PodcastEpisode) -> Result<PodcastEpisode> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO podcast_episodes (
                podcast_id, title, overview, episode_number, season_number,
                air_date_utc, duration_ms, download_url, file_size,
                has_file, podcast_file_id, monitored, guid
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id
            "#,
        )
        .bind(episode.podcast_id)
        .bind(&episode.title)
        .bind(&episode.overview)
        .bind(episode.episode_number)
        .bind(episode.season_number)
        .bind(episode.air_date_utc)
        .bind(episode.duration_ms)
        .bind(&episode.download_url)
        .bind(episode.file_size)
        .bind(episode.has_file)
        .bind(episode.podcast_file_id)
        .bind(episode.monitored)
        .bind(&episode.guid)
        .fetch_one(pool)
        .await?;

        self.find_by_id(row.0)
            .await?
            .context("Failed to retrieve inserted podcast episode")
    }

    /// Update an existing episode
    pub async fn update(&self, episode: &PodcastEpisode) -> Result<PodcastEpisode> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE podcast_episodes SET
                title = $1, overview = $2, episode_number = $3, season_number = $4,
                air_date_utc = $5, duration_ms = $6, download_url = $7, file_size = $8,
                has_file = $9, podcast_file_id = $10, monitored = $11, guid = $12
            WHERE id = $13
            "#,
        )
        .bind(&episode.title)
        .bind(&episode.overview)
        .bind(episode.episode_number)
        .bind(episode.season_number)
        .bind(episode.air_date_utc)
        .bind(episode.duration_ms)
        .bind(&episode.download_url)
        .bind(episode.file_size)
        .bind(episode.has_file)
        .bind(episode.podcast_file_id)
        .bind(episode.monitored)
        .bind(&episode.guid)
        .bind(episode.id)
        .execute(pool)
        .await?;

        self.find_by_id(episode.id)
            .await?
            .context("Failed to retrieve updated podcast episode")
    }

    /// Delete an episode
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM podcast_episodes WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Convert PostgreSQL row to PodcastEpisode entity
    fn row_to_episode(&self, row: &sqlx::postgres::PgRow) -> Result<PodcastEpisode> {
        Ok(PodcastEpisode {
            id: row.try_get("id")?,
            podcast_id: row.try_get("podcast_id")?,
            title: row.try_get("title")?,
            overview: row.try_get("overview")?,
            episode_number: row.try_get("episode_number")?,
            season_number: row.try_get("season_number")?,
            air_date_utc: row.try_get("air_date_utc")?,
            duration_ms: row.try_get("duration_ms")?,
            download_url: row.try_get("download_url")?,
            file_size: row.try_get("file_size")?,
            has_file: row.try_get("has_file")?,
            podcast_file_id: row.try_get("podcast_file_id")?,
            monitored: row.try_get("monitored")?,
            guid: row.try_get("guid")?,
        })
    }
}
