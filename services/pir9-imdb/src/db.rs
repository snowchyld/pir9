//! Database operations for the IMDB service

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use tracing::info;

use crate::models::*;

/// Info needed to resume a previously failed/cancelled sync
pub struct ResumeInfo {
    pub sync_id: i64,
    pub last_processed_id: i64,
    pub rows_processed: i64,
    pub rows_inserted: i64,
    pub rows_updated: i64,
}

/// Repository for database operations
#[derive(Clone)]
pub struct DbRepository {
    pool: PgPool,
}

impl DbRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Search series by title
    ///
    /// Relevance ranking: exact match > starts-with > contains.
    /// Within each tier, sorts by votes (popularity) then rating.
    /// Excludes adult content from search results.
    pub async fn search_series(&self, query: &str, limit: u32) -> Result<Vec<ImdbSeries>> {
        let exact_pattern = query.to_string();
        let starts_pattern = format!("{}%", query);
        let contains_pattern = format!("%{}%", query);
        // Normalize: strip non-alphanumeric, collapse spaces for fuzzy matching
        let clean_query: String = query
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let clean_contains = format!("%{}%", clean_query);
        let limit = limit.min(100) as i64;

        let rows = sqlx::query(
            r#"
            SELECT imdb_id, title, original_title, start_year, end_year,
                   runtime_minutes, genres, is_adult, title_type, rating, votes, last_synced_at
            FROM imdb_series
            WHERE (
                title ILIKE $3 OR original_title ILIKE $3
                OR regexp_replace(title, '[^a-zA-Z0-9 ]', ' ', 'g') ILIKE $5
                OR regexp_replace(original_title, '[^a-zA-Z0-9 ]', ' ', 'g') ILIKE $5
            )
              AND is_adult = false
            ORDER BY
                CASE
                    WHEN title ILIKE $1 OR original_title ILIKE $1 THEN 0
                    WHEN title ILIKE $2 OR original_title ILIKE $2 THEN 1
                    ELSE 2
                END,
                votes DESC NULLS LAST,
                rating DESC NULLS LAST
            LIMIT $4
            "#,
        )
        .bind(&exact_pattern)
        .bind(&starts_pattern)
        .bind(&contains_pattern)
        .bind(limit)
        .bind(&clean_contains)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<ImdbSeries> = rows
            .iter()
            .map(|row| {
                let db_series = DbSeries {
                    imdb_id: row.get("imdb_id"),
                    title: row.get("title"),
                    original_title: row.get("original_title"),
                    start_year: row.get("start_year"),
                    end_year: row.get("end_year"),
                    runtime_minutes: row.get("runtime_minutes"),
                    genres: row.get("genres"),
                    is_adult: row.get("is_adult"),
                    title_type: row.get("title_type"),
                    rating: row.get("rating"),
                    votes: row.get("votes"),
                    last_synced_at: row.get("last_synced_at"),
                };
                db_series.to_api()
            })
            .collect();

        Ok(results)
    }

    /// Get a series by IMDB ID
    pub async fn get_series(&self, imdb_id: &str) -> Result<Option<ImdbSeries>> {
        let numeric_id = parse_imdb_id(imdb_id).ok_or_else(|| anyhow::anyhow!("Invalid IMDB ID"))?;

        let row = sqlx::query(
            r#"
            SELECT imdb_id, title, original_title, start_year, end_year,
                   runtime_minutes, genres, is_adult, title_type, rating, votes, last_synced_at
            FROM imdb_series
            WHERE imdb_id = $1
            "#,
        )
        .bind(numeric_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let db_series = DbSeries {
                imdb_id: row.get("imdb_id"),
                title: row.get("title"),
                original_title: row.get("original_title"),
                start_year: row.get("start_year"),
                end_year: row.get("end_year"),
                runtime_minutes: row.get("runtime_minutes"),
                genres: row.get("genres"),
                is_adult: row.get("is_adult"),
                title_type: row.get("title_type"),
                rating: row.get("rating"),
                votes: row.get("votes"),
                last_synced_at: row.get("last_synced_at"),
            };
            db_series.to_api()
        }))
    }

    /// Get episodes for a series
    pub async fn get_episodes(&self, imdb_id: &str) -> Result<Vec<ImdbEpisode>> {
        let numeric_id = parse_imdb_id(imdb_id).ok_or_else(|| anyhow::anyhow!("Invalid IMDB ID"))?;

        let rows = sqlx::query(
            r#"
            SELECT imdb_id, parent_imdb_id, season_number, episode_number,
                   title, runtime_minutes, rating, votes, air_date, last_synced_at
            FROM imdb_episodes
            WHERE parent_imdb_id = $1
            ORDER BY season_number NULLS LAST, episode_number NULLS LAST
            "#,
        )
        .bind(numeric_id)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<ImdbEpisode> = rows
            .iter()
            .map(|row| {
                let db_ep = DbEpisode {
                    imdb_id: row.get("imdb_id"),
                    parent_imdb_id: row.get("parent_imdb_id"),
                    season_number: row.get("season_number"),
                    episode_number: row.get("episode_number"),
                    title: row.get("title"),
                    runtime_minutes: row.get("runtime_minutes"),
                    rating: row.get("rating"),
                    votes: row.get("votes"),
                    air_date: row.get("air_date"),
                    last_synced_at: row.get("last_synced_at"),
                };
                db_ep.to_api()
            })
            .collect();

        Ok(results)
    }

    /// Search movies by title
    ///
    /// Relevance ranking: exact match > starts-with > contains.
    /// Within each tier, sorts by votes (popularity) then rating.
    /// Excludes adult content from search results.
    /// Returns cached TMDB data if available (no on-demand TMDB fetches for search).
    pub async fn search_movies(&self, query: &str, limit: u32) -> Result<Vec<ImdbMovie>> {
        let exact_pattern = query.to_string();
        let starts_pattern = format!("{}%", query);
        let contains_pattern = format!("%{}%", query);
        // Normalize: strip non-alphanumeric, collapse spaces for fuzzy matching
        let clean_query: String = query
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let clean_contains = format!("%{}%", clean_query);
        let limit = limit.min(100) as i64;

        let rows = sqlx::query(
            r#"
            SELECT imdb_id, title, original_title, year,
                   runtime_minutes, genres, is_adult, rating, votes, last_synced_at,
                   tmdb_id, poster_url, fanart_url, tmdb_fetched_at
            FROM imdb_movies
            WHERE (
                title ILIKE $3 OR original_title ILIKE $3
                OR regexp_replace(title, '[^a-zA-Z0-9 ]', ' ', 'g') ILIKE $5
                OR regexp_replace(original_title, '[^a-zA-Z0-9 ]', ' ', 'g') ILIKE $5
            )
              AND is_adult = false
            ORDER BY
                CASE
                    WHEN title ILIKE $1 OR original_title ILIKE $1 THEN 0
                    WHEN title ILIKE $2 OR original_title ILIKE $2 THEN 1
                    ELSE 2
                END,
                votes DESC NULLS LAST,
                rating DESC NULLS LAST
            LIMIT $4
            "#,
        )
        .bind(&exact_pattern)
        .bind(&starts_pattern)
        .bind(&contains_pattern)
        .bind(limit)
        .bind(&clean_contains)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<ImdbMovie> = rows
            .iter()
            .map(|row| {
                let db_movie = DbMovie {
                    imdb_id: row.get("imdb_id"),
                    title: row.get("title"),
                    original_title: row.get("original_title"),
                    year: row.get("year"),
                    runtime_minutes: row.get("runtime_minutes"),
                    genres: row.get("genres"),
                    is_adult: row.get("is_adult"),
                    rating: row.get("rating"),
                    votes: row.get("votes"),
                    last_synced_at: row.get("last_synced_at"),
                    tmdb_id: row.get("tmdb_id"),
                    poster_url: row.get("poster_url"),
                    fanart_url: row.get("fanart_url"),
                    tmdb_fetched_at: row.get("tmdb_fetched_at"),
                };
                db_movie.to_api()
            })
            .collect();

        Ok(results)
    }

    /// Get a movie by IMDB ID (returns DbMovie so handler can check tmdb_fetched_at)
    pub async fn get_movie(&self, imdb_id: &str) -> Result<Option<DbMovie>> {
        let numeric_id = parse_imdb_id(imdb_id).ok_or_else(|| anyhow::anyhow!("Invalid IMDB ID"))?;

        let row = sqlx::query(
            r#"
            SELECT imdb_id, title, original_title, year,
                   runtime_minutes, genres, is_adult, rating, votes, last_synced_at,
                   tmdb_id, poster_url, fanart_url, tmdb_fetched_at
            FROM imdb_movies
            WHERE imdb_id = $1
            "#,
        )
        .bind(numeric_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| DbMovie {
            imdb_id: row.get("imdb_id"),
            title: row.get("title"),
            original_title: row.get("original_title"),
            year: row.get("year"),
            runtime_minutes: row.get("runtime_minutes"),
            genres: row.get("genres"),
            is_adult: row.get("is_adult"),
            rating: row.get("rating"),
            votes: row.get("votes"),
            last_synced_at: row.get("last_synced_at"),
            tmdb_id: row.get("tmdb_id"),
            poster_url: row.get("poster_url"),
            fanart_url: row.get("fanart_url"),
            tmdb_fetched_at: row.get("tmdb_fetched_at"),
        }))
    }

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<ImdbStats> {
        let series_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM imdb_series")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let episode_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM imdb_episodes")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let movie_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM imdb_movies")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let people_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM imdb_people")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let credits_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM imdb_credits")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        // Get last sync time
        let last_sync: Option<String> = sqlx::query_scalar(
            r#"
            SELECT TO_CHAR(completed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
            FROM imdb_sync_status
            WHERE status = 'completed'
            ORDER BY completed_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);

        // Get database size
        let db_size: Option<i64> = sqlx::query_scalar(
            "SELECT pg_database_size(current_database())"
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);

        Ok(ImdbStats {
            series_count,
            episode_count,
            movie_count,
            people_count,
            credits_count,
            last_sync,
            db_size_bytes: db_size,
        })
    }

    /// Get sync status for all datasets
    pub async fn get_sync_status(&self) -> Result<SyncStatus> {
        async fn get_dataset_status(pool: &PgPool, dataset: &str) -> Result<Option<DatasetSyncStatus>> {
            let row = sqlx::query(
                r#"
                SELECT dataset_name, rows_processed, rows_inserted, rows_updated,
                       started_at, completed_at, status, error_message
                FROM imdb_sync_status
                WHERE dataset_name = $1
                ORDER BY CASE WHEN status = 'running' THEN 0 ELSE 1 END, started_at DESC
                LIMIT 1
                "#,
            )
            .bind(dataset)
            .fetch_optional(pool)
            .await?;

            Ok(row.map(|r| DatasetSyncStatus {
                dataset_name: r.get("dataset_name"),
                rows_processed: r.get("rows_processed"),
                rows_inserted: r.get("rows_inserted"),
                rows_updated: r.get("rows_updated"),
                started_at: r.get::<chrono::DateTime<chrono::Utc>, _>("started_at").to_rfc3339(),
                completed_at: r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at").map(|d| d.to_rfc3339()),
                status: r.get("status"),
                error_message: r.get("error_message"),
                is_running: false,
                download_progress: None,
                download_size_bytes: None,
                download_bytes_done: None,
                current_phase: None,
            }))
        }

        Ok(SyncStatus {
            is_running: false, // Enriched by the handler from the in-memory SyncHandle
            title_basics: get_dataset_status(&self.pool, "title.basics.tsv.gz").await?,
            title_episodes: get_dataset_status(&self.pool, "title.episode.tsv.gz").await?,
            title_ratings: get_dataset_status(&self.pool, "title.ratings.tsv.gz").await?,
            name_basics: get_dataset_status(&self.pool, "name.basics.tsv.gz").await?,
            title_principals: get_dataset_status(&self.pool, "title.principals.tsv.gz").await?,
        })
    }

    /// Start a sync record
    pub async fn start_sync(&self, dataset_name: &str) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO imdb_sync_status (dataset_name, started_at, status, rows_processed, rows_inserted, rows_updated)
            VALUES ($1, NOW(), 'running', 0, 0, 0)
            RETURNING id
            "#,
        )
        .bind(dataset_name)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Update sync progress and save resume checkpoint
    pub async fn update_sync_progress_with_resume(
        &self,
        sync_id: i64,
        rows_processed: i64,
        rows_inserted: i64,
        rows_updated: i64,
        last_processed_id: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_sync_status
            SET rows_processed = $2, rows_inserted = $3, rows_updated = $4, last_processed_id = $5
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .bind(rows_processed)
        .bind(rows_inserted)
        .bind(rows_updated)
        .bind(last_processed_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get the last completed sync time for a dataset (for skip-if-recent)
    pub async fn last_completed_sync_time(&self, dataset: &str) -> Result<Option<DateTime<Utc>>> {
        let result: Option<DateTime<Utc>> = sqlx::query_scalar(
            r#"
            SELECT completed_at
            FROM imdb_sync_status
            WHERE dataset_name = $1 AND status = 'completed'
            ORDER BY completed_at DESC
            LIMIT 1
            "#,
        )
        .bind(dataset)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Find a failed/cancelled sync that can be resumed
    pub async fn get_resumable_sync(&self, dataset: &str) -> Result<Option<ResumeInfo>> {
        let row = sqlx::query(
            r#"
            SELECT id, last_processed_id, rows_processed, rows_inserted, rows_updated
            FROM imdb_sync_status
            WHERE dataset_name = $1
              AND status IN ('failed', 'cancelled')
              AND last_processed_id IS NOT NULL
              AND last_processed_id > 0
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(dataset)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| ResumeInfo {
            sync_id: r.get("id"),
            last_processed_id: r.get("last_processed_id"),
            rows_processed: r.get("rows_processed"),
            rows_inserted: r.get("rows_inserted"),
            rows_updated: r.get("rows_updated"),
        }))
    }

    /// Resume a previously failed/cancelled sync
    pub async fn resume_sync(&self, sync_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_sync_status
            SET status = 'running', completed_at = NULL, error_message = NULL
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark any stale 'running' syncs as failed (startup cleanup after crash)
    pub async fn fail_stale_running_syncs(&self) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE imdb_sync_status
            SET status = 'failed', completed_at = NOW(), error_message = 'Marked as failed on startup (stale running sync)'
            WHERE status = 'running'
            "#,
        )
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected() as i64;
        if count > 0 {
            info!("Cleaned up {} stale running sync(s) from previous run", count);
        }
        Ok(count)
    }

    /// Cancel a sync by setting its status (used when the task itself detects cancellation)
    pub async fn cancel_sync(&self, sync_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_sync_status
            SET status = 'cancelled', completed_at = NOW(), error_message = 'Cancelled by user'
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Complete a sync
    pub async fn complete_sync(&self, sync_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_sync_status
            SET status = 'completed', completed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Fail a sync
    pub async fn fail_sync(&self, sync_id: i64, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_sync_status
            SET status = 'failed', completed_at = NOW(), error_message = $2
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .bind(error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Upsert a series
    pub async fn upsert_series(&self, series: &DbSeries) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO imdb_series (imdb_id, title, original_title, start_year, end_year,
                                     runtime_minutes, genres, is_adult, title_type, rating, votes, last_synced_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (imdb_id) DO UPDATE SET
                title = EXCLUDED.title,
                original_title = EXCLUDED.original_title,
                start_year = EXCLUDED.start_year,
                end_year = EXCLUDED.end_year,
                runtime_minutes = EXCLUDED.runtime_minutes,
                genres = EXCLUDED.genres,
                is_adult = EXCLUDED.is_adult,
                title_type = EXCLUDED.title_type,
                rating = COALESCE(EXCLUDED.rating, imdb_series.rating),
                votes = COALESCE(EXCLUDED.votes, imdb_series.votes),
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(series.imdb_id)
        .bind(&series.title)
        .bind(&series.original_title)
        .bind(series.start_year)
        .bind(series.end_year)
        .bind(series.runtime_minutes)
        .bind(&series.genres)
        .bind(series.is_adult)
        .bind(&series.title_type)
        .bind(series.rating)
        .bind(series.votes)
        .bind(series.last_synced_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Upsert an episode
    pub async fn upsert_episode(&self, episode: &DbEpisode) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO imdb_episodes (imdb_id, parent_imdb_id, season_number, episode_number,
                                       title, runtime_minutes, rating, votes, air_date, last_synced_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (imdb_id) DO UPDATE SET
                parent_imdb_id = EXCLUDED.parent_imdb_id,
                season_number = EXCLUDED.season_number,
                episode_number = EXCLUDED.episode_number,
                title = COALESCE(EXCLUDED.title, imdb_episodes.title),
                runtime_minutes = COALESCE(EXCLUDED.runtime_minutes, imdb_episodes.runtime_minutes),
                rating = COALESCE(EXCLUDED.rating, imdb_episodes.rating),
                votes = COALESCE(EXCLUDED.votes, imdb_episodes.votes),
                air_date = COALESCE(EXCLUDED.air_date, imdb_episodes.air_date),
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(episode.imdb_id)
        .bind(episode.parent_imdb_id)
        .bind(episode.season_number)
        .bind(episode.episode_number)
        .bind(&episode.title)
        .bind(episode.runtime_minutes)
        .bind(episode.rating)
        .bind(episode.votes)
        .bind(episode.air_date)
        .bind(episode.last_synced_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update ratings for a series
    pub async fn update_series_rating(&self, imdb_id: i64, rating: f64, votes: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_series
            SET rating = $2, votes = $3, last_synced_at = NOW()
            WHERE imdb_id = $1
            "#,
        )
        .bind(imdb_id)
        .bind(rating)
        .bind(votes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Upsert a movie
    pub async fn upsert_movie(&self, movie: &DbMovie) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO imdb_movies (imdb_id, title, original_title, year,
                                     runtime_minutes, genres, is_adult, rating, votes, last_synced_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (imdb_id) DO UPDATE SET
                title = EXCLUDED.title,
                original_title = EXCLUDED.original_title,
                year = EXCLUDED.year,
                runtime_minutes = EXCLUDED.runtime_minutes,
                genres = EXCLUDED.genres,
                is_adult = EXCLUDED.is_adult,
                rating = COALESCE(EXCLUDED.rating, imdb_movies.rating),
                votes = COALESCE(EXCLUDED.votes, imdb_movies.votes),
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(movie.imdb_id)
        .bind(&movie.title)
        .bind(&movie.original_title)
        .bind(movie.year)
        .bind(movie.runtime_minutes)
        .bind(&movie.genres)
        .bind(movie.is_adult)
        .bind(movie.rating)
        .bind(movie.votes)
        .bind(movie.last_synced_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update ratings for a movie
    pub async fn update_movie_rating(&self, imdb_id: i64, rating: f64, votes: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_movies
            SET rating = $2, votes = $3, last_synced_at = NOW()
            WHERE imdb_id = $1
            "#,
        )
        .bind(imdb_id)
        .bind(rating)
        .bind(votes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Cache TMDB data for a movie (on-demand enrichment)
    pub async fn update_movie_tmdb_data(
        &self,
        imdb_id: i64,
        tmdb_id: Option<i64>,
        poster_url: Option<&str>,
        fanart_url: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_movies
            SET tmdb_id = $2, poster_url = $3, fanart_url = $4, tmdb_fetched_at = NOW()
            WHERE imdb_id = $1
            "#,
        )
        .bind(imdb_id)
        .bind(tmdb_id)
        .bind(poster_url)
        .bind(fanart_url)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ── Batch upsert methods (UNNEST-based) ──────────────────────────

    /// Batch upsert series using UNNEST arrays
    pub async fn upsert_series_batch(&self, batch: &[DbSeries]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut imdb_ids = Vec::with_capacity(batch.len());
        let mut titles = Vec::with_capacity(batch.len());
        let mut original_titles = Vec::with_capacity(batch.len());
        let mut start_years = Vec::with_capacity(batch.len());
        let mut end_years = Vec::with_capacity(batch.len());
        let mut runtime_mins = Vec::with_capacity(batch.len());
        let mut genres_vec = Vec::with_capacity(batch.len());
        let mut is_adults = Vec::with_capacity(batch.len());
        let mut title_types = Vec::with_capacity(batch.len());
        let mut ratings = Vec::with_capacity(batch.len());
        let mut votes_vec = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for s in batch {
            imdb_ids.push(s.imdb_id);
            titles.push(s.title.as_str());
            original_titles.push(s.original_title.as_deref());
            start_years.push(s.start_year);
            end_years.push(s.end_year);
            runtime_mins.push(s.runtime_minutes);
            genres_vec.push(s.genres.as_deref());
            is_adults.push(s.is_adult);
            title_types.push(s.title_type.as_str());
            ratings.push(s.rating);
            votes_vec.push(s.votes);
            synced_ats.push(s.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO imdb_series (imdb_id, title, original_title, start_year, end_year,
                                     runtime_minutes, genres, is_adult, title_type, rating, votes, last_synced_at)
            SELECT * FROM UNNEST(
                $1::bigint[], $2::text[], $3::text[], $4::int[], $5::int[],
                $6::int[], $7::text[], $8::bool[], $9::text[], $10::float8[], $11::bigint[], $12::timestamptz[]
            )
            ON CONFLICT (imdb_id) DO UPDATE SET
                title = EXCLUDED.title,
                original_title = EXCLUDED.original_title,
                start_year = EXCLUDED.start_year,
                end_year = EXCLUDED.end_year,
                runtime_minutes = EXCLUDED.runtime_minutes,
                genres = EXCLUDED.genres,
                is_adult = EXCLUDED.is_adult,
                title_type = EXCLUDED.title_type,
                rating = COALESCE(EXCLUDED.rating, imdb_series.rating),
                votes = COALESCE(EXCLUDED.votes, imdb_series.votes),
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&imdb_ids)
        .bind(&titles)
        .bind(&original_titles)
        .bind(&start_years)
        .bind(&end_years)
        .bind(&runtime_mins)
        .bind(&genres_vec)
        .bind(&is_adults)
        .bind(&title_types)
        .bind(&ratings)
        .bind(&votes_vec)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch upsert movies using UNNEST arrays
    pub async fn upsert_movie_batch(&self, batch: &[DbMovie]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut imdb_ids = Vec::with_capacity(batch.len());
        let mut titles = Vec::with_capacity(batch.len());
        let mut original_titles = Vec::with_capacity(batch.len());
        let mut years = Vec::with_capacity(batch.len());
        let mut runtime_mins = Vec::with_capacity(batch.len());
        let mut genres_vec = Vec::with_capacity(batch.len());
        let mut is_adults = Vec::with_capacity(batch.len());
        let mut ratings = Vec::with_capacity(batch.len());
        let mut votes_vec = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for m in batch {
            imdb_ids.push(m.imdb_id);
            titles.push(m.title.as_str());
            original_titles.push(m.original_title.as_deref());
            years.push(m.year);
            runtime_mins.push(m.runtime_minutes);
            genres_vec.push(m.genres.as_deref());
            is_adults.push(m.is_adult);
            ratings.push(m.rating);
            votes_vec.push(m.votes);
            synced_ats.push(m.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO imdb_movies (imdb_id, title, original_title, year,
                                     runtime_minutes, genres, is_adult, rating, votes, last_synced_at)
            SELECT * FROM UNNEST(
                $1::bigint[], $2::text[], $3::text[], $4::int[],
                $5::int[], $6::text[], $7::bool[], $8::float8[], $9::bigint[], $10::timestamptz[]
            )
            ON CONFLICT (imdb_id) DO UPDATE SET
                title = EXCLUDED.title,
                original_title = EXCLUDED.original_title,
                year = EXCLUDED.year,
                runtime_minutes = EXCLUDED.runtime_minutes,
                genres = EXCLUDED.genres,
                is_adult = EXCLUDED.is_adult,
                rating = COALESCE(EXCLUDED.rating, imdb_movies.rating),
                votes = COALESCE(EXCLUDED.votes, imdb_movies.votes),
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&imdb_ids)
        .bind(&titles)
        .bind(&original_titles)
        .bind(&years)
        .bind(&runtime_mins)
        .bind(&genres_vec)
        .bind(&is_adults)
        .bind(&ratings)
        .bind(&votes_vec)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch upsert episodes using UNNEST arrays
    pub async fn upsert_episode_batch(&self, batch: &[DbEpisode]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut imdb_ids = Vec::with_capacity(batch.len());
        let mut parent_ids = Vec::with_capacity(batch.len());
        let mut season_nums = Vec::with_capacity(batch.len());
        let mut episode_nums = Vec::with_capacity(batch.len());
        let mut titles = Vec::with_capacity(batch.len());
        let mut runtime_mins = Vec::with_capacity(batch.len());
        let mut ratings = Vec::with_capacity(batch.len());
        let mut votes_vec = Vec::with_capacity(batch.len());
        let mut air_dates = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for e in batch {
            imdb_ids.push(e.imdb_id);
            parent_ids.push(e.parent_imdb_id);
            season_nums.push(e.season_number);
            episode_nums.push(e.episode_number);
            titles.push(e.title.as_deref());
            runtime_mins.push(e.runtime_minutes);
            ratings.push(e.rating);
            votes_vec.push(e.votes);
            air_dates.push(e.air_date);
            synced_ats.push(e.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO imdb_episodes (imdb_id, parent_imdb_id, season_number, episode_number,
                                       title, runtime_minutes, rating, votes, air_date, last_synced_at)
            SELECT * FROM UNNEST(
                $1::bigint[], $2::bigint[], $3::int[], $4::int[],
                $5::text[], $6::int[], $7::float8[], $8::bigint[], $9::date[], $10::timestamptz[]
            )
            ON CONFLICT (imdb_id) DO UPDATE SET
                parent_imdb_id = EXCLUDED.parent_imdb_id,
                season_number = EXCLUDED.season_number,
                episode_number = EXCLUDED.episode_number,
                title = COALESCE(EXCLUDED.title, imdb_episodes.title),
                runtime_minutes = COALESCE(EXCLUDED.runtime_minutes, imdb_episodes.runtime_minutes),
                rating = COALESCE(EXCLUDED.rating, imdb_episodes.rating),
                votes = COALESCE(EXCLUDED.votes, imdb_episodes.votes),
                air_date = COALESCE(EXCLUDED.air_date, imdb_episodes.air_date),
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&imdb_ids)
        .bind(&parent_ids)
        .bind(&season_nums)
        .bind(&episode_nums)
        .bind(&titles)
        .bind(&runtime_mins)
        .bind(&ratings)
        .bind(&votes_vec)
        .bind(&air_dates)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch update ratings for series using UNNEST
    pub async fn update_series_ratings_batch(
        &self,
        ids: &[i64],
        ratings: &[f64],
        votes: &[i64],
    ) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let result = sqlx::query(
            r#"
            UPDATE imdb_series s
            SET rating = v.rating, votes = v.votes, last_synced_at = NOW()
            FROM (SELECT * FROM UNNEST($1::bigint[], $2::float8[], $3::bigint[]) AS t(id, rating, votes)) v
            WHERE s.imdb_id = v.id
            "#,
        )
        .bind(ids)
        .bind(ratings)
        .bind(votes)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch update ratings for movies using UNNEST
    pub async fn update_movie_ratings_batch(
        &self,
        ids: &[i64],
        ratings: &[f64],
        votes: &[i64],
    ) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let result = sqlx::query(
            r#"
            UPDATE imdb_movies m
            SET rating = v.rating, votes = v.votes, last_synced_at = NOW()
            FROM (SELECT * FROM UNNEST($1::bigint[], $2::float8[], $3::bigint[]) AS t(id, rating, votes)) v
            WHERE m.imdb_id = v.id
            "#,
        )
        .bind(ids)
        .bind(ratings)
        .bind(votes)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Air date backfill methods ───────────────────────────────────

    /// Get IMDB IDs of series that have episodes without air dates.
    /// Skips series that were attempted within the last 7 days to avoid
    /// re-querying TVMaze for shows that aren't in their database.
    /// Uses tvmaze_checked_at (not last_synced_at, which is set by IMDB sync).
    /// Orders by votes DESC so popular series (likely in TVMaze) are processed first.
    pub async fn get_series_needing_air_dates(&self, limit: i64) -> Result<Vec<i64>> {
        let ids: Vec<i64> = sqlx::query_scalar(
            r#"
            SELECT e.parent_imdb_id
            FROM imdb_episodes e
            JOIN imdb_series s ON s.imdb_id = e.parent_imdb_id
            WHERE e.air_date IS NULL
              AND (e.tvmaze_checked_at IS NULL OR e.tvmaze_checked_at < NOW() - INTERVAL '7 days')
            GROUP BY e.parent_imdb_id, s.votes
            ORDER BY s.votes DESC NULLS LAST
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(ids)
    }

    /// Mark episodes as attempted so we don't re-query TVMaze for shows
    /// that aren't in their database on every backfill run.
    /// Uses tvmaze_checked_at (not last_synced_at, which is set by IMDB sync).
    pub async fn mark_episodes_attempted(&self, parent_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE imdb_episodes
            SET tvmaze_checked_at = NOW()
            WHERE parent_imdb_id = $1 AND air_date IS NULL
            "#,
        )
        .bind(parent_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Batch update episode air dates and titles using UNNEST arrays.
    /// Matches on (parent_imdb_id, season_number, episode_number).
    pub async fn update_episode_air_dates(
        &self,
        parent_id: i64,
        seasons: &[i32],
        episodes: &[i32],
        air_dates: &[chrono::NaiveDate],
        titles: &[Option<&str>],
    ) -> Result<u64> {
        if seasons.is_empty() {
            return Ok(0);
        }

        let result = sqlx::query(
            r#"
            UPDATE imdb_episodes e
            SET air_date = v.air_date,
                title = COALESCE(v.title, e.title),
                tvmaze_checked_at = NOW()
            FROM (
                SELECT *
                FROM UNNEST($1::int[], $2::int[], $3::date[], $4::text[])
                    AS t(season, episode, air_date, title)
            ) v
            WHERE e.parent_imdb_id = $5
              AND e.season_number = v.season
              AND e.episode_number = v.episode
            "#,
        )
        .bind(seasons)
        .bind(episodes)
        .bind(air_dates)
        .bind(titles)
        .bind(parent_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── People & Credits methods ───────────────────────────────────

    /// Batch upsert people using UNNEST arrays
    pub async fn upsert_people_batch(&self, batch: &[DbPerson]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut nconsts = Vec::with_capacity(batch.len());
        let mut names = Vec::with_capacity(batch.len());
        let mut birth_years = Vec::with_capacity(batch.len());
        let mut death_years = Vec::with_capacity(batch.len());
        let mut professions = Vec::with_capacity(batch.len());
        let mut known_fors = Vec::with_capacity(batch.len());

        for p in batch {
            nconsts.push(p.nconst);
            names.push(p.primary_name.as_str());
            birth_years.push(p.birth_year);
            death_years.push(p.death_year);
            professions.push(p.primary_profession.as_deref());
            known_fors.push(p.known_for_titles.as_deref());
        }

        let result = sqlx::query(
            r#"
            INSERT INTO imdb_people (nconst, primary_name, birth_year, death_year, primary_profession, known_for_titles)
            SELECT * FROM UNNEST(
                $1::bigint[], $2::text[], $3::smallint[], $4::smallint[], $5::text[], $6::text[]
            )
            ON CONFLICT (nconst) DO UPDATE SET
                primary_name = EXCLUDED.primary_name,
                birth_year = EXCLUDED.birth_year,
                death_year = EXCLUDED.death_year,
                primary_profession = EXCLUDED.primary_profession,
                known_for_titles = EXCLUDED.known_for_titles
            "#,
        )
        .bind(&nconsts)
        .bind(&names)
        .bind(&birth_years)
        .bind(&death_years)
        .bind(&professions)
        .bind(&known_fors)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch upsert credits using UNNEST arrays
    pub async fn upsert_credits_batch(&self, batch: &[DbCredit]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut tconsts = Vec::with_capacity(batch.len());
        let mut nconsts = Vec::with_capacity(batch.len());
        let mut orderings = Vec::with_capacity(batch.len());
        let mut categories = Vec::with_capacity(batch.len());
        let mut jobs = Vec::with_capacity(batch.len());
        let mut characters_vec = Vec::with_capacity(batch.len());

        for c in batch {
            tconsts.push(c.tconst);
            nconsts.push(c.nconst);
            orderings.push(c.ordering);
            categories.push(c.category.as_str());
            jobs.push(c.job.as_deref());
            characters_vec.push(c.characters.as_deref());
        }

        let result = sqlx::query(
            r#"
            INSERT INTO imdb_credits (tconst, nconst, ordering, category, job, characters)
            SELECT * FROM UNNEST(
                $1::bigint[], $2::bigint[], $3::smallint[], $4::text[], $5::text[], $6::text[]
            )
            ON CONFLICT (tconst, nconst, ordering) DO UPDATE SET
                category = EXCLUDED.category,
                job = EXCLUDED.job,
                characters = EXCLUDED.characters
            "#,
        )
        .bind(&tconsts)
        .bind(&nconsts)
        .bind(&orderings)
        .bind(&categories)
        .bind(&jobs)
        .bind(&characters_vec)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get credits for a title (JOIN with people for names), ordered by billing
    pub async fn get_credits_for_title(&self, imdb_id: &str) -> Result<Vec<ImdbCredit>> {
        let numeric_id = parse_imdb_id(imdb_id).ok_or_else(|| anyhow::anyhow!("Invalid IMDB ID"))?;

        let rows = sqlx::query(
            r#"
            SELECT c.nconst, p.primary_name, c.ordering, c.category, c.job, c.characters
            FROM imdb_credits c
            JOIN imdb_people p ON p.nconst = c.nconst
            WHERE c.tconst = $1
            ORDER BY c.ordering
            "#,
        )
        .bind(numeric_id)
        .fetch_all(&self.pool)
        .await?;

        let credits = rows
            .iter()
            .map(|row| {
                let nconst: i64 = row.get("nconst");
                let characters_raw: Option<String> = row.get("characters");
                // IMDB stores characters as JSON array string: ["Char1","Char2"]
                let characters = characters_raw.and_then(|s| {
                    serde_json::from_str::<Vec<String>>(&s).ok()
                });

                ImdbCredit {
                    nconst: format!("nm{:07}", nconst),
                    name: row.get("primary_name"),
                    category: row.get("category"),
                    job: row.get("job"),
                    characters,
                    ordering: row.get("ordering"),
                }
            })
            .collect();

        Ok(credits)
    }

    /// Get a person by nconst
    pub async fn get_person(&self, nconst: &str) -> Result<Option<ImdbPerson>> {
        let numeric_id = parse_nconst(nconst).ok_or_else(|| anyhow::anyhow!("Invalid nconst"))?;

        let row = sqlx::query(
            r#"
            SELECT nconst, primary_name, birth_year, death_year, primary_profession, known_for_titles
            FROM imdb_people
            WHERE nconst = $1
            "#,
        )
        .bind(numeric_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| {
            let nconst_val: i64 = r.get("nconst");
            let profession_str: Option<String> = r.get("primary_profession");
            let known_for_str: Option<String> = r.get("known_for_titles");

            ImdbPerson {
                nconst: format!("nm{:07}", nconst_val),
                name: r.get("primary_name"),
                birth_year: r.get("birth_year"),
                death_year: r.get("death_year"),
                professions: profession_str
                    .map(|s| s.split(',').map(String::from).collect())
                    .unwrap_or_default(),
                known_for: known_for_str
                    .map(|s| {
                        s.split(',')
                            .filter_map(|t| t.trim().parse::<i64>().ok())
                            .map(|id| format!("tt{:07}", id))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        }))
    }

    /// Get pool for direct access (used by sync)
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
