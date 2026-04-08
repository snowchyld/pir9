#![allow(dead_code)]
//! Database repositories
//! Repository pattern implementations for database access

use anyhow::Result;
use sqlx::Row;

use crate::core::datastore::Database;

/// Base repository trait
#[async_trait::async_trait]
pub trait Repository<T, ID> {
    async fn find_by_id(&self, id: ID) -> Result<Option<T>>;
    async fn find_all(&self) -> Result<Vec<T>>;
    async fn insert(&self, entity: &T) -> Result<T>;
    async fn update(&self, entity: &T) -> Result<T>;
    async fn delete(&self, id: ID) -> Result<()>;
}

/// Generic repository implementation
pub struct BaseRepository<T> {
    db: Database,
    table_name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> BaseRepository<T> {
    pub fn new(db: Database, table_name: &str) -> Self {
        Self {
            db,
            table_name: table_name.to_string(),
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Repository for tags
pub struct TagRepository {
    db: Database,
}

impl TagRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<(i64, String)>> {
        let pool = self.db.pool();
        let rows = sqlx::query("SELECT id, label FROM tags ORDER BY label")
            .fetch_all(pool)
            .await?;
        let mut tags = Vec::new();
        for row in rows {
            tags.push((row.try_get("id")?, row.try_get("label")?));
        }
        Ok(tags)
    }

    pub async fn insert(&self, label: &str) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as("INSERT INTO tags (label) VALUES ($1) RETURNING id")
            .bind(label)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM tags WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update(&self, id: i64, label: &str) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("UPDATE tags SET label = $1 WHERE id = $2")
            .bind(label)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for episode files
pub struct EpisodeFileRepository {
    db: Database,
}

impl EpisodeFileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::EpisodeFileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::EpisodeFileDbModel>(
            "SELECT * FROM episode_files ORDER BY date_added DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::EpisodeFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeFileDbModel>(
            "SELECT * FROM episode_files WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_series_id(
        &self,
        series_id: i64,
    ) -> Result<Vec<super::models::EpisodeFileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::EpisodeFileDbModel>(
            "SELECT * FROM episode_files WHERE series_id = $1 ORDER BY season_number, relative_path"
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_path(
        &self,
        path: &str,
    ) -> Result<Option<super::models::EpisodeFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeFileDbModel>(
            "SELECT * FROM episode_files WHERE path = $1",
        )
        .bind(path)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, episode_file: &super::models::EpisodeFileDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO episode_files (
                series_id, season_number, relative_path, path, size,
                date_added, scene_name, release_group, quality, languages,
                media_info, original_file_path, file_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id
            "#,
        )
        .bind(episode_file.series_id)
        .bind(episode_file.season_number)
        .bind(&episode_file.relative_path)
        .bind(&episode_file.path)
        .bind(episode_file.size)
        .bind(episode_file.date_added)
        .bind(&episode_file.scene_name)
        .bind(&episode_file.release_group)
        .bind(&episode_file.quality)
        .bind(&episode_file.languages)
        .bind(&episode_file.media_info)
        .bind(&episode_file.original_file_path)
        .bind(&episode_file.file_hash)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, episode_file: &super::models::EpisodeFileDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE episode_files SET
                series_id = $1, season_number = $2, relative_path = $3, path = $4, size = $5,
                date_added = $6, scene_name = $7, release_group = $8, quality = $9, languages = $10,
                media_info = $11, original_file_path = $12, file_hash = $13
            WHERE id = $14
            "#,
        )
        .bind(episode_file.series_id)
        .bind(episode_file.season_number)
        .bind(&episode_file.relative_path)
        .bind(&episode_file.path)
        .bind(episode_file.size)
        .bind(episode_file.date_added)
        .bind(&episode_file.scene_name)
        .bind(&episode_file.release_group)
        .bind(&episode_file.quality)
        .bind(&episode_file.languages)
        .bind(&episode_file.media_info)
        .bind(&episode_file.original_file_path)
        .bind(&episode_file.file_hash)
        .bind(episode_file.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM episode_files WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_by_series_id(&self, series_id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM episode_files WHERE series_id = $1")
            .bind(series_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for history
pub struct HistoryRepository {
    db: Database,
}

impl HistoryRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_paged(
        &self,
        page: i32,
        page_size: i32,
        sort_key: &str,
        sort_direction: &str,
        event_type: Option<i32>,
    ) -> Result<(Vec<super::models::HistoryDbModel>, i64)> {
        let offset = (page - 1) * page_size;
        let order = if sort_direction.to_lowercase() == "ascending" {
            "ASC"
        } else {
            "DESC"
        };
        let sort_column = match sort_key {
            "date" => "date",
            "series.sortTitle" | "seriesTitle" => "series_id",
            "sourceTitle" => "source_title",
            "eventType" => "event_type",
            _ => "date",
        };

        let pool = self.db.pool();

        if let Some(evt) = event_type {
            let query = format!(
                "SELECT * FROM history WHERE event_type = $1 ORDER BY {} {} LIMIT $2 OFFSET $3",
                sort_column, order
            );
            let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(&query)
                .bind(evt)
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?;
            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM history WHERE event_type = $1")
                    .bind(evt)
                    .fetch_one(pool)
                    .await?;
            Ok((rows, count.0))
        } else {
            let query = format!(
                "SELECT * FROM history ORDER BY {} {} LIMIT $1 OFFSET $2",
                sort_column, order
            );
            let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(&query)
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?;
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM history")
                .fetch_one(pool)
                .await?;
            Ok((rows, count.0))
        }
    }

    pub async fn get_since(
        &self,
        date: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            "SELECT * FROM history WHERE date >= $1 ORDER BY date DESC",
        )
        .bind(date)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_series_id(
        &self,
        series_id: i64,
    ) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            "SELECT * FROM history WHERE series_id = $1 ORDER BY date DESC",
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_episode_id(
        &self,
        episode_id: i64,
    ) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            "SELECT * FROM history WHERE episode_id = $1 ORDER BY date DESC",
        )
        .bind(episode_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_for_series(
        &self,
        series_id: i64,
        limit: i32,
    ) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            "SELECT * FROM history WHERE series_id = $1 ORDER BY date DESC LIMIT $2",
        )
        .bind(series_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn insert(&self, history: &super::models::HistoryDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO history (
                series_id, episode_id, movie_id, source_title, quality, languages,
                custom_formats, custom_format_score, quality_cutoff_not_met,
                date, download_id, event_type, data
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id
            "#,
        )
        .bind(history.series_id)
        .bind(history.episode_id)
        .bind(history.movie_id)
        .bind(&history.source_title)
        .bind(&history.quality)
        .bind(&history.languages)
        .bind(&history.custom_formats)
        .bind(history.custom_format_score)
        .bind(history.quality_cutoff_not_met)
        .bind(history.date)
        .bind(&history.download_id)
        .bind(history.event_type)
        .bind(&history.data)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM history WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_bulk(&self, ids: &[i64]) -> Result<u64> {
        let pool = self.db.pool();
        let mut total = 0u64;
        for id in ids {
            let result = sqlx::query("DELETE FROM history WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            total += result.rows_affected();
        }
        Ok(total)
    }

    pub async fn clear_by_event_type(&self, event_type: i32) -> Result<u64> {
        let pool = self.db.pool();
        let result = sqlx::query("DELETE FROM history WHERE event_type = $1")
            .bind(event_type)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Get recent completed downloads for the queue "Completed" tab.
    /// Returns the latest history entry per download_id for Grabbed (1)
    /// and DownloadFolderImported (3) events.
    pub async fn get_recent_completed(
        &self,
        limit: i32,
    ) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            r#"
            SELECT DISTINCT ON (download_id) *
            FROM history
            WHERE event_type IN (1, 3)
              AND download_id IS NOT NULL
              AND download_id != ''
            ORDER BY download_id, date DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

/// Repository for root folders
pub struct RootFolderRepository {
    db: Database,
}

impl RootFolderRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::RootFolderDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::RootFolderDbModel>(
            "SELECT * FROM root_folders ORDER BY path",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::RootFolderDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::RootFolderDbModel>(
            "SELECT * FROM root_folders WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, path: &str, content_type: &str) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            "INSERT INTO root_folders (path, accessible, content_type) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(path)
        .bind(true)
        .bind(content_type)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn get_by_content_types(
        &self,
        types: &[String],
    ) -> Result<Vec<super::models::RootFolderDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::RootFolderDbModel>(
            "SELECT * FROM root_folders WHERE content_type = ANY($1) ORDER BY path",
        )
        .bind(types)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM root_folders WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for series
pub struct SeriesRepository {
    db: Database,
}

impl SeriesRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::SeriesDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::SeriesDbModel>(
            "SELECT * FROM series ORDER BY sort_title",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::SeriesDbModel>> {
        let pool = self.db.pool();
        let row =
            sqlx::query_as::<_, super::models::SeriesDbModel>("SELECT * FROM series WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        Ok(row)
    }

    pub async fn get_by_tvdb_id(
        &self,
        tvdb_id: i64,
    ) -> Result<Option<super::models::SeriesDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::SeriesDbModel>(
            "SELECT * FROM series WHERE tvdb_id = $1",
        )
        .bind(tvdb_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_imdb_id(
        &self,
        imdb_id: &str,
    ) -> Result<Option<super::models::SeriesDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::SeriesDbModel>(
            "SELECT * FROM series WHERE imdb_id = $1",
        )
        .bind(imdb_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, series: &super::models::SeriesDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO series (
                tvdb_id, tv_rage_id, tv_maze_id, imdb_id, tmdb_id,
                title, clean_title, sort_title, status, overview,
                monitored, monitor_new_items, quality_profile_id, language_profile_id,
                season_folder, series_type, title_slug, path, root_folder_path,
                year, first_aired, last_aired, runtime, network, certification,
                use_scene_numbering, episode_ordering, added, last_info_sync,
                imdb_rating, imdb_votes
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9, $10,
                $11, $12, $13, $14,
                $15, $16, $17, $18, $19,
                $20, $21, $22, $23, $24, $25,
                $26, $27, $28, $29,
                $30, $31
            ) RETURNING id
            "#,
        )
        .bind(series.tvdb_id)
        .bind(series.tv_rage_id)
        .bind(series.tv_maze_id)
        .bind(&series.imdb_id)
        .bind(series.tmdb_id)
        .bind(&series.title)
        .bind(&series.clean_title)
        .bind(&series.sort_title)
        .bind(series.status)
        .bind(&series.overview)
        .bind(series.monitored)
        .bind(series.monitor_new_items)
        .bind(series.quality_profile_id)
        .bind(series.language_profile_id)
        .bind(series.season_folder)
        .bind(series.series_type)
        .bind(&series.title_slug)
        .bind(&series.path)
        .bind(&series.root_folder_path)
        .bind(series.year)
        .bind(series.first_aired)
        .bind(series.last_aired)
        .bind(series.runtime)
        .bind(&series.network)
        .bind(&series.certification)
        .bind(series.use_scene_numbering)
        .bind(&series.episode_ordering)
        .bind(series.added)
        .bind(series.last_info_sync)
        .bind(series.imdb_rating)
        .bind(series.imdb_votes)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, series: &super::models::SeriesDbModel) -> Result<()> {
        let pool = self.db.pool();

        // Resolve tvdb_id conflicts: clear the conflicting series' tvdb_id
        if series.tvdb_id > 0 {
            if let Some(existing) = self.get_by_tvdb_id(series.tvdb_id).await? {
                if existing.id != series.id {
                    tracing::warn!(
                        "Series update: tvdb_id {} conflicts with series id={} '{}', clearing conflicting tvdb_id",
                        series.tvdb_id, existing.id, existing.title
                    );
                    let _ = sqlx::query("UPDATE series SET tvdb_id = 0 WHERE id = $1")
                        .bind(existing.id)
                        .execute(pool)
                        .await;
                }
            }
        }

        sqlx::query(
            r#"
            UPDATE series SET
                tvdb_id = $1, tv_rage_id = $2, tv_maze_id = $3, imdb_id = $4, tmdb_id = $5,
                title = $6, clean_title = $7, sort_title = $8, status = $9, overview = $10,
                monitored = $11, monitor_new_items = $12, quality_profile_id = $13, language_profile_id = $14,
                season_folder = $15, series_type = $16, title_slug = $17, path = $18, root_folder_path = $19,
                year = $20, first_aired = $21, last_aired = $22, runtime = $23, network = $24, certification = $25,
                use_scene_numbering = $26, episode_ordering = $27, last_info_sync = $28,
                imdb_rating = $29, imdb_votes = $30
            WHERE id = $31
            "#
        )
        .bind(series.tvdb_id)
        .bind(series.tv_rage_id)
        .bind(series.tv_maze_id)
        .bind(&series.imdb_id)
        .bind(series.tmdb_id)
        .bind(&series.title)
        .bind(&series.clean_title)
        .bind(&series.sort_title)
        .bind(series.status)
        .bind(&series.overview)
        .bind(series.monitored)
        .bind(series.monitor_new_items)
        .bind(series.quality_profile_id)
        .bind(series.language_profile_id)
        .bind(series.season_folder)
        .bind(series.series_type)
        .bind(&series.title_slug)
        .bind(&series.path)
        .bind(&series.root_folder_path)
        .bind(series.year)
        .bind(series.first_aired)
        .bind(series.last_aired)
        .bind(series.runtime)
        .bind(&series.network)
        .bind(&series.certification)
        .bind(series.use_scene_numbering)
        .bind(&series.episode_ordering)
        .bind(series.last_info_sync)
        .bind(series.imdb_rating)
        .bind(series.imdb_votes)
        .bind(series.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM series WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for movies
pub struct MovieRepository {
    db: Database,
}

impl MovieRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::MovieDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::MovieDbModel>(
            "SELECT * FROM movies ORDER BY sort_title",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::MovieDbModel>> {
        let pool = self.db.pool();
        let row =
            sqlx::query_as::<_, super::models::MovieDbModel>("SELECT * FROM movies WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        Ok(row)
    }

    pub async fn get_by_tmdb_id(
        &self,
        tmdb_id: i64,
    ) -> Result<Option<super::models::MovieDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::MovieDbModel>(
            "SELECT * FROM movies WHERE tmdb_id = $1",
        )
        .bind(tmdb_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_imdb_id(
        &self,
        imdb_id: &str,
    ) -> Result<Option<super::models::MovieDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::MovieDbModel>(
            "SELECT * FROM movies WHERE imdb_id = $1",
        )
        .bind(imdb_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_path(&self, path: &str) -> Result<Option<super::models::MovieDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::MovieDbModel>(
            "SELECT * FROM movies WHERE path = $1",
        )
        .bind(path)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, movie: &super::models::MovieDbModel) -> Result<i64> {
        let pool = self.db.pool();
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
            ) RETURNING id
            "#,
        )
        .bind(movie.tmdb_id)
        .bind(&movie.imdb_id)
        .bind(&movie.title)
        .bind(&movie.clean_title)
        .bind(&movie.sort_title)
        .bind(movie.status)
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
        .bind(&movie.genres)
        .bind(&movie.tags)
        .bind(&movie.images)
        .bind(movie.has_file)
        .bind(movie.movie_file_id)
        .bind(movie.added)
        .bind(movie.last_info_sync)
        .bind(movie.imdb_rating)
        .bind(movie.imdb_votes)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, movie: &super::models::MovieDbModel) -> Result<()> {
        let pool = self.db.pool();

        // Resolve tmdb_id conflicts: mark conflicting movie as [DUPE]
        if movie.tmdb_id > 0 {
            if let Some(existing) = self.get_by_tmdb_id(movie.tmdb_id).await? {
                if existing.id != movie.id {
                    tracing::warn!(
                        "Movie update: tmdb_id {} conflicts with movie id={} '{}', marking as duplicate",
                        movie.tmdb_id, existing.id, existing.title
                    );
                    let dupe_title = if existing.title.starts_with("[DUPE] ") {
                        existing.title.clone()
                    } else {
                        format!("[DUPE] {}", existing.title)
                    };
                    let _ = sqlx::query(
                        "UPDATE movies SET tmdb_id = 0, title = $1, clean_title = $2 WHERE id = $3",
                    )
                    .bind(&dupe_title)
                    .bind(&dupe_title.to_lowercase().replace(' ', ""))
                    .bind(existing.id)
                    .execute(pool)
                    .await;
                }
            }
        }

        // Resolve imdb_id conflicts: mark conflicting movie as [DUPE]
        if let Some(ref imdb_id) = movie.imdb_id {
            if !imdb_id.is_empty() {
                if let Some(existing) = self.get_by_imdb_id(imdb_id).await? {
                    if existing.id != movie.id {
                        tracing::warn!(
                            "Movie update: imdb_id {} conflicts with movie id={} '{}', marking as duplicate",
                            imdb_id, existing.id, existing.title
                        );
                        let dupe_title = if existing.title.starts_with("[DUPE] ") {
                            existing.title.clone()
                        } else {
                            format!("[DUPE] {}", existing.title)
                        };
                        let _ = sqlx::query(
                            "UPDATE movies SET imdb_id = NULL, title = $1, clean_title = $2 WHERE id = $3",
                        )
                        .bind(&dupe_title)
                        .bind(&dupe_title.to_lowercase().replace(' ', ""))
                        .bind(existing.id)
                        .execute(pool)
                        .await;
                    }
                }
            }
        }

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
        .bind(movie.status)
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
        .bind(&movie.genres)
        .bind(&movie.tags)
        .bind(&movie.images)
        .bind(movie.has_file)
        .bind(movie.movie_file_id)
        .bind(movie.last_info_sync)
        .bind(movie.imdb_rating)
        .bind(movie.imdb_votes)
        .bind(movie.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM movies WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Get missing movies (monitored, no file) with pagination
    pub async fn get_missing(
        &self,
        monitored_only: bool,
        page: i32,
        page_size: i32,
        sort_key: &str,
        sort_direction: &str,
    ) -> Result<(Vec<super::models::MovieDbModel>, i64)> {
        let order = if sort_direction.to_lowercase() == "descending" {
            "DESC"
        } else {
            "ASC"
        };
        let order_by = match sort_key {
            "title" | "sortTitle" => "sort_title",
            "year" => "year",
            "added" => "added",
            _ => "sort_title",
        };
        let offset = (page - 1) * page_size;

        let pool = self.db.pool();
        let mut base_where = "has_file = false".to_string();
        if monitored_only {
            base_where.push_str(" AND monitored = true");
        }

        let count_sql = format!("SELECT COUNT(*) FROM movies WHERE {}", base_where);
        let total: (i64,) = sqlx::query_as(&count_sql).fetch_one(pool).await?;

        let data_sql = format!(
            "SELECT * FROM movies WHERE {} ORDER BY {} {} LIMIT $1 OFFSET $2",
            base_where, order_by, order
        );
        let rows = sqlx::query_as::<_, super::models::MovieDbModel>(&data_sql)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        Ok((rows, total.0))
    }
}

/// Repository for movie files
pub struct MovieFileRepository {
    db: Database,
}

impl MovieFileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::MovieFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::MovieFileDbModel>(
            "SELECT * FROM movie_files WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_movie_id(
        &self,
        movie_id: i64,
    ) -> Result<Option<super::models::MovieFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::MovieFileDbModel>(
            "SELECT * FROM movie_files WHERE movie_id = $1",
        )
        .bind(movie_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, file: &super::models::MovieFileDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO movie_files (
                movie_id, relative_path, path, size, date_added,
                scene_name, release_group, quality, languages,
                media_info, original_file_path, edition, file_hash
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            ) RETURNING id
            "#,
        )
        .bind(file.movie_id)
        .bind(&file.relative_path)
        .bind(&file.path)
        .bind(file.size)
        .bind(file.date_added)
        .bind(&file.scene_name)
        .bind(&file.release_group)
        .bind(&file.quality)
        .bind(&file.languages)
        .bind(&file.media_info)
        .bind(&file.original_file_path)
        .bind(&file.edition)
        .bind(&file.file_hash)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM movie_files WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for episodes
pub struct EpisodeRepository {
    db: Database,
}

impl EpisodeRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes ORDER BY series_id, season_number, episode_number",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_series_id(
        &self,
        series_id: i64,
    ) -> Result<Vec<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE series_id = $1 ORDER BY season_number, episode_number",
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_imdb_id(
        &self,
        imdb_id: &str,
    ) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE imdb_id = $1",
        )
        .bind(imdb_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, episode: &super::models::EpisodeDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO episodes (
                series_id, tvdb_id, episode_file_id, season_number, episode_number,
                absolute_episode_number, scene_absolute_episode_number, scene_episode_number,
                scene_season_number, title, overview, air_date, air_date_utc, runtime,
                has_file, monitored, unverified_scene_numbering, added, last_search_time,
                imdb_id, imdb_rating, imdb_votes
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8,
                $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19,
                $20, $21, $22
            ) RETURNING id
            "#,
        )
        .bind(episode.series_id)
        .bind(episode.tvdb_id)
        .bind(episode.episode_file_id)
        .bind(episode.season_number)
        .bind(episode.episode_number)
        .bind(episode.absolute_episode_number)
        .bind(episode.scene_absolute_episode_number)
        .bind(episode.scene_episode_number)
        .bind(episode.scene_season_number)
        .bind(&episode.title)
        .bind(&episode.overview)
        .bind(episode.air_date)
        .bind(episode.air_date_utc)
        .bind(episode.runtime)
        .bind(episode.has_file)
        .bind(episode.monitored)
        .bind(episode.unverified_scene_numbering)
        .bind(episode.added)
        .bind(episode.last_search_time)
        .bind(&episode.imdb_id)
        .bind(episode.imdb_rating)
        .bind(episode.imdb_votes)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, episode: &super::models::EpisodeDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE episodes SET
                series_id = $1, tvdb_id = $2, episode_file_id = $3, season_number = $4, episode_number = $5,
                absolute_episode_number = $6, scene_absolute_episode_number = $7, scene_episode_number = $8,
                scene_season_number = $9, title = $10, overview = $11, air_date = $12, air_date_utc = $13, runtime = $14,
                has_file = $15, monitored = $16, unverified_scene_numbering = $17, last_search_time = $18,
                imdb_id = $19, imdb_rating = $20, imdb_votes = $21
            WHERE id = $22
            "#
        )
        .bind(episode.series_id)
        .bind(episode.tvdb_id)
        .bind(episode.episode_file_id)
        .bind(episode.season_number)
        .bind(episode.episode_number)
        .bind(episode.absolute_episode_number)
        .bind(episode.scene_absolute_episode_number)
        .bind(episode.scene_episode_number)
        .bind(episode.scene_season_number)
        .bind(&episode.title)
        .bind(&episode.overview)
        .bind(episode.air_date)
        .bind(episode.air_date_utc)
        .bind(episode.runtime)
        .bind(episode.has_file)
        .bind(episode.monitored)
        .bind(episode.unverified_scene_numbering)
        .bind(episode.last_search_time)
        .bind(&episode.imdb_id)
        .bind(episode.imdb_rating)
        .bind(episode.imdb_votes)
        .bind(episode.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM episodes WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_by_series_id(&self, series_id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM episodes WHERE series_id = $1")
            .bind(series_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn get_by_series_and_season(
        &self,
        series_id: i64,
        season_number: i32,
    ) -> Result<Vec<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE series_id = $1 AND season_number = $2 ORDER BY episode_number"
        )
        .bind(series_id)
        .bind(season_number)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_series_season_episode(
        &self,
        series_id: i64,
        season_number: i32,
        episode_number: i32,
    ) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        // Primary: match by current season/episode numbers
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE series_id = $1 AND season_number = $2 AND episode_number = $3"
        )
        .bind(series_id)
        .bind(season_number)
        .bind(episode_number)
        .fetch_optional(pool)
        .await?;
        if row.is_some() {
            return Ok(row);
        }
        // Fallback: match by scene (original aired) numbers — handles DVD/alternate orderings
        // where season_number/episode_number hold the remapped values but filenames use aired numbers
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE series_id = $1 AND scene_season_number = $2 AND scene_episode_number = $3"
        )
        .bind(series_id)
        .bind(season_number)
        .bind(episode_number)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_series_and_absolute(
        &self,
        series_id: i64,
        absolute_episode_number: i32,
    ) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE series_id = $1 AND absolute_episode_number = $2",
        )
        .bind(series_id)
        .bind(absolute_episode_number)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_tvdb_id(
        &self,
        tvdb_id: i64,
    ) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE tvdb_id = $1",
        )
        .bind(tvdb_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn update_monitored(
        &self,
        episode_ids: &[i64],
        monitored: bool,
    ) -> Result<Vec<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        for id in episode_ids {
            sqlx::query("UPDATE episodes SET monitored = $1 WHERE id = $2")
                .bind(monitored)
                .bind(id)
                .execute(pool)
                .await?;
        }

        // Fetch updated episodes
        let mut episodes = Vec::new();
        for id in episode_ids {
            if let Some(ep) = self.get_by_id(*id).await? {
                episodes.push(ep);
            }
        }

        Ok(episodes)
    }

    /// Get missing episodes (monitored, no file, aired in past)
    ///
    /// `exclude_episode_ids` omits episodes that are actively downloading from both
    /// the result set and the total count, so pagination stays accurate.
    /// `series_types` optionally filters by series.series_type (0=standard, 1=daily, 2=anime).
    pub async fn get_missing(
        &self,
        monitored_only: bool,
        page: i32,
        page_size: i32,
        sort_key: &str,
        sort_direction: &str,
        exclude_episode_ids: &[i64],
        series_types: Option<&[i32]>,
    ) -> Result<(Vec<super::models::EpisodeDbModel>, i64)> {
        let order = if sort_direction.to_lowercase() == "descending" {
            "DESC"
        } else {
            "ASC"
        };
        let order_by = match sort_key {
            "airDateUtc" | "airDate" => "air_date_utc",
            "seriesTitle" => "series_id",
            "episodeTitle" | "title" => "title",
            "episodeNumber" => "season_number, episode_number",
            _ => "air_date_utc",
        };
        let offset = (page - 1) * page_size;
        let has_exclusions = !exclude_episode_ids.is_empty();

        let pool = self.db.pool();
        let mut base_where =
            "has_file = false AND air_date_utc IS NOT NULL AND air_date_utc < NOW() AND season_number > 0".to_string();
        if monitored_only {
            base_where.push_str(" AND monitored = true");
        }
        // Filter by series type via subquery (values are i32 literals, safe to inline)
        if let Some(types) = series_types {
            if !types.is_empty() {
                let type_list = types
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                base_where.push_str(&format!(
                    " AND series_id IN (SELECT id FROM series WHERE series_type IN ({}))",
                    type_list
                ));
            }
        }

        // Count query: exclude IDs are $1 (no LIMIT/OFFSET params)
        let total: (i64,) = if has_exclusions {
            let count_sql = format!(
                "SELECT COUNT(*) FROM episodes WHERE {} AND id NOT IN (SELECT UNNEST($1::bigint[]))",
                base_where
            );
            sqlx::query_as(&count_sql)
                .bind(exclude_episode_ids)
                .fetch_one(pool)
                .await?
        } else {
            let count_sql = format!("SELECT COUNT(*) FROM episodes WHERE {}", base_where);
            sqlx::query_as(&count_sql).fetch_one(pool).await?
        };

        // Data query: LIMIT=$1, OFFSET=$2, exclude IDs=$3
        let rows = if has_exclusions {
            let data_sql = format!(
                "SELECT * FROM episodes WHERE {} AND id NOT IN (SELECT UNNEST($3::bigint[])) ORDER BY {} {} LIMIT $1 OFFSET $2",
                base_where, order_by, order
            );
            sqlx::query_as::<_, super::models::EpisodeDbModel>(&data_sql)
                .bind(page_size)
                .bind(offset)
                .bind(exclude_episode_ids)
                .fetch_all(pool)
                .await?
        } else {
            let data_sql = format!(
                "SELECT * FROM episodes WHERE {} ORDER BY {} {} LIMIT $1 OFFSET $2",
                base_where, order_by, order
            );
            sqlx::query_as::<_, super::models::EpisodeDbModel>(&data_sql)
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?
        };

        Ok((rows, total.0))
    }

    /// Get episodes by air date range (for calendar)
    pub async fn get_by_air_date_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
        include_unmonitored: bool,
        include_specials: bool,
    ) -> Result<Vec<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let mut where_clauses = vec!["air_date_utc IS NOT NULL".to_string()];

        if !include_unmonitored {
            where_clauses.push("monitored = true".to_string());
        }

        if !include_specials {
            where_clauses.push("season_number > 0".to_string());
        }

        let query = format!(
            "SELECT * FROM episodes WHERE {} AND air_date_utc >= $1 AND air_date_utc <= $2 ORDER BY air_date_utc ASC",
            where_clauses.join(" AND ")
        );

        let rows = sqlx::query_as::<_, super::models::EpisodeDbModel>(&query)
            .bind(start)
            .bind(end)
            .fetch_all(pool)
            .await?;

        Ok(rows)
    }

    /// Get episodes with files below quality cutoff
    ///
    /// Joins episodes → episode_files → series → quality_profiles, then filters
    /// in Rust where the file's quality weight is below the profile's cutoff weight.
    /// Rust-side filtering is necessary because quality profiles store items as nested
    /// JSON that SQL cannot easily walk.
    /// `series_types` optionally filters by series.series_type (0=standard, 1=daily, 2=anime).
    pub async fn get_cutoff_unmet(
        &self,
        page: i32,
        page_size: i32,
        sort_key: &str,
        sort_direction: &str,
        series_types: Option<&[i32]>,
        exclude_episode_ids: &[i64],
    ) -> Result<(Vec<super::models::EpisodeDbModel>, i64)> {
        use crate::core::profiles::qualities::QualityModel;
        use std::collections::HashMap;

        let pool = self.db.pool();

        // Step 1: Load all quality profiles into a cutoff map (profile_id → cutoff weight)
        let profiles = sqlx::query_as::<_, super::models::QualityProfileDbModel>(
            "SELECT * FROM quality_profiles",
        )
        .fetch_all(pool)
        .await?;

        let cutoff_map: HashMap<i64, i32> =
            profiles.into_iter().map(|p| (p.id, p.cutoff)).collect();

        // Step 2: Load all episode files into a quality map (file_id → quality weight)
        let episode_files =
            sqlx::query_as::<_, super::models::EpisodeFileDbModel>("SELECT * FROM episode_files")
                .fetch_all(pool)
                .await?;

        let file_quality_map: HashMap<i64, i32> = episode_files
            .into_iter()
            .map(|ef| {
                let weight = serde_json::from_str::<QualityModel>(&ef.quality)
                    .map(|qm| qm.quality.weight())
                    .unwrap_or(0);
                (ef.id, weight)
            })
            .collect();

        // Step 3: Load series → (quality_profile_id, series_type) mapping
        let series_rows = sqlx::query_as::<_, super::models::SeriesDbModel>("SELECT * FROM series")
            .fetch_all(pool)
            .await?;

        let series_profile_map: HashMap<i64, i64> = series_rows
            .iter()
            .map(|s| (s.id, s.quality_profile_id))
            .collect();

        // Build series_type set for filtering (if requested)
        let series_type_map: HashMap<i64, i32> =
            series_rows.iter().map(|s| (s.id, s.series_type)).collect();

        // Step 4: Fetch all monitored episodes with files
        let episodes = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE monitored = true AND has_file = true AND episode_file_id IS NOT NULL"
        )
        .fetch_all(pool)
        .await?;

        // Step 5: Filter — keep episodes where file quality < profile cutoff
        //         and optionally filter by series type and exclude specific episode IDs
        let mut cutoff_unmet: Vec<super::models::EpisodeDbModel> = episodes
            .into_iter()
            .filter(|ep| {
                // Exclude specific episode IDs (e.g., actively downloading)
                if !exclude_episode_ids.is_empty() && exclude_episode_ids.contains(&ep.id) {
                    return false;
                }
                // Series type filter
                if let Some(types) = series_types {
                    let st = series_type_map.get(&ep.series_id).copied().unwrap_or(0);
                    if !types.contains(&st) {
                        return false;
                    }
                }
                let file_id = match ep.episode_file_id {
                    Some(id) => id,
                    None => return false,
                };
                let file_weight = file_quality_map.get(&file_id).copied().unwrap_or(0);
                let profile_id = series_profile_map.get(&ep.series_id).copied().unwrap_or(0);
                let cutoff = cutoff_map.get(&profile_id).copied().unwrap_or(0);
                file_weight < cutoff
            })
            .collect();

        let total = cutoff_unmet.len() as i64;

        // Sort
        let desc = sort_direction.eq_ignore_ascii_case("desc")
            || sort_direction.eq_ignore_ascii_case("descending");
        match sort_key {
            "airDateUtc" | "airDate" => cutoff_unmet.sort_by(|a, b| {
                let cmp = a.air_date_utc.cmp(&b.air_date_utc);
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
            "seriesTitle" | "seriesId" => cutoff_unmet.sort_by(|a, b| {
                let cmp = a.series_id.cmp(&b.series_id);
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
            "episodeTitle" | "title" => cutoff_unmet.sort_by(|a, b| {
                let cmp = a.title.to_lowercase().cmp(&b.title.to_lowercase());
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
            "episodeNumber" => cutoff_unmet.sort_by(|a, b| {
                let cmp =
                    (a.season_number, a.episode_number).cmp(&(b.season_number, b.episode_number));
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
            _ => cutoff_unmet.sort_by(|a, b| {
                let cmp = a.air_date_utc.cmp(&b.air_date_utc);
                if desc {
                    cmp.reverse()
                } else {
                    cmp
                }
            }),
        }

        // Paginate
        let offset = ((page.max(1) - 1) * page_size) as usize;
        let paginated: Vec<super::models::EpisodeDbModel> = cutoff_unmet
            .into_iter()
            .skip(offset)
            .take(page_size as usize)
            .collect();

        Ok((paginated, total))
    }
}

/// Repository for quality profiles
pub struct QualityProfileRepository {
    db: Database,
}

impl QualityProfileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::QualityProfileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::QualityProfileDbModel>(
            "SELECT * FROM quality_profiles ORDER BY name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::QualityProfileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::QualityProfileDbModel>(
            "SELECT * FROM quality_profiles WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(
        &self,
        name: &str,
        upgrade_allowed: bool,
        cutoff: i32,
        items: &str,
    ) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO quality_profiles (name, upgrade_allowed, cutoff, items, min_format_score, cutoff_format_score, format_items)
            VALUES ($1, $2, $3, $4, 0, 0, '[]')
            RETURNING id
            "#
        )
        .bind(name)
        .bind(upgrade_allowed)
        .bind(cutoff)
        .bind(items)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(
        &self,
        id: i64,
        name: &str,
        upgrade_allowed: bool,
        cutoff: i32,
        items: &str,
    ) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            "UPDATE quality_profiles SET name = $1, upgrade_allowed = $2, cutoff = $3, items = $4 WHERE id = $5"
        )
        .bind(name)
        .bind(upgrade_allowed)
        .bind(cutoff)
        .bind(items)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM quality_profiles WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for commands
pub struct CommandRepository {
    db: Database,
}

/// Command database model
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommandDbModel {
    pub id: i64,
    pub name: String,
    pub command_name: String,
    pub message: Option<String>,
    pub body: Option<crate::core::datastore::models::JsonString>,
    pub priority: String,
    pub status: String,
    pub result: Option<String>,
    pub queued: chrono::DateTime<chrono::Utc>,
    pub started: Option<chrono::DateTime<chrono::Utc>>,
    pub ended: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<String>,
    pub exception: Option<String>,
    pub trigger: String,
    pub state_change_time: chrono::DateTime<chrono::Utc>,
    pub send_updates_to_client: bool,
    pub update_scheduled_task: bool,
    pub last_execution_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl CommandRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<CommandDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, CommandDbModel>(
            "SELECT * FROM commands ORDER BY queued DESC LIMIT 100",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<CommandDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, CommandDbModel>("SELECT * FROM commands WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    pub async fn insert(
        &self,
        name: &str,
        command_name: &str,
        body: Option<&str>,
        trigger: &str,
    ) -> Result<i64> {
        let now = chrono::Utc::now();
        // Set send_updates_to_client=true for manual commands to enable UI updates
        let send_updates = trigger == "manual";

        // Parse body as JSON for JSONB column (falls back to JSON null)
        let body_json: serde_json::Value = body
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO commands (name, command_name, body, priority, status, queued, trigger, state_change_time, send_updates_to_client)
            VALUES ($1, $2, $3, 'normal', 'queued', $4, $5, $6, $7)
            RETURNING id
            "#
        )
        .bind(name)
        .bind(command_name)
        .bind(&body_json)
        .bind(now)
        .bind(trigger)
        .bind(now)
        .bind(send_updates)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update_status(&self, id: i64, status: &str, result: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now();
        let ended = if status == "completed" || status == "failed" {
            Some(now)
        } else {
            None
        };

        let pool = self.db.pool();
        sqlx::query(
            "UPDATE commands SET status = $1, result = $2, state_change_time = $3, ended = $4 WHERE id = $5"
        )
        .bind(status)
        .bind(result)
        .bind(now)
        .bind(ended)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn start_command(&self, id: i64) -> Result<()> {
        let now = chrono::Utc::now();

        let pool = self.db.pool();
        sqlx::query(
            "UPDATE commands SET status = 'started', started = $1, state_change_time = $2 WHERE id = $3"
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM commands WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Clean up stale commands from previous server sessions
    /// Marks any commands stuck in 'queued' or 'started' status as 'failed'
    pub async fn cleanup_stale(&self) -> Result<u64> {
        let now = chrono::Utc::now();

        let pool = self.db.pool();
        let result = sqlx::query(
            r#"
            UPDATE commands
            SET status = 'failed',
                result = 'failed',
                message = 'Command interrupted by server restart',
                ended = $1,
                state_change_time = $2
            WHERE status IN ('queued', 'started')
            "#,
        )
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

/// Repository for logs
pub struct LogRepository {
    db: Database,
}

impl LogRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Get paginated logs with optional level filtering
    pub async fn get_paged(
        &self,
        page: i32,
        page_size: i32,
        level: Option<&str>,
        sort_key: &str,
        sort_direction: &str,
    ) -> Result<(Vec<super::models::LogDbModel>, i64)> {
        let offset = (page - 1) * page_size;
        let order = if sort_direction.to_lowercase() == "ascending" {
            "ASC"
        } else {
            "DESC"
        };

        // Map sort key to column name
        let sort_column = match sort_key {
            "time" => "time",
            "level" => "level",
            "logger" => "logger",
            _ => "time",
        };

        let pool = self.db.pool();
        let (rows, total) = if let Some(level_filter) = level {
            let level_filter = level_filter.to_lowercase();
            let query = format!(
                "SELECT * FROM logs WHERE LOWER(level) = $1 ORDER BY {} {} LIMIT $2 OFFSET $3",
                sort_column, order
            );
            let rows = sqlx::query_as::<_, super::models::LogDbModel>(&query)
                .bind(&level_filter)
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?;

            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM logs WHERE LOWER(level) = $1")
                .bind(&level_filter)
                .fetch_one(pool)
                .await?;

            (rows, count.0)
        } else {
            let query = format!(
                "SELECT * FROM logs ORDER BY {} {} LIMIT $1 OFFSET $2",
                sort_column, order
            );
            let rows = sqlx::query_as::<_, super::models::LogDbModel>(&query)
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?;

            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM logs")
                .fetch_one(pool)
                .await?;

            (rows, count.0)
        };
        Ok((rows, total))
    }

    /// Insert a new log entry
    pub async fn insert(
        &self,
        level: &str,
        logger: &str,
        message: &str,
        exception: Option<&str>,
        exception_type: Option<&str>,
    ) -> Result<i64> {
        let now = chrono::Utc::now();

        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO logs (time, level, logger, message, exception, exception_type)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(now)
        .bind(level)
        .bind(logger)
        .bind(message)
        .bind(exception)
        .bind(exception_type)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    /// Delete logs older than the specified date
    pub async fn delete_older_than(&self, before: chrono::DateTime<chrono::Utc>) -> Result<u64> {
        let pool = self.db.pool();
        let result = sqlx::query("DELETE FROM logs WHERE time < $1")
            .bind(before)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Clear all logs
    pub async fn clear_all(&self) -> Result<u64> {
        let pool = self.db.pool();
        let result = sqlx::query("DELETE FROM logs").execute(pool).await?;
        Ok(result.rows_affected())
    }
}

/// Repository for download clients
pub struct DownloadClientRepository {
    db: Database,
}

impl DownloadClientRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::DownloadClientDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::DownloadClientDbModel>(
            "SELECT * FROM download_clients ORDER BY priority, name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::DownloadClientDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::DownloadClientDbModel>(
            "SELECT * FROM download_clients WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, client: &super::models::DownloadClientDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO download_clients (enable, protocol, priority, name, implementation, config_contract, settings, tags, remove_completed_downloads, remove_failed_downloads)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#
        )
        .bind(client.enable)
        .bind(client.protocol)
        .bind(client.priority)
        .bind(&client.name)
        .bind(&client.implementation)
        .bind(&client.config_contract)
        .bind(&client.settings)
        .bind(&client.tags)
        .bind(client.remove_completed_downloads)
        .bind(client.remove_failed_downloads)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, client: &super::models::DownloadClientDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE download_clients SET
                enable = $1, protocol = $2, priority = $3, name = $4,
                implementation = $5, config_contract = $6, settings = $7, tags = $8,
                remove_completed_downloads = $9, remove_failed_downloads = $10
            WHERE id = $11
            "#,
        )
        .bind(client.enable)
        .bind(client.protocol)
        .bind(client.priority)
        .bind(&client.name)
        .bind(&client.implementation)
        .bind(&client.config_contract)
        .bind(&client.settings)
        .bind(&client.tags)
        .bind(client.remove_completed_downloads)
        .bind(client.remove_failed_downloads)
        .bind(client.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM download_clients WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for notifications
pub struct NotificationRepository {
    db: Database,
}

impl NotificationRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::NotificationDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::NotificationDbModel>(
            "SELECT * FROM notifications ORDER BY name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::NotificationDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::NotificationDbModel>(
            "SELECT * FROM notifications WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Get notifications enabled for a specific event type
    pub async fn get_enabled_for_event(
        &self,
        event_type: &str,
    ) -> Result<Vec<super::models::NotificationDbModel>> {
        // Map event type to column name
        let column = match event_type {
            "grab" => "on_grab",
            "download" => "on_download",
            "upgrade" => "on_upgrade",
            "rename" => "on_rename",
            "series_delete" => "on_series_delete",
            "episode_file_delete" => "on_episode_file_delete",
            "episode_file_delete_for_upgrade" => "on_episode_file_delete_for_upgrade",
            "health_issue" => "on_health_issue",
            "health_restored" => "on_health_restored",
            "manual_interaction_required" => "on_manual_interaction_required",
            "application_update" => "on_application_update",
            _ => return Ok(vec![]),
        };

        let query = format!("SELECT * FROM notifications WHERE {} = true", column);

        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::NotificationDbModel>(&query)
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn insert(&self, notification: &super::models::NotificationDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO notifications (
                name, implementation, config_contract,
                on_grab, on_download, on_upgrade, on_rename, on_series_delete,
                on_episode_file_delete, on_episode_file_delete_for_upgrade,
                on_health_issue, on_health_restored, on_manual_interaction_required,
                on_application_update, include_health_warnings, settings, tags
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            RETURNING id
            "#,
        )
        .bind(&notification.name)
        .bind(&notification.implementation)
        .bind(&notification.config_contract)
        .bind(notification.on_grab)
        .bind(notification.on_download)
        .bind(notification.on_upgrade)
        .bind(notification.on_rename)
        .bind(notification.on_series_delete)
        .bind(notification.on_episode_file_delete)
        .bind(notification.on_episode_file_delete_for_upgrade)
        .bind(notification.on_health_issue)
        .bind(notification.on_health_restored)
        .bind(notification.on_manual_interaction_required)
        .bind(notification.on_application_update)
        .bind(notification.include_health_warnings)
        .bind(&notification.settings)
        .bind(&notification.tags)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, notification: &super::models::NotificationDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE notifications SET
                name = $1, implementation = $2, config_contract = $3,
                on_grab = $4, on_download = $5, on_upgrade = $6, on_rename = $7,
                on_series_delete = $8, on_episode_file_delete = $9,
                on_episode_file_delete_for_upgrade = $10, on_health_issue = $11,
                on_health_restored = $12, on_manual_interaction_required = $13,
                on_application_update = $14, include_health_warnings = $15,
                settings = $16, tags = $17
            WHERE id = $18
            "#,
        )
        .bind(&notification.name)
        .bind(&notification.implementation)
        .bind(&notification.config_contract)
        .bind(notification.on_grab)
        .bind(notification.on_download)
        .bind(notification.on_upgrade)
        .bind(notification.on_rename)
        .bind(notification.on_series_delete)
        .bind(notification.on_episode_file_delete)
        .bind(notification.on_episode_file_delete_for_upgrade)
        .bind(notification.on_health_issue)
        .bind(notification.on_health_restored)
        .bind(notification.on_manual_interaction_required)
        .bind(notification.on_application_update)
        .bind(notification.include_health_warnings)
        .bind(&notification.settings)
        .bind(&notification.tags)
        .bind(notification.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM notifications WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

// TrackedDownloadRepository removed — tracked downloads now use
// per-content-type JSONL flat files (see core/queue/tracked.rs).

/// Repository for indexers
pub struct IndexerRepository {
    db: Database,
}

impl IndexerRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::IndexerDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::IndexerDbModel>(
            "SELECT * FROM indexers ORDER BY priority, name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::IndexerDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::IndexerDbModel>(
            "SELECT * FROM indexers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, indexer: &super::models::IndexerDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO indexers (name, implementation, config_contract, enable_rss, enable_automatic_search,
                                  enable_interactive_search, protocol, priority, download_client_id, settings, tags)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#
        )
        .bind(&indexer.name)
        .bind(&indexer.implementation)
        .bind(&indexer.config_contract)
        .bind(indexer.enable_rss)
        .bind(indexer.enable_automatic_search)
        .bind(indexer.enable_interactive_search)
        .bind(indexer.protocol)
        .bind(indexer.priority)
        .bind(indexer.download_client_id)
        .bind(&indexer.settings)
        .bind(&indexer.tags)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, indexer: &super::models::IndexerDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE indexers SET
                name = $1, implementation = $2, config_contract = $3, enable_rss = $4,
                enable_automatic_search = $5, enable_interactive_search = $6, protocol = $7,
                priority = $8, download_client_id = $9, settings = $10, tags = $11
            WHERE id = $12
            "#,
        )
        .bind(&indexer.name)
        .bind(&indexer.implementation)
        .bind(&indexer.config_contract)
        .bind(indexer.enable_rss)
        .bind(indexer.enable_automatic_search)
        .bind(indexer.enable_interactive_search)
        .bind(indexer.protocol)
        .bind(indexer.priority)
        .bind(indexer.download_client_id)
        .bind(&indexer.settings)
        .bind(&indexer.tags)
        .bind(indexer.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM indexers WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for blocklist entries
pub struct BlocklistRepository {
    db: Database,
}

impl BlocklistRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::BlocklistDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::BlocklistDbModel>(
            "SELECT * FROM blocklist ORDER BY date DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_paged(
        &self,
        page: i32,
        page_size: i32,
        sort_key: &str,
        sort_direction: &str,
    ) -> Result<(Vec<super::models::BlocklistDbModel>, i64)> {
        let offset = (page - 1) * page_size;
        let order = if sort_direction.to_lowercase() == "ascending" {
            "ASC"
        } else {
            "DESC"
        };
        let sort_column = match sort_key {
            "date" => "date",
            "sourceTitle" => "source_title",
            "indexer" => "indexer",
            _ => "date",
        };

        let pool = self.db.pool();
        let query = format!(
            "SELECT * FROM blocklist ORDER BY {} {} LIMIT $1 OFFSET $2",
            sort_column, order
        );
        let rows = sqlx::query_as::<_, super::models::BlocklistDbModel>(&query)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blocklist")
            .fetch_one(pool)
            .await?;

        Ok((rows, count.0))
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::BlocklistDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::BlocklistDbModel>(
            "SELECT * FROM blocklist WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn count(&self) -> Result<i64> {
        let pool = self.db.pool();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blocklist")
            .fetch_one(pool)
            .await?;
        Ok(count.0)
    }

    pub async fn insert(&self, item: &super::models::BlocklistDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO blocklist (
                series_id, episode_ids, source_title, quality, languages,
                custom_formats, custom_format_score, protocol, indexer, message, date
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#,
        )
        .bind(item.series_id)
        .bind(&item.episode_ids)
        .bind(&item.source_title)
        .bind(&item.quality)
        .bind(&item.languages)
        .bind(&item.custom_formats)
        .bind(item.custom_format_score)
        .bind(item.protocol)
        .bind(&item.indexer)
        .bind(&item.message)
        .bind(item.date)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM blocklist WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_bulk(&self, ids: &[i64]) -> Result<u64> {
        let pool = self.db.pool();
        let mut total = 0u64;
        for id in ids {
            let result = sqlx::query("DELETE FROM blocklist WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            total += result.rows_affected();
        }
        Ok(total)
    }
}

/// Repository for custom formats
pub struct CustomFormatRepository {
    db: Database,
}

impl CustomFormatRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::CustomFormatDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::CustomFormatDbModel>(
            "SELECT * FROM custom_formats ORDER BY name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::CustomFormatDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::CustomFormatDbModel>(
            "SELECT * FROM custom_formats WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, item: &super::models::CustomFormatDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO custom_formats (name, include_custom_format_when_renaming, specifications)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(&item.name)
        .bind(item.include_custom_format_when_renaming)
        .bind(&item.specifications)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, item: &super::models::CustomFormatDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE custom_formats SET
                name = $1, include_custom_format_when_renaming = $2, specifications = $3
            WHERE id = $4
            "#,
        )
        .bind(&item.name)
        .bind(item.include_custom_format_when_renaming)
        .bind(&item.specifications)
        .bind(item.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM custom_formats WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for custom filters
pub struct CustomFilterRepository {
    db: Database,
}

impl CustomFilterRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::CustomFilterDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::CustomFilterDbModel>(
            "SELECT * FROM custom_filters ORDER BY label",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::CustomFilterDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::CustomFilterDbModel>(
            "SELECT * FROM custom_filters WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, item: &super::models::CustomFilterDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO custom_filters (filter_type, label, filters)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(&item.filter_type)
        .bind(&item.label)
        .bind(&item.filters)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, item: &super::models::CustomFilterDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE custom_filters SET
                filter_type = $1, label = $2, filters = $3
            WHERE id = $4
            "#,
        )
        .bind(&item.filter_type)
        .bind(&item.label)
        .bind(&item.filters)
        .bind(item.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM custom_filters WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for remote path mappings
pub struct RemotePathMappingRepository {
    db: Database,
}

impl RemotePathMappingRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::RemotePathMappingDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::RemotePathMappingDbModel>(
            "SELECT * FROM remote_path_mappings ORDER BY host",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(
        &self,
        id: i64,
    ) -> Result<Option<super::models::RemotePathMappingDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::RemotePathMappingDbModel>(
            "SELECT * FROM remote_path_mappings WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, item: &super::models::RemotePathMappingDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO remote_path_mappings (host, remote_path, local_path)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(&item.host)
        .bind(&item.remote_path)
        .bind(&item.local_path)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, item: &super::models::RemotePathMappingDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE remote_path_mappings SET
                host = $1, remote_path = $2, local_path = $3
            WHERE id = $4
            "#,
        )
        .bind(&item.host)
        .bind(&item.remote_path)
        .bind(&item.local_path)
        .bind(item.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM remote_path_mappings WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for delay profiles
pub struct DelayProfileRepository {
    db: Database,
}

impl DelayProfileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::DelayProfileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::DelayProfileDbModel>(
            "SELECT * FROM delay_profiles ORDER BY id",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::DelayProfileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::DelayProfileDbModel>(
            "SELECT * FROM delay_profiles WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, item: &super::models::DelayProfileDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO delay_profiles (
                enable_usenet, enable_torrent, preferred_protocol,
                usenet_delay, torrent_delay, bypass_if_highest_quality,
                bypass_if_above_custom_format_score, tags
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(item.enable_usenet)
        .bind(item.enable_torrent)
        .bind(item.preferred_protocol)
        .bind(item.usenet_delay)
        .bind(item.torrent_delay)
        .bind(item.bypass_if_highest_quality)
        .bind(item.bypass_if_above_custom_format_score)
        .bind(&item.tags)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, item: &super::models::DelayProfileDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE delay_profiles SET
                enable_usenet = $1, enable_torrent = $2, preferred_protocol = $3,
                usenet_delay = $4, torrent_delay = $5, bypass_if_highest_quality = $6,
                bypass_if_above_custom_format_score = $7, tags = $8
            WHERE id = $9
            "#,
        )
        .bind(item.enable_usenet)
        .bind(item.enable_torrent)
        .bind(item.preferred_protocol)
        .bind(item.usenet_delay)
        .bind(item.torrent_delay)
        .bind(item.bypass_if_highest_quality)
        .bind(item.bypass_if_above_custom_format_score)
        .bind(&item.tags)
        .bind(item.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM delay_profiles WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for language profiles
pub struct LanguageProfileRepository {
    db: Database,
}

impl LanguageProfileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::LanguageProfileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::LanguageProfileDbModel>(
            "SELECT * FROM language_profiles ORDER BY name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(
        &self,
        id: i64,
    ) -> Result<Option<super::models::LanguageProfileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::LanguageProfileDbModel>(
            "SELECT * FROM language_profiles WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, item: &super::models::LanguageProfileDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO language_profiles (name, upgrade_allowed, cutoff, languages)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(&item.name)
        .bind(item.upgrade_allowed)
        .bind(item.cutoff)
        .bind(&item.languages)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, item: &super::models::LanguageProfileDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE language_profiles SET
                name = $1, upgrade_allowed = $2, cutoff = $3, languages = $4
            WHERE id = $5
            "#,
        )
        .bind(&item.name)
        .bind(item.upgrade_allowed)
        .bind(item.cutoff)
        .bind(&item.languages)
        .bind(item.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM language_profiles WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for import exclusions (global exclusion list)
pub struct ImportExclusionRepository {
    db: Database,
}

impl ImportExclusionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::ImportExclusionDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::ImportExclusionDbModel>(
            "SELECT * FROM import_exclusions ORDER BY title",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(
        &self,
        id: i64,
    ) -> Result<Option<super::models::ImportExclusionDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::ImportExclusionDbModel>(
            "SELECT * FROM import_exclusions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn add(
        &self,
        tmdb_id: Option<i32>,
        imdb_id: Option<&str>,
        tvdb_id: Option<i32>,
        title: &str,
        year: Option<i32>,
        content_type: &str,
    ) -> Result<i64> {
        let pool = self.db.pool();
        let tmdb_id_i64 = tmdb_id.map(|v| v as i64);
        let tvdb_id_i64 = tvdb_id.map(|v| v as i64);
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO import_exclusions (tmdb_id, imdb_id, tvdb_id, title, year, content_type, added)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            RETURNING id
            "#,
        )
        .bind(tmdb_id_i64)
        .bind(imdb_id)
        .bind(tvdb_id_i64)
        .bind(title)
        .bind(year)
        .bind(content_type)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM import_exclusions WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn is_excluded_movie(
        &self,
        tmdb_id: Option<i64>,
        imdb_id: Option<&str>,
    ) -> Result<bool> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM import_exclusions
            WHERE content_type = 'movie'
              AND (
                (tmdb_id IS NOT NULL AND tmdb_id = $1)
                OR (imdb_id IS NOT NULL AND imdb_id = $2)
              )
            "#,
        )
        .bind(tmdb_id)
        .bind(imdb_id)
        .fetch_one(pool)
        .await?;
        Ok(row.0 > 0)
    }

    pub async fn is_excluded_series(
        &self,
        tvdb_id: Option<i64>,
        imdb_id: Option<&str>,
    ) -> Result<bool> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM import_exclusions
            WHERE content_type = 'series'
              AND (
                (tvdb_id IS NOT NULL AND tvdb_id = $1)
                OR (imdb_id IS NOT NULL AND imdb_id = $2)
              )
            "#,
        )
        .bind(tvdb_id)
        .bind(imdb_id)
        .fetch_one(pool)
        .await?;
        Ok(row.0 > 0)
    }
}

/// Repository for import lists
pub struct ImportListRepository {
    db: Database,
}

impl ImportListRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::ImportListDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::ImportListDbModel>(
            "SELECT * FROM import_lists ORDER BY name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::ImportListDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::ImportListDbModel>(
            "SELECT * FROM import_lists WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, item: &super::models::ImportListDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO import_lists (
                name, enabled, list_type, list_url, root_folder_path,
                quality_profile_id, monitored, search_on_add, content_type,
                sync_interval_hours, last_synced_at, tags
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id
            "#,
        )
        .bind(&item.name)
        .bind(item.enabled)
        .bind(&item.list_type)
        .bind(&item.list_url)
        .bind(&item.root_folder_path)
        .bind(item.quality_profile_id)
        .bind(item.monitored)
        .bind(item.search_on_add)
        .bind(&item.content_type)
        .bind(item.sync_interval_hours)
        .bind(item.last_synced_at)
        .bind(&item.tags)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, item: &super::models::ImportListDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE import_lists SET
                name = $1, enabled = $2, list_type = $3, list_url = $4,
                root_folder_path = $5, quality_profile_id = $6, monitored = $7,
                search_on_add = $8, content_type = $9, sync_interval_hours = $10,
                last_synced_at = $11, tags = $12
            WHERE id = $13
            "#,
        )
        .bind(&item.name)
        .bind(item.enabled)
        .bind(&item.list_type)
        .bind(&item.list_url)
        .bind(&item.root_folder_path)
        .bind(item.quality_profile_id)
        .bind(item.monitored)
        .bind(item.search_on_add)
        .bind(&item.content_type)
        .bind(item.sync_interval_hours)
        .bind(item.last_synced_at)
        .bind(&item.tags)
        .bind(item.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM import_lists WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn get_enabled(&self) -> Result<Vec<super::models::ImportListDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::ImportListDbModel>(
            "SELECT * FROM import_lists WHERE enabled = true ORDER BY name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_last_synced(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("UPDATE import_lists SET last_synced_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for per-import-list exclusions (tracks already-processed items)
pub struct ImportListExclusionRepository {
    db: Database,
}

impl ImportListExclusionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn is_excluded(&self, list_id: i64, external_id: &str) -> Result<bool> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM import_list_exclusions
            WHERE list_id = $1 AND external_id = $2
            "#,
        )
        .bind(list_id)
        .bind(external_id)
        .fetch_one(pool)
        .await?;
        Ok(row.0 > 0)
    }

    pub async fn add(
        &self,
        list_id: i64,
        external_id: &str,
        title: &str,
        content_type: &str,
    ) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            INSERT INTO import_list_exclusions (list_id, external_id, title, content_type, added)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (list_id, external_id) DO NOTHING
            "#,
        )
        .bind(list_id)
        .bind(external_id)
        .bind(title)
        .bind(content_type)
        .execute(pool)
        .await?;
        Ok(())
    }
}

// ============================================================================
// Music domain repositories
// ============================================================================

/// Repository for artists
pub struct ArtistRepository {
    db: Database,
}

impl ArtistRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::ArtistDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::ArtistDbModel>(
            "SELECT * FROM artists ORDER BY sort_name",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::ArtistDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::ArtistDbModel>(
            "SELECT * FROM artists WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_musicbrainz_id(
        &self,
        musicbrainz_id: &str,
    ) -> Result<Option<super::models::ArtistDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::ArtistDbModel>(
            "SELECT * FROM artists WHERE musicbrainz_id = $1",
        )
        .bind(musicbrainz_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_path(&self, path: &str) -> Result<Option<super::models::ArtistDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::ArtistDbModel>(
            "SELECT * FROM artists WHERE path = $1",
        )
        .bind(path)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, artist: &super::models::ArtistDbModel) -> Result<i64> {
        let pool = self.db.pool();
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
            ) RETURNING id
            "#,
        )
        .bind(&artist.musicbrainz_id)
        .bind(&artist.name)
        .bind(&artist.clean_name)
        .bind(&artist.sort_name)
        .bind(&artist.overview)
        .bind(&artist.artist_type)
        .bind(&artist.status)
        .bind(&artist.genres)
        .bind(&artist.images)
        .bind(&artist.tags)
        .bind(&artist.path)
        .bind(&artist.root_folder_path)
        .bind(artist.quality_profile_id)
        .bind(artist.monitored)
        .bind(artist.added)
        .bind(artist.last_info_sync)
        .bind(&artist.title_slug)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, artist: &super::models::ArtistDbModel) -> Result<()> {
        let pool = self.db.pool();
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
        .bind(&artist.status)
        .bind(&artist.genres)
        .bind(&artist.images)
        .bind(&artist.tags)
        .bind(&artist.path)
        .bind(&artist.root_folder_path)
        .bind(artist.quality_profile_id)
        .bind(artist.monitored)
        .bind(artist.last_info_sync)
        .bind(&artist.title_slug)
        .bind(artist.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        // Albums and tracks cascade on delete via FK
        sqlx::query("DELETE FROM artists WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for albums
pub struct AlbumRepository {
    db: Database,
}

impl AlbumRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::AlbumDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::AlbumDbModel>(
            "SELECT * FROM albums ORDER BY artist_id, release_date",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::AlbumDbModel>> {
        let pool = self.db.pool();
        let row =
            sqlx::query_as::<_, super::models::AlbumDbModel>("SELECT * FROM albums WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        Ok(row)
    }

    pub async fn get_by_artist_id(
        &self,
        artist_id: i64,
    ) -> Result<Vec<super::models::AlbumDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::AlbumDbModel>(
            "SELECT * FROM albums WHERE artist_id = $1 ORDER BY release_date",
        )
        .bind(artist_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_musicbrainz_id(
        &self,
        musicbrainz_id: &str,
    ) -> Result<Option<super::models::AlbumDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::AlbumDbModel>(
            "SELECT * FROM albums WHERE musicbrainz_id = $1",
        )
        .bind(musicbrainz_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, album: &super::models::AlbumDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO albums (
                artist_id, musicbrainz_id, title, clean_title, album_type,
                secondary_types, release_date, genres, images, monitored, added, last_info_sync,
                title_slug
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            ) RETURNING id
            "#,
        )
        .bind(album.artist_id)
        .bind(&album.musicbrainz_id)
        .bind(&album.title)
        .bind(&album.clean_title)
        .bind(&album.album_type)
        .bind(&album.secondary_types)
        .bind(album.release_date)
        .bind(&album.genres)
        .bind(&album.images)
        .bind(album.monitored)
        .bind(album.added)
        .bind(album.last_info_sync)
        .bind(&album.title_slug)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn get_by_slug(
        &self,
        artist_id: i64,
        slug: &str,
    ) -> Result<Option<super::models::AlbumDbModel>> {
        let pool = self.db.pool();
        let album = sqlx::query_as::<_, super::models::AlbumDbModel>(
            "SELECT * FROM albums WHERE artist_id = $1 AND title_slug = $2 LIMIT 1",
        )
        .bind(artist_id)
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(album)
    }

    pub async fn update(&self, album: &super::models::AlbumDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE albums SET
                artist_id = $1, musicbrainz_id = $2, title = $3, clean_title = $4,
                album_type = $5, secondary_types = $6, release_date = $7, genres = $8,
                images = $9, monitored = $10, last_info_sync = $11, title_slug = $12
            WHERE id = $13
            "#,
        )
        .bind(album.artist_id)
        .bind(&album.musicbrainz_id)
        .bind(&album.title)
        .bind(&album.clean_title)
        .bind(&album.album_type)
        .bind(&album.secondary_types)
        .bind(album.release_date)
        .bind(&album.genres)
        .bind(&album.images)
        .bind(album.monitored)
        .bind(album.last_info_sync)
        .bind(&album.title_slug)
        .bind(album.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        // Tracks cascade on delete via FK
        sqlx::query("DELETE FROM albums WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for tracks
pub struct TrackRepository {
    db: Database,
}

impl TrackRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::TrackDbModel>> {
        let pool = self.db.pool();
        let row =
            sqlx::query_as::<_, super::models::TrackDbModel>("SELECT * FROM tracks WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        Ok(row)
    }

    pub async fn get_by_album_id(&self, album_id: i64) -> Result<Vec<super::models::TrackDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::TrackDbModel>(
            "SELECT * FROM tracks WHERE album_id = $1 ORDER BY disc_number, track_number",
        )
        .bind(album_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_artist_id(
        &self,
        artist_id: i64,
    ) -> Result<Vec<super::models::TrackDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::TrackDbModel>(
            "SELECT * FROM tracks WHERE artist_id = $1 ORDER BY album_id, disc_number, track_number",
        )
        .bind(artist_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn insert(&self, track: &super::models::TrackDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO tracks (
                album_id, artist_id, title, track_number, disc_number,
                duration_ms, has_file, track_file_id, monitored, air_date_utc
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
            ) RETURNING id
            "#,
        )
        .bind(track.album_id)
        .bind(track.artist_id)
        .bind(&track.title)
        .bind(track.track_number)
        .bind(track.disc_number)
        .bind(track.duration_ms)
        .bind(track.has_file)
        .bind(track.track_file_id)
        .bind(track.monitored)
        .bind(track.air_date_utc)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, track: &super::models::TrackDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE tracks SET
                album_id = $1, artist_id = $2, title = $3, track_number = $4,
                disc_number = $5, duration_ms = $6, has_file = $7,
                track_file_id = $8, monitored = $9, air_date_utc = $10
            WHERE id = $11
            "#,
        )
        .bind(track.album_id)
        .bind(track.artist_id)
        .bind(&track.title)
        .bind(track.track_number)
        .bind(track.disc_number)
        .bind(track.duration_ms)
        .bind(track.has_file)
        .bind(track.track_file_id)
        .bind(track.monitored)
        .bind(track.air_date_utc)
        .bind(track.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM tracks WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for track files
pub struct TrackFileRepository {
    db: Database,
}

impl TrackFileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::TrackFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::TrackFileDbModel>(
            "SELECT * FROM track_files WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_artist_id(
        &self,
        artist_id: i64,
    ) -> Result<Vec<super::models::TrackFileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::TrackFileDbModel>(
            "SELECT * FROM track_files WHERE artist_id = $1 ORDER BY album_id, path",
        )
        .bind(artist_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_album_id(
        &self,
        album_id: i64,
    ) -> Result<Vec<super::models::TrackFileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::TrackFileDbModel>(
            "SELECT * FROM track_files WHERE album_id = $1 ORDER BY path",
        )
        .bind(album_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn insert(&self, file: &super::models::TrackFileDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO track_files (
                artist_id, album_id, relative_path, path, size,
                quality, media_info, date_added
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8
            ) RETURNING id
            "#,
        )
        .bind(file.artist_id)
        .bind(file.album_id)
        .bind(&file.relative_path)
        .bind(&file.path)
        .bind(file.size)
        .bind(&file.quality)
        .bind(&file.media_info)
        .bind(file.date_added)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM track_files WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for podcasts
pub struct PodcastRepository {
    db: Database,
}

impl PodcastRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::PodcastDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::PodcastDbModel>(
            "SELECT * FROM podcasts ORDER BY sort_title",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::PodcastDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::PodcastDbModel>(
            "SELECT * FROM podcasts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_feed_url(
        &self,
        feed_url: &str,
    ) -> Result<Option<super::models::PodcastDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::PodcastDbModel>(
            "SELECT * FROM podcasts WHERE feed_url = $1",
        )
        .bind(feed_url)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, podcast: &super::models::PodcastDbModel) -> Result<i64> {
        let pool = self.db.pool();
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
            ) RETURNING id
            "#,
        )
        .bind(&podcast.title)
        .bind(&podcast.clean_title)
        .bind(&podcast.sort_title)
        .bind(&podcast.overview)
        .bind(&podcast.author)
        .bind(&podcast.feed_url)
        .bind(&podcast.website_url)
        .bind(&podcast.genres)
        .bind(&podcast.images)
        .bind(&podcast.tags)
        .bind(&podcast.path)
        .bind(&podcast.root_folder_path)
        .bind(podcast.quality_profile_id)
        .bind(podcast.monitored)
        .bind(podcast.added)
        .bind(podcast.last_info_sync)
        .bind(&podcast.title_slug)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, podcast: &super::models::PodcastDbModel) -> Result<()> {
        let pool = self.db.pool();
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
        .bind(&podcast.genres)
        .bind(&podcast.images)
        .bind(&podcast.tags)
        .bind(&podcast.path)
        .bind(&podcast.root_folder_path)
        .bind(podcast.quality_profile_id)
        .bind(podcast.monitored)
        .bind(podcast.last_info_sync)
        .bind(&podcast.title_slug)
        .bind(podcast.id)
        .execute(pool)
        .await?;
        Ok(())
    }

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
}

/// Repository for podcast episodes
pub struct PodcastEpisodeRepository {
    db: Database,
}

impl PodcastEpisodeRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_by_podcast_id(
        &self,
        podcast_id: i64,
    ) -> Result<Vec<super::models::PodcastEpisodeDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::PodcastEpisodeDbModel>(
            "SELECT * FROM podcast_episodes WHERE podcast_id = $1 ORDER BY season_number, COALESCE(episode_number, 0)",
        )
        .bind(podcast_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::PodcastEpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::PodcastEpisodeDbModel>(
            "SELECT * FROM podcast_episodes WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_guid(
        &self,
        podcast_id: i64,
        guid: &str,
    ) -> Result<Option<super::models::PodcastEpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::PodcastEpisodeDbModel>(
            "SELECT * FROM podcast_episodes WHERE podcast_id = $1 AND guid = $2",
        )
        .bind(podcast_id)
        .bind(guid)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, episode: &super::models::PodcastEpisodeDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO podcast_episodes (
                podcast_id, title, overview, episode_number, season_number,
                air_date_utc, duration_ms, download_url, file_size,
                has_file, podcast_file_id, monitored, guid
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            ) RETURNING id
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
        Ok(row.0)
    }

    pub async fn update(&self, episode: &super::models::PodcastEpisodeDbModel) -> Result<()> {
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
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM podcast_episodes WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for podcast files
pub struct PodcastFileRepository {
    db: Database,
}

impl PodcastFileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::PodcastFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::PodcastFileDbModel>(
            "SELECT * FROM podcast_files WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_podcast_id(
        &self,
        podcast_id: i64,
    ) -> Result<Vec<super::models::PodcastFileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::PodcastFileDbModel>(
            "SELECT * FROM podcast_files WHERE podcast_id = $1 ORDER BY date_added DESC",
        )
        .bind(podcast_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn insert(&self, file: &super::models::PodcastFileDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO podcast_files (
                podcast_id, relative_path, path, size, quality,
                media_info, date_added
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7
            ) RETURNING id
            "#,
        )
        .bind(file.podcast_id)
        .bind(&file.relative_path)
        .bind(&file.path)
        .bind(file.size)
        .bind(&file.quality)
        .bind(&file.media_info)
        .bind(file.date_added)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM podcast_files WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for audiobooks
pub struct AudiobookRepository {
    db: Database,
}

impl AudiobookRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_all(&self) -> Result<Vec<super::models::AudiobookDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::AudiobookDbModel>(
            "SELECT * FROM audiobooks ORDER BY sort_title",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::AudiobookDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::AudiobookDbModel>(
            "SELECT * FROM audiobooks WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_isbn(&self, isbn: &str) -> Result<Option<super::models::AudiobookDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::AudiobookDbModel>(
            "SELECT * FROM audiobooks WHERE isbn = $1",
        )
        .bind(isbn)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_asin(&self, asin: &str) -> Result<Option<super::models::AudiobookDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::AudiobookDbModel>(
            "SELECT * FROM audiobooks WHERE asin = $1",
        )
        .bind(asin)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, audiobook: &super::models::AudiobookDbModel) -> Result<i64> {
        let pool = self.db.pool();
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
            ) RETURNING id
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
        .bind(&audiobook.genres)
        .bind(&audiobook.images)
        .bind(&audiobook.tags)
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
        Ok(row.0)
    }

    pub async fn update(&self, audiobook: &super::models::AudiobookDbModel) -> Result<()> {
        let pool = self.db.pool();
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
        .bind(&audiobook.genres)
        .bind(&audiobook.images)
        .bind(&audiobook.tags)
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
        Ok(())
    }

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
}

/// Repository for audiobook chapters
pub struct AudiobookChapterRepository {
    db: Database,
}

impl AudiobookChapterRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_by_audiobook_id(
        &self,
        audiobook_id: i64,
    ) -> Result<Vec<super::models::AudiobookChapterDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::AudiobookChapterDbModel>(
            "SELECT * FROM audiobook_chapters WHERE audiobook_id = $1 ORDER BY chapter_number",
        )
        .bind(audiobook_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(
        &self,
        id: i64,
    ) -> Result<Option<super::models::AudiobookChapterDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::AudiobookChapterDbModel>(
            "SELECT * FROM audiobook_chapters WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, chapter: &super::models::AudiobookChapterDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO audiobook_chapters (
                audiobook_id, title, chapter_number, duration_ms,
                has_file, monitored
            ) VALUES (
                $1, $2, $3, $4, $5, $6
            ) RETURNING id
            "#,
        )
        .bind(chapter.audiobook_id)
        .bind(&chapter.title)
        .bind(chapter.chapter_number)
        .bind(chapter.duration_ms)
        .bind(chapter.has_file)
        .bind(chapter.monitored)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn update(&self, chapter: &super::models::AudiobookChapterDbModel) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE audiobook_chapters SET
                title = $1, chapter_number = $2, duration_ms = $3,
                has_file = $4, monitored = $5
            WHERE id = $6
            "#,
        )
        .bind(&chapter.title)
        .bind(chapter.chapter_number)
        .bind(chapter.duration_ms)
        .bind(chapter.has_file)
        .bind(chapter.monitored)
        .bind(chapter.id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM audiobook_chapters WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// Repository for audiobook files
pub struct AudiobookFileRepository {
    db: Database,
}

impl AudiobookFileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::AudiobookFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::AudiobookFileDbModel>(
            "SELECT * FROM audiobook_files WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_audiobook_id(
        &self,
        audiobook_id: i64,
    ) -> Result<Vec<super::models::AudiobookFileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::AudiobookFileDbModel>(
            "SELECT * FROM audiobook_files WHERE audiobook_id = $1 ORDER BY date_added DESC",
        )
        .bind(audiobook_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn insert(&self, file: &super::models::AudiobookFileDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO audiobook_files (
                audiobook_id, relative_path, path, size, quality,
                media_info, date_added
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7
            ) RETURNING id
            "#,
        )
        .bind(file.audiobook_id)
        .bind(&file.relative_path)
        .bind(&file.path)
        .bind(file.size)
        .bind(&file.quality)
        .bind(&file.media_info)
        .bind(file.date_added)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM audiobook_files WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
