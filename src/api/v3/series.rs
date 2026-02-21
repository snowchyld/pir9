//! Series API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::SeriesDbModel;
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
    pub episode_ordering: String,
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
pub async fn get_series(State(state): State<Arc<AppState>>) -> Json<Vec<SeriesResource>> {
    let repo = SeriesRepository::new(state.db.clone());

    let series = match repo.get_all().await {
        Ok(s) => s,
        Err(_) => return Json(vec![]),
    };

    let mut result = Vec::with_capacity(series.len());
    for s in series {
        // Fetch episode statistics
        let (episode_count, episode_file_count, season_count, total_episode_count) =
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
            use_scene_numbering: s.use_scene_numbering || s.episode_ordering != "aired",
            episode_ordering: s.episode_ordering.clone(),
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
            ratings: RatingResource {
                votes: 0,
                value: 0.0,
            },
            statistics: SeriesStatisticsResource {
                season_count,
                episode_file_count,
                episode_count,
                total_episode_count,
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

    let (episode_count, episode_file_count, season_count, total_episode_count) =
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
        use_scene_numbering: series.use_scene_numbering || series.episode_ordering != "aired",
        episode_ordering: series.episode_ordering.clone(),
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
        ratings: RatingResource {
            votes: 0,
            value: 0.0,
        },
        statistics: SeriesStatisticsResource {
            season_count,
            episode_file_count,
            episode_count,
            total_episode_count,
            size_on_disk: 0,
            percent_of_episodes: if episode_count > 0 {
                (episode_file_count as f64 / episode_count as f64) * 100.0
            } else {
                0.0
            },
        },
    }))
}

async fn get_series_stats(
    db: &crate::core::datastore::Database,
    series_id: i64,
) -> (i32, i32, i32, i32) {
    use sqlx::Row;

    let pool = db.pool();
    if let Ok(row) = sqlx::query(
        r#"
        SELECT
            COUNT(CASE WHEN monitored = true THEN 1 END)::int as episode_count,
            SUM(CASE WHEN has_file = true AND monitored = true THEN 1 ELSE 0 END)::int as episode_file_count,
            COUNT(DISTINCT season_number)::int as season_count,
            COUNT(*)::int as total_episode_count
        FROM episodes
        WHERE series_id = $1
        "#,
    )
    .bind(series_id)
    .fetch_one(pool)
    .await
    {
        return (
            row.try_get::<i32, _>("episode_count").unwrap_or(0),
            row.try_get::<i32, _>("episode_file_count").unwrap_or(0),
            row.try_get::<i32, _>("season_count").unwrap_or(0),
            row.try_get::<i32, _>("total_episode_count").unwrap_or(0),
        );
    }
    (0, 0, 0, 0)
}

/// Sonarr v3 add-series request body
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct AddSeriesRequest {
    pub tvdb_id: Option<i64>,
    #[serde(alias = "tvdbid")]
    pub tvdbid: Option<i64>,
    pub title: String,
    #[serde(default = "default_profile_id")]
    pub quality_profile_id: i64,
    #[serde(default)]
    pub profile_id: Option<i64>,
    pub root_folder_path: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub monitored: bool,
    #[serde(default = "default_series_type")]
    pub series_type: String,
    #[serde(default)]
    pub season_folder: bool,
    #[serde(default)]
    pub tags: Vec<i64>,
    #[serde(default)]
    pub seasons: Vec<serde_json::Value>,
    #[serde(default)]
    pub language_profile_id: Option<i64>,
    // AddOptions for Sonarr scripts
    #[serde(default)]
    pub add_options: Option<serde_json::Value>,
}

fn default_profile_id() -> i64 {
    1
}

fn default_series_type() -> String {
    "standard".to_string()
}

/// POST /api/v3/series — add a new series (Sonarr v3 compatible)
async fn add_series(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddSeriesRequest>,
) -> impl IntoResponse {
    // Accept tvdbId from either field name (tvdb_id or tvdbid)
    let tvdb_id = body.tvdb_id.or(body.tvdbid).unwrap_or(0);
    if tvdb_id == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"errorMessage": "tvdbId is required"})),
        )
            .into_response();
    }

    let profile_id = body.profile_id.unwrap_or(body.quality_profile_id);

    tracing::info!(
        "v3 add series: tvdb_id={}, title='{}', root_folder='{}', profile={}",
        tvdb_id,
        body.title,
        body.root_folder_path.as_deref().unwrap_or(""),
        profile_id
    );

    let repo = SeriesRepository::new(state.db.clone());

    // Check if already exists
    if let Ok(Some(existing)) = repo.get_by_tvdb_id(tvdb_id).await {
        tracing::info!("Series already exists: id={}, title={}", existing.id, existing.title);
        let (episode_count, episode_file_count, season_count, total_episode_count) =
            get_series_stats(&state.db, existing.id).await;
        return (
            StatusCode::OK,
            Json(serde_json::to_value(build_series_resource(
                existing,
                episode_count,
                episode_file_count,
                season_count,
                total_episode_count,
            )).expect("serialize")),
        )
            .into_response();
    }

    // Build path: rootFolderPath/Title or explicit path
    let full_path = body.path.clone().unwrap_or_else(|| {
        let root = body.root_folder_path.as_deref().unwrap_or("/data/series");
        format!("{}/{}", root.trim_end_matches('/'), body.title)
    });
    let root_folder_path = body.root_folder_path.clone().unwrap_or_default();

    let clean = body
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("");

    let slug = body
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string();

    let series_type_int = match body.series_type.as_str() {
        "anime" => 2,
        "daily" => 1,
        _ => 0,
    };

    let db_series = SeriesDbModel {
        id: 0,
        tvdb_id,
        tv_rage_id: 0,
        tv_maze_id: 0,
        imdb_id: None,
        tmdb_id: 0,
        title: body.title.clone(),
        clean_title: clean.clone(),
        sort_title: clean,
        status: 0,
        overview: None,
        monitored: body.monitored,
        monitor_new_items: 0,
        quality_profile_id: profile_id,
        language_profile_id: body.language_profile_id,
        season_folder: body.season_folder,
        series_type: series_type_int,
        title_slug: slug,
        path: full_path,
        root_folder_path,
        year: 0,
        first_aired: None,
        last_aired: None,
        runtime: 0,
        network: None,
        certification: None,
        use_scene_numbering: false,
        episode_ordering: "aired".to_string(),
        added: chrono::Utc::now(),
        last_info_sync: None,
        imdb_rating: None,
        imdb_votes: None,
    };

    let id = match repo.insert(&db_series).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to insert series: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"errorMessage": format!("Failed to add series: {}", e)})),
            )
                .into_response();
        }
    };

    tracing::info!("Created series via v3: id={}, title={}, tvdb_id={}", id, body.title, tvdb_id);

    // Create series folder on disk
    let series_path = &db_series.path;
    if !series_path.is_empty() {
        let path = std::path::Path::new(series_path);
        if !path.exists() {
            match tokio::fs::create_dir_all(path).await {
                Ok(()) => tracing::info!("Created series folder: {}", series_path),
                Err(e) => tracing::warn!("Failed to create series folder {}: {}", series_path, e),
            }
        }
    }

    // Spawn background refresh (fetch episodes + metadata from Skyhook/IMDB)
    let db_clone = state.db.clone();
    let metadata_svc = state.metadata_service.clone();
    let hybrid_bus = state.hybrid_event_bus.clone();
    let consumer = state.scan_result_consumer.get().cloned();
    let title_clone = body.title.clone();
    tokio::spawn(async move {
        tracing::info!("Auto-refreshing new series: {} (id={})", title_clone, id);
        if let Err(e) = crate::api::v5::series::auto_refresh_series(id, &db_clone, &metadata_svc).await {
            tracing::error!("Failed to auto-refresh series {}: {}", id, e);
        }
        if let Err(e) = crate::api::v5::series::auto_scan_series(
            id,
            &db_clone,
            hybrid_bus.as_ref(),
            consumer.as_ref(),
        ).await {
            tracing::error!("Failed to auto-scan series {}: {}", id, e);
        }
    });

    // Fetch and return created series
    let created = match repo.get_by_id(id).await {
        Ok(Some(s)) => s,
        _ => {
            return (
                StatusCode::CREATED,
                Json(serde_json::json!({"id": id, "title": body.title})),
            )
                .into_response();
        }
    };

    let (episode_count, episode_file_count, season_count, total_episode_count) =
        get_series_stats(&state.db, created.id).await;

    (
        StatusCode::CREATED,
        Json(
            serde_json::to_value(build_series_resource(
                created,
                episode_count,
                episode_file_count,
                season_count,
                total_episode_count,
            ))
            .expect("serialize"),
        ),
    )
        .into_response()
}

fn build_series_resource(
    s: SeriesDbModel,
    episode_count: i32,
    episode_file_count: i32,
    season_count: i32,
    total_episode_count: i32,
) -> SeriesResource {
    SeriesResource {
        id: s.id as i32,
        title: s.title.clone(),
        sort_title: s.sort_title.clone(),
        status: match s.status {
            0 => "continuing".to_string(),
            1 => "ended".to_string(),
            _ => "continuing".to_string(),
        },
        ended: s.status == 1,
        overview: s.overview.unwrap_or_default(),
        network: s.network.unwrap_or_default(),
        air_time: None,
        images: vec![
            ImageResource { cover_type: "poster".to_string(), url: format!("/MediaCover/Series/{}/poster.jpg", s.id), remote_url: None },
            ImageResource { cover_type: "banner".to_string(), url: format!("/MediaCover/Series/{}/banner.jpg", s.id), remote_url: None },
            ImageResource { cover_type: "fanart".to_string(), url: format!("/MediaCover/Series/{}/fanart.jpg", s.id), remote_url: None },
        ],
        seasons: vec![],
        year: s.year,
        path: s.path,
        quality_profile_id: s.quality_profile_id as i32,
        season_folder: s.season_folder,
        monitored: s.monitored,
        use_scene_numbering: s.use_scene_numbering || s.episode_ordering != "aired",
        episode_ordering: s.episode_ordering.clone(),
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
        clean_title: s.clean_title,
        imdb_id: s.imdb_id,
        title_slug: s.title_slug,
        root_folder_path: s.root_folder_path,
        genres: vec![],
        tags: vec![],
        added: s.added.to_rfc3339(),
        ratings: RatingResource { votes: 0, value: 0.0 },
        statistics: SeriesStatisticsResource {
            season_count,
            episode_file_count,
            episode_count,
            total_episode_count,
            size_on_disk: 0,
            percent_of_episodes: if episode_count > 0 {
                (episode_file_count as f64 / episode_count as f64) * 100.0
            } else {
                0.0
            },
        },
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_series).post(add_series))
        .route("/{id}", get(get_series_by_id))
}
