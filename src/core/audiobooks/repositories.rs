#![allow(dead_code, unused_imports)]
//! Audiobook data repositories
//! Database access layer for Audiobook entities

use anyhow::{Context, Result};
use sqlx::Row;
use std::collections::HashSet;

use crate::core::audiobooks::models::{Audiobook, AudiobookChapter, AudiobookFile};
use crate::core::datastore::Database;
use crate::core::tv::MediaCover;

/// Repository for Audiobook entities
#[derive(Debug, Clone)]
pub struct AudiobookRepository {
    db: Database,
}

impl AudiobookRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find audiobook by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<Audiobook>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM audiobooks WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_audiobook(&row)?)),
            None => Ok(None),
        }
    }

    /// Find audiobook by ISBN
    pub async fn find_by_isbn(&self, isbn: &str) -> Result<Option<Audiobook>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM audiobooks WHERE isbn = $1")
            .bind(isbn)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_audiobook(&row)?)),
            None => Ok(None),
        }
    }

    /// Find audiobook by ASIN
    pub async fn find_by_asin(&self, asin: &str) -> Result<Option<Audiobook>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM audiobooks WHERE asin = $1")
            .bind(asin)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_audiobook(&row)?)),
            None => Ok(None),
        }
    }

    /// Find all audiobooks
    pub async fn find_all(&self) -> Result<Vec<Audiobook>> {
        let pool = self.db.pool();
        let rows = sqlx::query("SELECT * FROM audiobooks ORDER BY sort_title")
            .fetch_all(pool)
            .await?;

        let mut audiobooks = Vec::new();
        for row in rows {
            audiobooks.push(self.row_to_audiobook(&row)?);
        }
        Ok(audiobooks)
    }

    /// Insert a new audiobook
    pub async fn insert(&self, audiobook: &Audiobook) -> Result<Audiobook> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&audiobook.genres)?;
        let tags_json = serde_json::to_string(&audiobook.tags)?;
        let images_json = serde_json::to_string(&audiobook.images)?;

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO audiobooks (
                title, clean_title, sort_title, author, narrator,
                overview, publisher, isbn, asin, duration_ms,
                release_date, genres, images, tags, path,
                root_folder_path, quality_profile_id, monitored,
                has_file, audiobook_file_id, added, last_info_sync,
                title_slug
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18,
                $19, $20, $21, $22, $23
            )
            RETURNING id
            "#,
        )
        .bind(&audiobook.title)
        .bind(&audiobook.clean_title)
        .bind(&audiobook.sort_title)
        .bind(&audiobook.author)
        .bind(&audiobook.narrator)
        .bind(&audiobook.overview)
        .bind(&audiobook.publisher)
        .bind(&audiobook.isbn)
        .bind(&audiobook.asin)
        .bind(audiobook.duration_ms)
        .bind(audiobook.release_date)
        .bind(&genres_json)
        .bind(&images_json)
        .bind(&tags_json)
        .bind(&audiobook.path)
        .bind(&audiobook.root_folder_path)
        .bind(audiobook.quality_profile_id)
        .bind(audiobook.monitored)
        .bind(audiobook.has_file)
        .bind(audiobook.audiobook_file_id)
        .bind(audiobook.added)
        .bind(audiobook.last_info_sync)
        .bind(&audiobook.title_slug)
        .fetch_one(pool)
        .await?;

        self.find_by_id(row.0)
            .await?
            .context("Failed to retrieve inserted audiobook")
    }

    /// Update an existing audiobook
    pub async fn update(&self, audiobook: &Audiobook) -> Result<Audiobook> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&audiobook.genres)?;
        let tags_json = serde_json::to_string(&audiobook.tags)?;
        let images_json = serde_json::to_string(&audiobook.images)?;

        sqlx::query(
            r#"
            UPDATE audiobooks SET
                title = $1, clean_title = $2, sort_title = $3, author = $4,
                narrator = $5, overview = $6, publisher = $7, isbn = $8,
                asin = $9, duration_ms = $10, release_date = $11, genres = $12,
                images = $13, tags = $14, path = $15, root_folder_path = $16,
                quality_profile_id = $17, monitored = $18, has_file = $19,
                audiobook_file_id = $20, last_info_sync = $21, title_slug = $22
            WHERE id = $23
            "#,
        )
        .bind(&audiobook.title)
        .bind(&audiobook.clean_title)
        .bind(&audiobook.sort_title)
        .bind(&audiobook.author)
        .bind(&audiobook.narrator)
        .bind(&audiobook.overview)
        .bind(&audiobook.publisher)
        .bind(&audiobook.isbn)
        .bind(&audiobook.asin)
        .bind(audiobook.duration_ms)
        .bind(audiobook.release_date)
        .bind(&genres_json)
        .bind(&images_json)
        .bind(&tags_json)
        .bind(&audiobook.path)
        .bind(&audiobook.root_folder_path)
        .bind(audiobook.quality_profile_id)
        .bind(audiobook.monitored)
        .bind(audiobook.has_file)
        .bind(audiobook.audiobook_file_id)
        .bind(audiobook.last_info_sync)
        .bind(&audiobook.title_slug)
        .bind(audiobook.id)
        .execute(pool)
        .await?;

        self.find_by_id(audiobook.id)
            .await?
            .context("Failed to retrieve updated audiobook")
    }

    /// Delete an audiobook
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM audiobook_chapters WHERE audiobook_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM audiobook_files WHERE audiobook_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM audiobooks WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Convert PostgreSQL row to Audiobook entity
    fn row_to_audiobook(&self, row: &sqlx::postgres::PgRow) -> Result<Audiobook> {
        let genres_str: String = row.try_get("genres")?;
        let tags_str: String = row.try_get("tags")?;
        let images_str: String = row.try_get("images")?;

        let genres: Vec<String> = serde_json::from_str(&genres_str).unwrap_or_default();
        let tags: HashSet<i64> = serde_json::from_str(&tags_str).unwrap_or_default();
        let images: Vec<MediaCover> = serde_json::from_str(&images_str).unwrap_or_default();

        Ok(Audiobook {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            clean_title: row.try_get("clean_title")?,
            sort_title: row.try_get("sort_title")?,
            overview: row.try_get("overview")?,
            author: row.try_get("author")?,
            narrator: row.try_get("narrator")?,
            publisher: row.try_get("publisher")?,
            isbn: row.try_get("isbn")?,
            asin: row.try_get("asin")?,
            duration_ms: row.try_get("duration_ms")?,
            release_date: row.try_get("release_date")?,
            genres,
            images,
            tags,
            path: row.try_get("path")?,
            root_folder_path: row.try_get("root_folder_path")?,
            quality_profile_id: row.try_get("quality_profile_id")?,
            title_slug: row.try_get("title_slug")?,
            monitored: row.try_get("monitored")?,
            has_file: row.try_get("has_file")?,
            audiobook_file_id: row.try_get("audiobook_file_id")?,
            added: row.try_get("added")?,
            last_info_sync: row.try_get("last_info_sync")?,
        })
    }
}

/// Repository for AudiobookChapter entities
#[derive(Debug, Clone)]
pub struct AudiobookChapterRepository {
    db: Database,
}

impl AudiobookChapterRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find chapter by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<AudiobookChapter>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM audiobook_chapters WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_chapter(&row)?)),
            None => Ok(None),
        }
    }

    /// Find chapters by audiobook ID
    pub async fn find_by_audiobook_id(&self, audiobook_id: i64) -> Result<Vec<AudiobookChapter>> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            "SELECT * FROM audiobook_chapters WHERE audiobook_id = $1 ORDER BY chapter_number",
        )
        .bind(audiobook_id)
        .fetch_all(pool)
        .await?;

        let mut chapters = Vec::new();
        for row in rows {
            chapters.push(self.row_to_chapter(&row)?);
        }
        Ok(chapters)
    }

    /// Convert PostgreSQL row to AudiobookChapter entity
    fn row_to_chapter(&self, row: &sqlx::postgres::PgRow) -> Result<AudiobookChapter> {
        Ok(AudiobookChapter {
            id: row.try_get("id")?,
            audiobook_id: row.try_get("audiobook_id")?,
            title: row.try_get("title")?,
            chapter_number: row.try_get("chapter_number")?,
            duration_ms: row.try_get("duration_ms")?,
            has_file: row.try_get("has_file")?,
            monitored: row.try_get("monitored")?,
        })
    }
}
