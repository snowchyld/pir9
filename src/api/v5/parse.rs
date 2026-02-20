//! Parse API endpoints (v5)

use axum::{extract::Query, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ParseQuery {
    pub title: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResource {
    pub title: Option<String>,
    pub parsed_episode_info: Option<serde_json::Value>,
    pub series: Option<serde_json::Value>,
    pub episodes: Vec<serde_json::Value>,
    pub languages: Vec<serde_json::Value>,
    pub quality: Option<serde_json::Value>,
    pub custom_formats: Vec<serde_json::Value>,
    pub custom_format_score: i32,
}

pub async fn parse(_query: Query<ParseQuery>) -> Json<ParseResource> {
    Json(ParseResource {
        title: None,
        parsed_episode_info: None,
        series: None,
        episodes: vec![],
        languages: vec![],
        quality: None,
        custom_formats: vec![],
        custom_format_score: 0,
    })
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(parse))
}
