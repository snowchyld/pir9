#![allow(dead_code, unused_imports)]
//! Music data repositories
//! Database access layer for Music entities (Artist, Album, Track)

use anyhow::{Context, Result};
use sqlx::Row;
use std::collections::HashSet;

use crate::core::datastore::Database;
use crate::core::music::models::{Album, Artist, Track, TrackFile};
use crate::core::music::ArtistStatusType;
use crate::core::tv::MediaCover;

/// Repository for Artist entities
#[derive(Debug, Clone)]
pub struct ArtistRepository {
    db: Database,
}

impl ArtistRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find artist by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<Artist>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM artists WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_artist(&row)?)),
            None => Ok(None),
        }
    }

    /// Find artist by MusicBrainz ID
    pub async fn find_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Artist>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM artists WHERE musicbrainz_id = $1")
            .bind(mbid)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_artist(&row)?)),
            None => Ok(None),
        }
    }

    /// Find all artists
    pub async fn find_all(&self) -> Result<Vec<Artist>> {
        let pool = self.db.pool();
        let rows = sqlx::query("SELECT * FROM artists ORDER BY sort_name")
            .fetch_all(pool)
            .await?;

        let mut artists = Vec::new();
        for row in rows {
            artists.push(self.row_to_artist(&row)?);
        }
        Ok(artists)
    }

    /// Insert a new artist
    pub async fn insert(&self, artist: &Artist) -> Result<Artist> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&artist.genres)?;
        let tags_json = serde_json::to_string(&artist.tags)?;
        let images_json = serde_json::to_string(&artist.images)?;

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO artists (
                musicbrainz_id, name, clean_name, sort_name, overview,
                artist_type, status, genres, images, tags,
                path, root_folder_path, quality_profile_id, monitored,
                added, last_info_sync, title_slug
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17
            )
            RETURNING id
            "#,
        )
        .bind(&artist.musicbrainz_id)
        .bind(&artist.name)
        .bind(&artist.clean_name)
        .bind(&artist.sort_name)
        .bind(&artist.overview)
        .bind(&artist.artist_type)
        .bind(artist.status.as_str())
        .bind(&genres_json)
        .bind(&images_json)
        .bind(&tags_json)
        .bind(&artist.path)
        .bind(&artist.root_folder_path)
        .bind(artist.quality_profile_id)
        .bind(artist.monitored)
        .bind(artist.added)
        .bind(artist.last_info_sync)
        .bind(&artist.title_slug)
        .fetch_one(pool)
        .await?;

        self.find_by_id(row.0)
            .await?
            .context("Failed to retrieve inserted artist")
    }

    /// Update an existing artist
    pub async fn update(&self, artist: &Artist) -> Result<Artist> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&artist.genres)?;
        let tags_json = serde_json::to_string(&artist.tags)?;
        let images_json = serde_json::to_string(&artist.images)?;

        sqlx::query(
            r#"
            UPDATE artists SET
                musicbrainz_id = $1, name = $2, clean_name = $3, sort_name = $4,
                overview = $5, artist_type = $6, status = $7, genres = $8,
                images = $9, tags = $10, path = $11, root_folder_path = $12,
                quality_profile_id = $13, monitored = $14, last_info_sync = $15,
                title_slug = $16
            WHERE id = $17
            "#,
        )
        .bind(&artist.musicbrainz_id)
        .bind(&artist.name)
        .bind(&artist.clean_name)
        .bind(&artist.sort_name)
        .bind(&artist.overview)
        .bind(&artist.artist_type)
        .bind(artist.status.as_str())
        .bind(&genres_json)
        .bind(&images_json)
        .bind(&tags_json)
        .bind(&artist.path)
        .bind(&artist.root_folder_path)
        .bind(artist.quality_profile_id)
        .bind(artist.monitored)
        .bind(artist.last_info_sync)
        .bind(&artist.title_slug)
        .bind(artist.id)
        .execute(pool)
        .await?;

        self.find_by_id(artist.id)
            .await?
            .context("Failed to retrieve updated artist")
    }

    /// Delete an artist
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        // track_files don't cascade via FK, delete explicitly
        sqlx::query("DELETE FROM track_files WHERE artist_id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        // albums and tracks cascade on delete via FK
        sqlx::query("DELETE FROM artists WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Convert PostgreSQL row to Artist entity
    fn row_to_artist(&self, row: &sqlx::postgres::PgRow) -> Result<Artist> {
        let genres_str: String = row.try_get("genres")?;
        let tags_str: String = row.try_get("tags")?;
        let images_str: String = row.try_get("images")?;
        let status_str: String = row.try_get("status")?;

        let genres: Vec<String> = serde_json::from_str(&genres_str).unwrap_or_default();
        let tags: HashSet<i64> = serde_json::from_str(&tags_str).unwrap_or_default();
        let images = serde_json::from_str(&images_str).unwrap_or_default();

        Ok(Artist {
            id: row.try_get("id")?,
            musicbrainz_id: row.try_get("musicbrainz_id")?,
            name: row.try_get("name")?,
            clean_name: row.try_get("clean_name")?,
            sort_name: row.try_get("sort_name")?,
            overview: row.try_get("overview")?,
            artist_type: row.try_get("artist_type")?,
            status: ArtistStatusType::from_str(&status_str),
            monitored: row.try_get("monitored")?,
            quality_profile_id: row.try_get("quality_profile_id")?,
            title_slug: row.try_get("title_slug")?,
            path: row.try_get("path")?,
            root_folder_path: row.try_get("root_folder_path")?,
            genres,
            images,
            tags,
            added: row.try_get("added")?,
            last_info_sync: row.try_get("last_info_sync")?,
        })
    }
}

/// Repository for Album entities
#[derive(Debug, Clone)]
pub struct AlbumRepository {
    db: Database,
}

impl AlbumRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find album by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<Album>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM albums WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_album(&row)?)),
            None => Ok(None),
        }
    }

    /// Find albums by artist ID
    pub async fn find_by_artist_id(&self, artist_id: i64) -> Result<Vec<Album>> {
        let pool = self.db.pool();
        let rows = sqlx::query("SELECT * FROM albums WHERE artist_id = $1 ORDER BY release_date")
            .bind(artist_id)
            .fetch_all(pool)
            .await?;

        let mut albums = Vec::new();
        for row in rows {
            albums.push(self.row_to_album(&row)?);
        }
        Ok(albums)
    }

    /// Find all albums
    pub async fn find_all(&self) -> Result<Vec<Album>> {
        let pool = self.db.pool();
        let rows = sqlx::query("SELECT * FROM albums ORDER BY artist_id, release_date")
            .fetch_all(pool)
            .await?;

        let mut albums = Vec::new();
        for row in rows {
            albums.push(self.row_to_album(&row)?);
        }
        Ok(albums)
    }

    /// Insert a new album
    pub async fn insert(&self, album: &Album) -> Result<Album> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&album.genres)?;
        let images_json = serde_json::to_string(&album.images)?;

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO albums (
                artist_id, musicbrainz_id, title, clean_title, album_type,
                release_date, genres, images, monitored, added, last_info_sync
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
            )
            RETURNING id
            "#,
        )
        .bind(album.artist_id)
        .bind(&album.musicbrainz_id)
        .bind(&album.title)
        .bind(&album.clean_title)
        .bind(&album.album_type)
        .bind(album.release_date)
        .bind(&genres_json)
        .bind(&images_json)
        .bind(album.monitored)
        .bind(album.added)
        .bind(album.last_info_sync)
        .fetch_one(pool)
        .await?;

        self.find_by_id(row.0)
            .await?
            .context("Failed to retrieve inserted album")
    }

    /// Update an existing album
    pub async fn update(&self, album: &Album) -> Result<Album> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&album.genres)?;
        let images_json = serde_json::to_string(&album.images)?;

        sqlx::query(
            r#"
            UPDATE albums SET
                artist_id = $1, musicbrainz_id = $2, title = $3, clean_title = $4,
                album_type = $5, release_date = $6, genres = $7, images = $8,
                monitored = $9, last_info_sync = $10
            WHERE id = $11
            "#,
        )
        .bind(album.artist_id)
        .bind(&album.musicbrainz_id)
        .bind(&album.title)
        .bind(&album.clean_title)
        .bind(&album.album_type)
        .bind(album.release_date)
        .bind(&genres_json)
        .bind(&images_json)
        .bind(album.monitored)
        .bind(album.last_info_sync)
        .bind(album.id)
        .execute(pool)
        .await?;

        self.find_by_id(album.id)
            .await?
            .context("Failed to retrieve updated album")
    }

    /// Delete an album
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        // tracks cascade on delete via FK
        sqlx::query("DELETE FROM albums WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Convert PostgreSQL row to Album entity
    fn row_to_album(&self, row: &sqlx::postgres::PgRow) -> Result<Album> {
        let genres_str: String = row.try_get("genres")?;
        let images_str: String = row.try_get("images")?;

        let genres: Vec<String> = serde_json::from_str(&genres_str).unwrap_or_default();
        let images = serde_json::from_str(&images_str).unwrap_or_default();

        Ok(Album {
            id: row.try_get("id")?,
            artist_id: row.try_get("artist_id")?,
            musicbrainz_id: row.try_get("musicbrainz_id")?,
            title: row.try_get("title")?,
            clean_title: row.try_get("clean_title")?,
            album_type: row.try_get("album_type")?,
            release_date: row.try_get("release_date")?,
            genres,
            images,
            monitored: row.try_get("monitored")?,
            added: row.try_get("added")?,
            last_info_sync: row.try_get("last_info_sync")?,
        })
    }
}

/// Repository for Track entities
#[derive(Debug, Clone)]
pub struct TrackRepository {
    db: Database,
}

impl TrackRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find track by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<Track>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM tracks WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_track(&row)?)),
            None => Ok(None),
        }
    }

    /// Find tracks by album ID
    pub async fn find_by_album_id(&self, album_id: i64) -> Result<Vec<Track>> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE album_id = $1 ORDER BY disc_number, track_number",
        )
        .bind(album_id)
        .fetch_all(pool)
        .await?;

        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(self.row_to_track(&row)?);
        }
        Ok(tracks)
    }

    /// Find tracks by artist ID
    pub async fn find_by_artist_id(&self, artist_id: i64) -> Result<Vec<Track>> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE artist_id = $1 ORDER BY album_id, disc_number, track_number",
        )
        .bind(artist_id)
        .fetch_all(pool)
        .await?;

        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(self.row_to_track(&row)?);
        }
        Ok(tracks)
    }

    /// Convert PostgreSQL row to Track entity
    fn row_to_track(&self, row: &sqlx::postgres::PgRow) -> Result<Track> {
        Ok(Track {
            id: row.try_get("id")?,
            album_id: row.try_get("album_id")?,
            artist_id: row.try_get("artist_id")?,
            title: row.try_get("title")?,
            track_number: row.try_get("track_number")?,
            disc_number: row.try_get("disc_number")?,
            duration_ms: row.try_get("duration_ms")?,
            has_file: row.try_get("has_file")?,
            track_file_id: row.try_get("track_file_id")?,
            monitored: row.try_get("monitored")?,
            air_date_utc: row.try_get("air_date_utc")?,
        })
    }
}
