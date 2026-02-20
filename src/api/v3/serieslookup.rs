//! Series Lookup API endpoints (v3)

use axum::{extract::Query, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct SeriesLookupQuery {
    pub term: Option<String>,
    pub tvdb_id: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesLookupResource {
    pub title: String,
    pub sort_title: String,
    pub status: String,
    pub ended: bool,
    pub overview: String,
    pub network: Option<String>,
    pub air_time: Option<String>,
    pub images: Vec<ImageResource>,
    pub remote_poster: Option<String>,
    pub seasons: Vec<SeasonResource>,
    pub year: i32,
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
    pub folder: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<i32>,
    pub added: String,
    pub ratings: RatingResource,
    pub statistics: Option<SeriesStatisticsResource>,
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

/// Skyhook API response structures
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookSearchResult {
    tvdb_id: i64,
    title: String,
    sort_title: Option<String>,
    overview: Option<String>,
    year: Option<i32>,
    status: Option<String>,
    images: Option<Vec<SkyhookImage>>,
    seasons: Option<Vec<SkyhookSeason>>,
    ratings: Option<SkyhookRatings>,
    genres: Option<Vec<String>>,
    network: Option<String>,
    runtime: Option<i32>,
    tvrage_id: Option<i64>,
    imdb_id: Option<String>,
    first_aired: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookImage {
    cover_type: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookSeason {
    season_number: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookRatings {
    votes: Option<i64>,
    value: Option<f64>,
}

fn clean_title(title: &str) -> String {
    title.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("")
}

fn generate_slug(title: &str) -> String {
    title.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

/// GET /api/v3/series/lookup
pub async fn lookup_series(Query(query): Query<SeriesLookupQuery>) -> Json<Vec<SeriesLookupResource>> {
    let term = match &query.term {
        Some(t) => t.clone(),
        None => return Json(vec![]),
    };

    // Query Sonarr's Skyhook service
    let url = format!(
        "http://skyhook.sonarr.tv/v1/tvdb/search/en/?term={}",
        urlencoding::encode(&term)
    );

    let client = reqwest::Client::new();
    let response = match client
        .get(&url)
        .header("User-Agent", "pir9/0.1.0")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to query Skyhook: {}", e);
            return Json(vec![]);
        }
    };

    if !response.status().is_success() {
        tracing::error!("Skyhook returned status: {}", response.status());
        return Json(vec![]);
    }

    let skyhook_results: Vec<SkyhookSearchResult> = match response.json().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to parse Skyhook response: {}", e);
            return Json(vec![]);
        }
    };

    // Convert Skyhook results to v3 format
    let results: Vec<SeriesLookupResource> = skyhook_results
        .into_iter()
        .map(|s| {
            let status = s.status.clone().unwrap_or_else(|| "unknown".to_string());
            let ended = status.to_lowercase() == "ended";
            let poster_url = s.images.as_ref()
                .and_then(|imgs| imgs.iter().find(|i| i.cover_type == "poster"))
                .map(|i| i.url.clone());

            SeriesLookupResource {
                title: s.title.clone(),
                sort_title: s.sort_title.unwrap_or_else(|| s.title.clone()),
                status,
                ended,
                overview: s.overview.unwrap_or_default(),
                network: s.network,
                air_time: None,
                images: s.images.unwrap_or_default().into_iter().map(|img| ImageResource {
                    cover_type: img.cover_type,
                    url: img.url.clone(),
                    remote_url: Some(img.url),
                }).collect(),
                remote_poster: poster_url,
                seasons: s.seasons.unwrap_or_default().into_iter().map(|season| SeasonResource {
                    season_number: season.season_number,
                    monitored: true,
                }).collect(),
                year: s.year.unwrap_or(0),
                quality_profile_id: 1,
                season_folder: true,
                monitored: true,
                use_scene_numbering: false,
                runtime: s.runtime.unwrap_or(0),
                tvdb_id: s.tvdb_id as i32,
                tv_rage_id: s.tvrage_id.unwrap_or(0) as i32,
                tv_maze_id: 0,
                first_aired: s.first_aired,
                series_type: "standard".to_string(),
                clean_title: clean_title(&s.title),
                imdb_id: s.imdb_id,
                title_slug: generate_slug(&s.title),
                folder: None,
                genres: s.genres.unwrap_or_default(),
                tags: vec![],
                added: chrono::Utc::now().to_rfc3339(),
                ratings: RatingResource {
                    votes: s.ratings.as_ref().and_then(|r| r.votes).unwrap_or(0) as i32,
                    value: s.ratings.as_ref().and_then(|r| r.value).unwrap_or(0.0),
                },
                statistics: None,
            }
        })
        .collect();

    Json(results)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(lookup_series))
}
