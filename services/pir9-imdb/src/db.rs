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
    pub async fn search_series(&self, query: &str, limit: u32) -> Result<Vec<ImdbSeries>> {
        let search_pattern = format!("%{}%", query);
        let limit = limit.min(100) as i64;

        let rows = sqlx::query(
            r#"
            SELECT imdb_id, title, original_title, start_year, end_year,
                   runtime_minutes, genres, is_adult, title_type, rating, votes, last_synced_at
            FROM imdb_series
            WHERE title ILIKE $1 OR original_title ILIKE $1
            ORDER BY votes DESC NULLS LAST, rating DESC NULLS LAST
            LIMIT $2
            "#,
        )
        .bind(&search_pattern)
        .bind(limit)
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
            }))
        }

        Ok(SyncStatus {
            is_running: false, // Enriched by the handler from the in-memory SyncHandle
            title_basics: get_dataset_status(&self.pool, "title.basics.tsv.gz").await?,
            title_episodes: get_dataset_status(&self.pool, "title.episode.tsv.gz").await?,
            title_ratings: get_dataset_status(&self.pool, "title.ratings.tsv.gz").await?,
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

    /// Get pool for direct access (used by sync)
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
