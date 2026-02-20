//! Series API endpoints

use axum::{
    extract::{Path, State},
    response::Json,
    routing::get,
    Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::core::datastore::repositories::SeriesRepository;
use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesResource {
    pub id: i32,
    pub title: String,
    pub sort_title: String,
    pub status: String,
    pub ended: bool,
    pub overview: String,
    pub network: String,
    pub air_time: Option<String>,
    pub images: Vec<ImageResource>,
    pub seasons: Vec<SeasonResource>,
    pub year: i32,
    pub path: String,
    pub quality_profile_id: i32,
    pub season_folder: bool,
    pub monitored: bool,
    pub use_scene_numbering: bool,
    pub runtime: i32,
    pub tvdb_id: i32,
    pub tv_rage_id: i32,
    pub tv_maze_id: i32,
    pub first_aired: Option<String>,
    pub series_type: String,
    pub clean_title: String,
    pub imdb_id: Option<String>,
    pub title_slug: String,
    pub root_folder_path: String,
    pub genres: Vec<String>,
    pub tags: Vec<i32>,
    pub added: String,
    pub ratings: RatingResource,
    pub statistics: SeriesStatisticsResource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageResource {
    pub cover_type: String,
    pub url: String,
    pub remote_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonResource {
    pub season_number: i32,
    pub monitored: bool,
    pub statistics: Option<SeasonStatisticsResource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonStatisticsResource {
    pub episode_file_count: i32,
    pub episode_count: i32,
    pub total_episode_count: i32,
    pub size_on_disk: i64,
    pub percent_of_episodes: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RatingResource {
    pub votes: i32,
    pub value: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesStatisticsResource {
    pub season_count: i32,
    pub episode_file_count: i32,
    pub episode_count: i32,
    pub total_episode_count: i32,
    pub size_on_disk: i64,
    pub percent_of_episodes: f64,
}

/// GET /api/v3/series
pub async fn get_series(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<SeriesResource>> {
    let repo = SeriesRepository::new(state.db.clone());

    let series = match repo.get_all().await {
        Ok(s) => s,
        Err(_) => return Json(vec![]),
    };

    let mut result = Vec::with_capacity(series.len());
    for s in series {
        // Fetch episode statistics
        let (episode_count, episode_file_count, season_count) =
            get_series_stats(&state.db, s.id).await;

        result.push(SeriesResource {
            id: s.id as i32,
            title: s.title.clone(),
            sort_title: s.sort_title.clone(),
            status: match s.status {
                0 => "continuing".to_string(),
                1 => "ended".to_string(),
                _ => "continuing".to_string(),
            },
            ended: s.status == 1,
            overview: s.overview.clone().unwrap_or_default(),
            network: s.network.clone().unwrap_or_default(),
            air_time: None,
            images: vec![
                ImageResource {
                    cover_type: "poster".to_string(),
                    url: format!("/MediaCover/Series/{}/poster.jpg", s.id),
                    remote_url: None,
                },
                ImageResource {
                    cover_type: "banner".to_string(),
                    url: format!("/MediaCover/Series/{}/banner.jpg", s.id),
                    remote_url: None,
                },
                ImageResource {
                    cover_type: "fanart".to_string(),
                    url: format!("/MediaCover/Series/{}/fanart.jpg", s.id),
                    remote_url: None,
                },
            ],
            seasons: vec![], // Would need to fetch seasons
            year: s.year,
            path: s.path.clone(),
            quality_profile_id: s.quality_profile_id as i32,
            season_folder: s.season_folder,
            monitored: s.monitored,
            use_scene_numbering: s.use_scene_numbering,
            runtime: s.runtime,
            tvdb_id: s.tvdb_id as i32,
            tv_rage_id: s.tv_rage_id as i32,
            tv_maze_id: s.tv_maze_id as i32,
            first_aired: s.first_aired.map(|d| d.to_string()),
            series_type: match s.series_type {
                0 => "standard".to_string(),
                1 => "daily".to_string(),
                2 => "anime".to_string(),
                _ => "standard".to_string(),
            },
            clean_title: s.clean_title.clone(),
            imdb_id: s.imdb_id.clone(),
            title_slug: s.title_slug.clone(),
            root_folder_path: s.root_folder_path.clone(),
            genres: vec![],
            tags: vec![],
            added: s.added.to_rfc3339(),
            ratings: RatingResource { votes: 0, value: 0.0 },
            statistics: SeriesStatisticsResource {
                season_count,
                episode_file_count,
                episode_count,
                total_episode_count: episode_count,
                size_on_disk: 0,
                percent_of_episodes: if episode_count > 0 {
                    (episode_file_count as f64 / episode_count as f64) * 100.0
                } else {
                    0.0
                },
            },
        });
    }

    Json(result)
}

/// GET /api/v3/series/:id
pub async fn get_series_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<Option<SeriesResource>> {
    let repo = SeriesRepository::new(state.db.clone());

    let series = match repo.get_by_id(id).await {
        Ok(Some(s)) => s,
        _ => return Json(None),
    };

    let (episode_count, episode_file_count, season_count) =
        get_series_stats(&state.db, series.id).await;

    Json(Some(SeriesResource {
        id: series.id as i32,
        title: series.title.clone(),
        sort_title: series.sort_title.clone(),
        status: match series.status {
            0 => "continuing".to_string(),
            1 => "ended".to_string(),
            _ => "continuing".to_string(),
        },
        ended: series.status == 1,
        overview: series.overview.clone().unwrap_or_default(),
        network: series.network.clone().unwrap_or_default(),
        air_time: None,
        images: vec![
            ImageResource {
                cover_type: "poster".to_string(),
                url: format!("/MediaCover/Series/{}/poster.jpg", series.id),
                remote_url: None,
            },
            ImageResource {
                cover_type: "banner".to_string(),
                url: format!("/MediaCover/Series/{}/banner.jpg", series.id),
                remote_url: None,
            },
            ImageResource {
                cover_type: "fanart".to_string(),
                url: format!("/MediaCover/Series/{}/fanart.jpg", series.id),
                remote_url: None,
            },
        ],
        seasons: vec![],
        year: series.year,
        path: series.path.clone(),
        quality_profile_id: series.quality_profile_id as i32,
        season_folder: series.season_folder,
        monitored: series.monitored,
        use_scene_numbering: series.use_scene_numbering,
        runtime: series.runtime,
        tvdb_id: series.tvdb_id as i32,
        tv_rage_id: series.tv_rage_id as i32,
        tv_maze_id: series.tv_maze_id as i32,
        first_aired: series.first_aired.map(|d| d.to_string()),
        series_type: match series.series_type {
            0 => "standard".to_string(),
            _ => "standard".to_string(),
        },
        clean_title: series.clean_title.clone(),
        imdb_id: series.imdb_id.clone(),
        title_slug: series.title_slug.clone(),
        root_folder_path: series.root_folder_path.clone(),
        genres: vec![],
        tags: vec![],
        added: series.added.to_rfc3339(),
        ratings: RatingResource { votes: 0, value: 0.0 },
        statistics: SeriesStatisticsResource {
            season_count,
            episode_file_count,
            episode_count,
            total_episode_count: episode_count,
            size_on_disk: 0,
            percent_of_episodes: if episode_count > 0 {
                (episode_file_count as f64 / episode_count as f64) * 100.0
            } else {
                0.0
            },
        },
    }))
}

async fn get_series_stats(db: &crate::core::datastore::Database, series_id: i64) -> (i32, i32, i32) {
    use sqlx::Row;

    let pool = db.pool();
    if let Ok(row) = sqlx::query(
        r#"
        SELECT
            COUNT(*)::int as episode_count,
            SUM(CASE WHEN has_file = true THEN 1 ELSE 0 END)::int as episode_file_count,
            COUNT(DISTINCT season_number)::int as season_count
        FROM episodes
        WHERE series_id = $1
        "#
    )
    .bind(series_id)
    .fetch_one(pool)
    .await {
        return (
            row.try_get::<i32, _>("episode_count").unwrap_or(0),
            row.try_get::<i32, _>("episode_file_count").unwrap_or(0),
            row.try_get::<i32, _>("season_count").unwrap_or(0),
        );
    }
    (0, 0, 0)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_series))
        .route("/{id}", get(get_series_by_id))
}
