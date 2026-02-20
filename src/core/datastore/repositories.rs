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

/// Unit of work for transactions
pub struct UnitOfWork {
    db: Database,
}

impl UnitOfWork {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn commit(&self) -> Result<()> {
        // Commit transaction
        Ok(())
    }

    pub async fn rollback(&self) -> Result<()> {
        // Rollback transaction
        Ok(())
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
            "SELECT * FROM episode_files ORDER BY date_added DESC"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::EpisodeFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeFileDbModel>(
            "SELECT * FROM episode_files WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_series_id(&self, series_id: i64) -> Result<Vec<super::models::EpisodeFileDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::EpisodeFileDbModel>(
            "SELECT * FROM episode_files WHERE series_id = $1 ORDER BY season_number, relative_path"
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_path(&self, path: &str) -> Result<Option<super::models::EpisodeFileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeFileDbModel>(
            "SELECT * FROM episode_files WHERE path = $1"
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
                media_info, original_file_path
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id
            "#
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
                media_info = $11, original_file_path = $12
            WHERE id = $13
            "#
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
        let order = if sort_direction.to_lowercase() == "ascending" { "ASC" } else { "DESC" };
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
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM history WHERE event_type = $1")
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

    pub async fn get_since(&self, date: chrono::DateTime<chrono::Utc>) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            "SELECT * FROM history WHERE date >= $1 ORDER BY date DESC"
        )
        .bind(date)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_series_id(&self, series_id: i64) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            "SELECT * FROM history WHERE series_id = $1 ORDER BY date DESC"
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_episode_id(&self, episode_id: i64) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            "SELECT * FROM history WHERE episode_id = $1 ORDER BY date DESC"
        )
        .bind(episode_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_for_series(&self, series_id: i64, limit: i32) -> Result<Vec<super::models::HistoryDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::HistoryDbModel>(
            "SELECT * FROM history WHERE series_id = $1 ORDER BY date DESC LIMIT $2"
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
                series_id, episode_id, source_title, quality, languages,
                custom_formats, custom_format_score, quality_cutoff_not_met,
                date, download_id, event_type, data
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id
            "#
        )
        .bind(history.series_id)
        .bind(history.episode_id)
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
            "SELECT * FROM root_folders"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::RootFolderDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::RootFolderDbModel>(
            "SELECT * FROM root_folders WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, path: &str) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as("INSERT INTO root_folders (path, accessible) VALUES ($1, $2) RETURNING id")
            .bind(path)
            .bind(true)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
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
            "SELECT * FROM series ORDER BY sort_title"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::SeriesDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::SeriesDbModel>(
            "SELECT * FROM series WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_tvdb_id(&self, tvdb_id: i64) -> Result<Option<super::models::SeriesDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::SeriesDbModel>(
            "SELECT * FROM series WHERE tvdb_id = $1"
        )
        .bind(tvdb_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_imdb_id(&self, imdb_id: &str) -> Result<Option<super::models::SeriesDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::SeriesDbModel>(
            "SELECT * FROM series WHERE imdb_id = $1"
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
                use_scene_numbering, added, last_info_sync,
                imdb_rating, imdb_votes
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9, $10,
                $11, $12, $13, $14,
                $15, $16, $17, $18, $19,
                $20, $21, $22, $23, $24, $25,
                $26, $27, $28,
                $29, $30
            ) RETURNING id
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
        sqlx::query(
            r#"
            UPDATE series SET
                tvdb_id = $1, tv_rage_id = $2, tv_maze_id = $3, imdb_id = $4, tmdb_id = $5,
                title = $6, clean_title = $7, sort_title = $8, status = $9, overview = $10,
                monitored = $11, monitor_new_items = $12, quality_profile_id = $13, language_profile_id = $14,
                season_folder = $15, series_type = $16, title_slug = $17, path = $18, root_folder_path = $19,
                year = $20, first_aired = $21, last_aired = $22, runtime = $23, network = $24, certification = $25,
                use_scene_numbering = $26, last_info_sync = $27,
                imdb_rating = $28, imdb_votes = $29
            WHERE id = $30
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
            "SELECT * FROM episodes ORDER BY series_id, season_number, episode_number"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_series_id(&self, series_id: i64) -> Result<Vec<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE series_id = $1 ORDER BY season_number, episode_number"
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_imdb_id(&self, imdb_id: &str) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE imdb_id = $1"
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

    pub async fn get_by_series_and_season(&self, series_id: i64, season_number: i32) -> Result<Vec<super::models::EpisodeDbModel>> {
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

    pub async fn get_by_series_season_episode(&self, series_id: i64, season_number: i32, episode_number: i32) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE series_id = $1 AND season_number = $2 AND episode_number = $3"
        )
        .bind(series_id)
        .bind(season_number)
        .bind(episode_number)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_tvdb_id(&self, tvdb_id: i64) -> Result<Option<super::models::EpisodeDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::EpisodeDbModel>(
            "SELECT * FROM episodes WHERE tvdb_id = $1"
        )
        .bind(tvdb_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn update_monitored(&self, episode_ids: &[i64], monitored: bool) -> Result<Vec<super::models::EpisodeDbModel>> {
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
    pub async fn get_missing(
        &self,
        monitored_only: bool,
        page: i32,
        page_size: i32,
        sort_key: &str,
        sort_direction: &str,
    ) -> Result<(Vec<super::models::EpisodeDbModel>, i64)> {
        let order = if sort_direction.to_lowercase() == "descending" { "DESC" } else { "ASC" };
        let order_by = match sort_key {
            "airDateUtc" | "airDate" => "air_date_utc",
            "seriesTitle" => "series_id",
            "episodeTitle" | "title" => "title",
            _ => "air_date_utc",
        };
        let offset = (page - 1) * page_size;

        let pool = self.db.pool();
        let mut where_clause = "has_file = false AND air_date_utc IS NOT NULL AND air_date_utc < NOW()".to_string();
        if monitored_only {
            where_clause.push_str(" AND monitored = true");
        }

        let count_query = format!("SELECT COUNT(*) FROM episodes WHERE {}", where_clause);
        let total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(pool)
            .await?;

        let query = format!(
            "SELECT * FROM episodes WHERE {} ORDER BY {} {} LIMIT $1 OFFSET $2",
            where_clause, order_by, order
        );

        let rows = sqlx::query_as::<_, super::models::EpisodeDbModel>(&query)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;

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
    pub async fn get_cutoff_unmet(
        &self,
        _page: i32,
        _page_size: i32,
        _sort_key: &str,
        _sort_direction: &str,
    ) -> Result<(Vec<super::models::EpisodeDbModel>, i64)> {
        let pool = self.db.pool();
        // For now, we return episodes that have files but we could add quality comparison later
        // This would require joining with episode_files and quality_profiles
        let where_clause = "has_file = true AND monitored = true";
        let count_query = format!("SELECT COUNT(*) FROM episodes WHERE {}", where_clause);
        let _total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(pool)
            .await?;

        // For now, return empty - full implementation requires quality profile comparison
        Ok((vec![], 0))
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
            "SELECT * FROM quality_profiles ORDER BY name"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::QualityProfileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::QualityProfileDbModel>(
            "SELECT * FROM quality_profiles WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, name: &str, upgrade_allowed: bool, cutoff: i32, items: &str) -> Result<i64> {
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

    pub async fn update(&self, id: i64, name: &str, upgrade_allowed: bool, cutoff: i32, items: &str) -> Result<()> {
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
    pub body: Option<String>,
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
            "SELECT * FROM commands ORDER BY queued DESC LIMIT 100"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<CommandDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, CommandDbModel>(
            "SELECT * FROM commands WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, name: &str, command_name: &str, body: Option<&str>, trigger: &str) -> Result<i64> {
        let now = chrono::Utc::now();
        // Set send_updates_to_client=true for manual commands to enable UI updates
        let send_updates = trigger == "manual";

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
        .bind(body)
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
        let ended = if status == "completed" || status == "failed" { Some(now) } else { None };

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
            "#
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
        let order = if sort_direction.to_lowercase() == "ascending" { "ASC" } else { "DESC" };

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
    pub async fn insert(&self, level: &str, logger: &str, message: &str, exception: Option<&str>, exception_type: Option<&str>) -> Result<i64> {
        let now = chrono::Utc::now();

        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO logs (time, level, logger, message, exception, exception_type)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#
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
        let result = sqlx::query("DELETE FROM logs")
            .execute(pool)
            .await?;
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
            "SELECT * FROM download_clients ORDER BY priority, name"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::DownloadClientDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::DownloadClientDbModel>(
            "SELECT * FROM download_clients WHERE id = $1"
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
            INSERT INTO download_clients (enable, protocol, priority, name, implementation, config_contract, settings, tags)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
                implementation = $5, config_contract = $6, settings = $7, tags = $8
            WHERE id = $9
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
            "SELECT * FROM notifications ORDER BY name"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::NotificationDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::NotificationDbModel>(
            "SELECT * FROM notifications WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Get notifications enabled for a specific event type
    pub async fn get_enabled_for_event(&self, event_type: &str) -> Result<Vec<super::models::NotificationDbModel>> {
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
            "#
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
            "#
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

/// Repository for tracked downloads
pub struct TrackedDownloadRepository {
    db: Database,
}

impl TrackedDownloadRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Get all active (non-completed/non-failed) tracked downloads
    pub async fn get_all_active(&self) -> Result<Vec<super::models::TrackedDownloadDbModel>> {
        // Status values: 0=Downloading, 1=ImportBlocked, 2=ImportPending, 3=Importing
        // Completed states: 4=Imported, 5=FailedPending, 6=Failed, 7=Ignored
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::TrackedDownloadDbModel>(
            "SELECT * FROM tracked_downloads WHERE status < 4 ORDER BY added DESC"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Get all tracked downloads
    pub async fn get_all(&self) -> Result<Vec<super::models::TrackedDownloadDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::TrackedDownloadDbModel>(
            "SELECT * FROM tracked_downloads ORDER BY added DESC"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Get tracked download by ID
    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::TrackedDownloadDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::TrackedDownloadDbModel>(
            "SELECT * FROM tracked_downloads WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Get tracked download by download client ID and download ID
    pub async fn get_by_download_id(&self, client_id: i64, download_id: &str) -> Result<Option<super::models::TrackedDownloadDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::TrackedDownloadDbModel>(
            "SELECT * FROM tracked_downloads WHERE download_client_id = $1 AND download_id = $2"
        )
        .bind(client_id)
        .bind(download_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Get tracked downloads by series ID
    pub async fn get_by_series_id(&self, series_id: i64) -> Result<Vec<super::models::TrackedDownloadDbModel>> {
        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, super::models::TrackedDownloadDbModel>(
            "SELECT * FROM tracked_downloads WHERE series_id = $1 ORDER BY added DESC"
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Insert a new tracked download
    pub async fn insert(&self, download: &super::models::TrackedDownloadDbModel) -> Result<i64> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO tracked_downloads (
                download_id, download_client_id, series_id, episode_ids, title,
                indexer, size, protocol, quality, languages, status,
                status_messages, error_message, output_path, is_upgrade, added
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING id
            "#
        )
        .bind(&download.download_id)
        .bind(download.download_client_id)
        .bind(download.series_id)
        .bind(&download.episode_ids)
        .bind(&download.title)
        .bind(&download.indexer)
        .bind(download.size)
        .bind(download.protocol)
        .bind(&download.quality)
        .bind(&download.languages)
        .bind(download.status)
        .bind(&download.status_messages)
        .bind(&download.error_message)
        .bind(&download.output_path)
        .bind(download.is_upgrade)
        .bind(download.added)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    /// Update tracked download status
    pub async fn update_status(&self, id: i64, status: i32, status_messages: &str, error_message: Option<&str>) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query(
            "UPDATE tracked_downloads SET status = $1, status_messages = $2, error_message = $3 WHERE id = $4"
        )
        .bind(status)
        .bind(status_messages)
        .bind(error_message)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update tracked download output path
    pub async fn update_output_path(&self, id: i64, output_path: &str) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("UPDATE tracked_downloads SET output_path = $1 WHERE id = $2")
            .bind(output_path)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Delete a tracked download
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM tracked_downloads WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Delete by download client ID and download ID
    pub async fn delete_by_download_id(&self, client_id: i64, download_id: &str) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM tracked_downloads WHERE download_client_id = $1 AND download_id = $2")
            .bind(client_id)
            .bind(download_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

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
            "SELECT * FROM indexers ORDER BY priority, name"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::IndexerDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::IndexerDbModel>(
            "SELECT * FROM indexers WHERE id = $1"
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
            "SELECT * FROM blocklist ORDER BY date DESC"
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
        let order = if sort_direction.to_lowercase() == "ascending" { "ASC" } else { "DESC" };
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
            "SELECT * FROM blocklist WHERE id = $1"
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
            "#
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
            "SELECT * FROM custom_formats ORDER BY name"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::CustomFormatDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::CustomFormatDbModel>(
            "SELECT * FROM custom_formats WHERE id = $1"
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
            "#
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
            "#
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
            "SELECT * FROM custom_filters ORDER BY label"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::CustomFilterDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::CustomFilterDbModel>(
            "SELECT * FROM custom_filters WHERE id = $1"
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
            "#
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
            "#
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
            "SELECT * FROM remote_path_mappings ORDER BY host"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::RemotePathMappingDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::RemotePathMappingDbModel>(
            "SELECT * FROM remote_path_mappings WHERE id = $1"
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
            "#
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
            "#
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
            "SELECT * FROM delay_profiles ORDER BY id"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::DelayProfileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::DelayProfileDbModel>(
            "SELECT * FROM delay_profiles WHERE id = $1"
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
            "#
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
            "#
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
            "SELECT * FROM language_profiles ORDER BY name"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<super::models::LanguageProfileDbModel>> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, super::models::LanguageProfileDbModel>(
            "SELECT * FROM language_profiles WHERE id = $1"
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
            "#
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
            "#
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
