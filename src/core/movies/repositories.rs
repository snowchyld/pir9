#![allow(dead_code, unused_imports)]
//! Movie data repositories
//! Database access layer for Movie entities

use anyhow::{Context, Result};
use sqlx::Row;
use std::collections::HashSet;

use crate::core::datastore::Database;
use crate::core::movies::models::Movie;
use crate::core::movies::MovieStatusType;

/// Repository for Movie entities
#[derive(Debug, Clone)]
pub struct MovieRepository {
    db: Database,
}

impl MovieRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find movie by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<Movie>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM movies WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_movie(&row)?)),
            None => Ok(None),
        }
    }

    /// Find movie by TMDB ID
    pub async fn find_by_tmdb_id(&self, tmdb_id: i64) -> Result<Option<Movie>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM movies WHERE tmdb_id = $1")
            .bind(tmdb_id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_movie(&row)?)),
            None => Ok(None),
        }
    }

    /// Find movie by IMDB ID
    pub async fn find_by_imdb_id(&self, imdb_id: &str) -> Result<Option<Movie>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM movies WHERE imdb_id = $1")
            .bind(imdb_id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_movie(&row)?)),
            None => Ok(None),
        }
    }

    /// Find all movies
    pub async fn find_all(&self) -> Result<Vec<Movie>> {
        let pool = self.db.pool();
        let rows = sqlx::query("SELECT * FROM movies ORDER BY sort_title")
            .fetch_all(pool)
            .await?;

        let mut movies = Vec::new();
        for row in rows {
            movies.push(self.row_to_movie(&row)?);
        }
        Ok(movies)
    }

    /// Insert a new movie
    pub async fn insert(&self, movie: &Movie) -> Result<Movie> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&movie.genres)?;
        let tags_json = serde_json::to_string(&movie.tags)?;
        let images_json = serde_json::to_string(&movie.images)?;

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO movies (
                tmdb_id, imdb_id, title, clean_title, sort_title, status,
                overview, monitored, quality_profile_id, title_slug,
                path, root_folder_path, year, release_date,
                physical_release_date, digital_release_date, runtime,
                studio, certification, genres, tags, images,
                has_file, movie_file_id, added, last_info_sync,
                imdb_rating, imdb_votes
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28
            )
            RETURNING id
            "#,
        )
        .bind(movie.tmdb_id)
        .bind(&movie.imdb_id)
        .bind(&movie.title)
        .bind(&movie.clean_title)
        .bind(&movie.sort_title)
        .bind(movie.status as i32)
        .bind(&movie.overview)
        .bind(movie.monitored)
        .bind(movie.quality_profile_id)
        .bind(&movie.title_slug)
        .bind(&movie.path)
        .bind(&movie.root_folder_path)
        .bind(movie.year)
        .bind(movie.release_date)
        .bind(movie.physical_release_date)
        .bind(movie.digital_release_date)
        .bind(movie.runtime)
        .bind(&movie.studio)
        .bind(&movie.certification)
        .bind(&genres_json)
        .bind(&tags_json)
        .bind(&images_json)
        .bind(movie.has_file)
        .bind(movie.movie_file_id)
        .bind(movie.added)
        .bind(movie.last_info_sync)
        .bind(movie.imdb_rating)
        .bind(movie.imdb_votes)
        .fetch_one(pool)
        .await?;

        self.find_by_id(row.0)
            .await?
            .context("Failed to retrieve inserted movie")
    }

    /// Update an existing movie
    pub async fn update(&self, movie: &Movie) -> Result<Movie> {
        let pool = self.db.pool();
        let genres_json = serde_json::to_string(&movie.genres)?;
        let tags_json = serde_json::to_string(&movie.tags)?;
        let images_json = serde_json::to_string(&movie.images)?;

        sqlx::query(
            r#"
            UPDATE movies SET
                tmdb_id = $1, imdb_id = $2, title = $3, clean_title = $4, sort_title = $5,
                status = $6, overview = $7, monitored = $8, quality_profile_id = $9,
                title_slug = $10, path = $11, root_folder_path = $12,
                year = $13, release_date = $14, physical_release_date = $15,
                digital_release_date = $16, runtime = $17, studio = $18,
                certification = $19, genres = $20, tags = $21, images = $22,
                has_file = $23, movie_file_id = $24, last_info_sync = $25,
                imdb_rating = $26, imdb_votes = $27
            WHERE id = $28
            "#,
        )
        .bind(movie.tmdb_id)
        .bind(&movie.imdb_id)
        .bind(&movie.title)
        .bind(&movie.clean_title)
        .bind(&movie.sort_title)
        .bind(movie.status as i32)
        .bind(&movie.overview)
        .bind(movie.monitored)
        .bind(movie.quality_profile_id)
        .bind(&movie.title_slug)
        .bind(&movie.path)
        .bind(&movie.root_folder_path)
        .bind(movie.year)
        .bind(movie.release_date)
        .bind(movie.physical_release_date)
        .bind(movie.digital_release_date)
        .bind(movie.runtime)
        .bind(&movie.studio)
        .bind(&movie.certification)
        .bind(&genres_json)
        .bind(&tags_json)
        .bind(&images_json)
        .bind(movie.has_file)
        .bind(movie.movie_file_id)
        .bind(movie.last_info_sync)
        .bind(movie.imdb_rating)
        .bind(movie.imdb_votes)
        .bind(movie.id)
        .execute(pool)
        .await?;

        self.find_by_id(movie.id)
            .await?
            .context("Failed to retrieve updated movie")
    }

    /// Delete a movie
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        // movie_files cascade on delete, but be explicit
        sqlx::query("DELETE FROM movie_files WHERE movie_id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        sqlx::query("DELETE FROM movies WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Convert PostgreSQL row to Movie entity
    fn row_to_movie(&self, row: &sqlx::postgres::PgRow) -> Result<Movie> {
        let genres_str: String = row.try_get("genres")?;
        let tags_str: String = row.try_get("tags")?;
        let images_str: String = row.try_get("images")?;

        let genres: Vec<String> = serde_json::from_str(&genres_str).unwrap_or_default();
        let tags: HashSet<i64> = serde_json::from_str(&tags_str).unwrap_or_default();
        let images = serde_json::from_str(&images_str).unwrap_or_default();

        Ok(Movie {
            id: row.try_get("id")?,
            tmdb_id: row.try_get("tmdb_id")?,
            imdb_id: row.try_get("imdb_id")?,
            title: row.try_get("title")?,
            clean_title: row.try_get("clean_title")?,
            sort_title: row.try_get("sort_title")?,
            status: match row.try_get::<i32, _>("status")? {
                0 => MovieStatusType::TBA,
                1 => MovieStatusType::Announced,
                2 => MovieStatusType::InCinemas,
                3 => MovieStatusType::Released,
                _ => MovieStatusType::Deleted,
            },
            overview: row.try_get("overview")?,
            monitored: row.try_get("monitored")?,
            quality_profile_id: row.try_get("quality_profile_id")?,
            quality_profile: None,
            title_slug: row.try_get("title_slug")?,
            path: row.try_get("path")?,
            root_folder_path: row.try_get("root_folder_path")?,
            year: row.try_get("year")?,
            release_date: row.try_get("release_date")?,
            physical_release_date: row.try_get("physical_release_date")?,
            digital_release_date: row.try_get("digital_release_date")?,
            runtime: row.try_get("runtime")?,
            studio: row.try_get("studio")?,
            certification: row.try_get("certification")?,
            genres,
            images,
            ratings: None,
            tags,
            has_file: row.try_get("has_file")?,
            movie_file_id: row.try_get("movie_file_id")?,
            imdb_rating: row.try_get("imdb_rating")?,
            imdb_votes: row.try_get("imdb_votes")?,
            added: row.try_get("added")?,
            last_info_sync: row.try_get("last_info_sync")?,
        })
    }
}
