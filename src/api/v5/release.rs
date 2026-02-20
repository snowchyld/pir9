//! Release API endpoints (v5)

use axum::{extract::Query, response::Json, routing::{get, post}, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseQuery {
    pub series_id: Option<i32>,
    pub episode_id: Option<i32>,
    pub season_number: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseResource {
    pub guid: String,
    pub quality: serde_json::Value,
    pub age: i32,
    pub age_hours: f64,
    pub age_minutes: f64,
    pub size: i64,
    pub indexer_id: i32,
    pub indexer: String,
    pub release_group: Option<String>,
    pub title: String,
    pub full_season: bool,
    pub season_number: i32,
    pub languages: Vec<serde_json::Value>,
    pub episode_numbers: Vec<i32>,
    pub approved: bool,
    pub rejected: bool,
    pub rejections: Vec<String>,
    pub publish_date: String,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub protocol: String,
}

pub async fn get_releases(_query: Query<ReleaseQuery>) -> Json<Vec<ReleaseResource>> {
    Json(vec![])
}

pub async fn create_release(Json(_body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

pub async fn push_release(Json(_body): Json<serde_json::Value>) -> Json<Vec<ReleaseResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_releases).post(create_release))
        .route("/push", post(push_release))
}
