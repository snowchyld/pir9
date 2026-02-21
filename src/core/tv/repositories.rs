#![allow(dead_code, unused_imports)]
//! TV Show data repositories
//! Database access layer for Series and Episode entities

use anyhow::{Context, Result};
use sqlx::Row;
use std::collections::HashSet;

use crate::core::datastore::Database;
use crate::core::tv::models::{Episode, Series};
use crate::core::tv::{NewItemMonitorType, SeriesStatusType, SeriesType};

/// Repository for Series entities
#[derive(Debug, Clone)]
pub struct SeriesRepository {
    db: Database,
}

impl SeriesRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find series by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<Series>> {
        let pool = self.db.pool();
        let row = sqlx::query(
            r#"
            SELECT s.*,
                   (SELECT COUNT(*) FROM episodes WHERE series_id = s.id) as episode_count,
                   (SELECT COUNT(*) FROM episodes WHERE series_id = s.id AND has_file = true) as episode_file_count
            FROM series s
            WHERE s.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_series(&row)?)),
            None => Ok(None),
        }
    }

    /// Find series by TVDB ID
    pub async fn find_by_tvdb_id(&self, tvdb_id: i64) -> Result<Option<Series>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM series WHERE tvdb_id = $1")
            .bind(tvdb_id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_series(&row)?)),
            None => Ok(None),
        }
    }

    /// Find all series
    pub async fn find_all(&self) -> Result<Vec<Series>> {
        let pool = self.db.pool();
        let rows = sqlx::query("SELECT * FROM series ORDER BY sort_title")
            .fetch_all(pool)
            .await?;

        let mut series = Vec::new();
        for row in rows {
            series.push(self.row_to_series(&row)?);
        }
        Ok(series)
    }

    /// Insert a new series
    pub async fn insert(&self, series: &Series) -> Result<Series> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO series (
                tvdb_id, tv_rage_id, tv_maze_id, imdb_id, tmdb_id,
                title, clean_title, sort_title, status, overview,
                monitored, monitor_new_items, quality_profile_id, language_profile_id,
                season_folder, series_type, title_slug, path, root_folder_path,
                year, first_aired, last_aired, runtime, network,
                certification, use_scene_numbering, episode_ordering, added, last_info_sync
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29)
            RETURNING id
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
        .bind(series.status as i32)
        .bind(&series.overview)
        .bind(series.monitored)
        .bind(series.monitor_new_items as i32)
        .bind(series.quality_profile_id)
        .bind(series.language_profile_id)
        .bind(series.season_folder)
        .bind(series.series_type as i32)
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
        .fetch_one(pool)
        .await?;

        self.find_by_id(row.0)
            .await?
            .context("Failed to retrieve inserted series")
    }

    /// Update an existing series
    pub async fn update(&self, series: &Series) -> Result<Series> {
        let pool = self.db.pool();

        // Resolve tvdb_id conflicts: clear the conflicting series' tvdb_id
        if series.tvdb_id > 0 {
            if let Some(existing) = self.find_by_tvdb_id(series.tvdb_id).await? {
                if existing.id != series.id {
                    tracing::warn!(
                        "Series update: tvdb_id {} conflicts with series id={} '{}', clearing conflicting tvdb_id",
                        series.tvdb_id, existing.id, existing.title
                    );
                    let _ = sqlx::query(
                        "UPDATE series SET tvdb_id = 0 WHERE id = $1",
                    )
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
                year = $20, first_aired = $21, last_aired = $22, runtime = $23, network = $24,
                certification = $25, use_scene_numbering = $26, episode_ordering = $27, last_info_sync = $28
            WHERE id = $29
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
        .bind(series.status as i32)
        .bind(&series.overview)
        .bind(series.monitored)
        .bind(series.monitor_new_items as i32)
        .bind(series.quality_profile_id)
        .bind(series.language_profile_id)
        .bind(series.season_folder)
        .bind(series.series_type as i32)
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
        .bind(series.id)
        .execute(pool)
        .await?;

        self.find_by_id(series.id)
            .await?
            .context("Failed to retrieve updated series")
    }

    /// Delete a series
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        // Delete episodes first (foreign key constraint)
        sqlx::query("DELETE FROM episodes WHERE series_id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        sqlx::query("DELETE FROM series WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Convert PostgreSQL row to Series entity
    fn row_to_series(&self, row: &sqlx::postgres::PgRow) -> Result<Series> {
        Ok(Series {
            id: row.try_get("id")?,
            tvdb_id: row.try_get("tvdb_id")?,
            tv_rage_id: row.try_get("tv_rage_id")?,
            tv_maze_id: row.try_get("tv_maze_id")?,
            imdb_id: row.try_get("imdb_id")?,
            tmdb_id: row.try_get("tmdb_id")?,
            mal_ids: HashSet::new(),
            anilist_ids: HashSet::new(),
            title: row.try_get("title")?,
            clean_title: row.try_get("clean_title")?,
            sort_title: row.try_get("sort_title")?,
            status: match row.try_get::<i32, _>("status")? {
                0 => SeriesStatusType::Continuing,
                1 => SeriesStatusType::Ended,
                2 => SeriesStatusType::Upcoming,
                _ => SeriesStatusType::Deleted,
            },
            overview: row.try_get("overview")?,
            air_time: None,
            monitored: row.try_get("monitored")?,
            monitor_new_items: match row.try_get::<i32, _>("monitor_new_items")? {
                0 => NewItemMonitorType::All,
                1 => NewItemMonitorType::Future,
                2 => NewItemMonitorType::Missing,
                3 => NewItemMonitorType::Existing,
                4 => NewItemMonitorType::None,
                5 => NewItemMonitorType::Pilot,
                6 => NewItemMonitorType::FirstSeason,
                7 => NewItemMonitorType::LastSeason,
                _ => NewItemMonitorType::All,
            },
            quality_profile_id: row.try_get("quality_profile_id")?,
            quality_profile: None,
            language_profile_id: row.try_get("language_profile_id")?,
            season_folder: row.try_get("season_folder")?,
            series_type: match row.try_get::<i32, _>("series_type")? {
                0 => SeriesType::Standard,
                1 => SeriesType::Daily,
                _ => SeriesType::Anime,
            },
            title_slug: row.try_get("title_slug")?,
            path: row.try_get("path")?,
            root_folder_path: row.try_get("root_folder_path")?,
            year: row.try_get("year")?,
            first_aired: row.try_get("first_aired")?,
            last_aired: row.try_get("last_aired")?,
            runtime: row.try_get("runtime")?,
            network: row.try_get("network")?,
            certification: row.try_get("certification")?,
            genres: Vec::new(),
            actors: Vec::new(),
            images: Vec::new(),
            ratings: None,
            use_scene_numbering: row.try_get("use_scene_numbering")?,
            episode_ordering: row.try_get::<String, _>("episode_ordering").unwrap_or_else(|_| "aired".to_string()),
            seasons: Vec::new(),
            tags: HashSet::new(),
            statistics: None,
            added: row.try_get("added")?,
            last_info_sync: row.try_get("last_info_sync")?,
        })
    }
}

/// Repository for Episode entities
#[derive(Debug, Clone)]
pub struct EpisodeRepository {
    db: Database,
}

impl EpisodeRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find episode by ID
    pub async fn find_by_id(&self, id: i64) -> Result<Option<Episode>> {
        let pool = self.db.pool();
        let row = sqlx::query("SELECT * FROM episodes WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_episode(&row)?)),
            None => Ok(None),
        }
    }

    /// Find episodes by series ID
    pub async fn find_by_series_id(&self, series_id: i64) -> Result<Vec<Episode>> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            "SELECT * FROM episodes WHERE series_id = $1 ORDER BY season_number, episode_number",
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;

        let mut episodes = Vec::new();
        for row in rows {
            episodes.push(self.row_to_episode(&row)?);
        }
        Ok(episodes)
    }

    /// Find episode by series, season, and episode number
    pub async fn find_by_series_season_episode(
        &self,
        series_id: i64,
        season_number: i32,
        episode_number: i32,
    ) -> Result<Option<Episode>> {
        let pool = self.db.pool();
        let row = sqlx::query(
            "SELECT * FROM episodes WHERE series_id = $1 AND season_number = $2 AND episode_number = $3",
        )
        .bind(series_id)
        .bind(season_number)
        .bind(episode_number)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_episode(&row)?)),
            None => Ok(None),
        }
    }

    /// Find missing episodes for a series
    pub async fn find_missing_by_series(&self, series_id: i64) -> Result<Vec<Episode>> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            r#"
            SELECT * FROM episodes
            WHERE series_id = $1
              AND monitored = true
              AND has_file = false
              AND air_date_utc IS NOT NULL
              AND air_date_utc < NOW()
            ORDER BY season_number, episode_number
            "#,
        )
        .bind(series_id)
        .fetch_all(pool)
        .await?;

        let mut episodes = Vec::new();
        for row in rows {
            episodes.push(self.row_to_episode(&row)?);
        }
        Ok(episodes)
    }

    /// Insert a new episode
    pub async fn insert(&self, episode: &Episode) -> Result<Episode> {
        let pool = self.db.pool();
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO episodes (
                series_id, tvdb_id, episode_file_id, season_number, episode_number,
                absolute_episode_number, scene_absolute_episode_number, scene_episode_number,
                scene_season_number, title, overview, air_date, air_date_utc, runtime,
                has_file, monitored, unverified_scene_numbering, added
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            RETURNING id
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
        .fetch_one(pool)
        .await?;

        self.find_by_id(row.0)
            .await?
            .context("Failed to retrieve inserted episode")
    }

    /// Update an existing episode
    pub async fn update(&self, episode: &Episode) -> Result<Episode> {
        let pool = self.db.pool();
        sqlx::query(
            r#"
            UPDATE episodes SET
                tvdb_id = $1, episode_file_id = $2, title = $3, overview = $4,
                air_date = $5, air_date_utc = $6, runtime = $7, has_file = $8,
                monitored = $9, unverified_scene_numbering = $10,
                absolute_episode_number = $11, last_search_time = $12
            WHERE id = $13
            "#,
        )
        .bind(episode.tvdb_id)
        .bind(episode.episode_file_id)
        .bind(&episode.title)
        .bind(&episode.overview)
        .bind(episode.air_date)
        .bind(episode.air_date_utc)
        .bind(episode.runtime)
        .bind(episode.has_file)
        .bind(episode.monitored)
        .bind(episode.unverified_scene_numbering)
        .bind(episode.absolute_episode_number)
        .bind(episode.last_search_time)
        .bind(episode.id)
        .execute(pool)
        .await?;

        self.find_by_id(episode.id)
            .await?
            .context("Failed to retrieve updated episode")
    }

    /// Delete an episode
    pub async fn delete(&self, id: i64) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("DELETE FROM episodes WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Convert PostgreSQL row to Episode entity
    fn row_to_episode(&self, row: &sqlx::postgres::PgRow) -> Result<Episode> {
        Ok(Episode {
            id: row.try_get("id")?,
            series_id: row.try_get("series_id")?,
            tvdb_id: row.try_get("tvdb_id")?,
            episode_file_id: row.try_get("episode_file_id")?,
            season_number: row.try_get("season_number")?,
            episode_number: row.try_get("episode_number")?,
            absolute_episode_number: row.try_get("absolute_episode_number")?,
            scene_absolute_episode_number: row.try_get("scene_absolute_episode_number")?,
            scene_episode_number: row.try_get("scene_episode_number")?,
            scene_season_number: row.try_get("scene_season_number")?,
            title: row.try_get("title")?,
            overview: row.try_get("overview")?,
            air_date: row.try_get("air_date")?,
            air_date_utc: row.try_get("air_date_utc")?,
            runtime: row.try_get("runtime")?,
            finale_type: None,
            has_file: row.try_get("has_file")?,
            monitored: row.try_get("monitored")?,
            unverified_scene_numbering: row.try_get("unverified_scene_numbering")?,
            images: Vec::new(),
            added: row.try_get("added")?,
            last_search_time: row.try_get("last_search_time")?,
        })
    }
}
