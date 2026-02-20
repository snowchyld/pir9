//! Season Pass API endpoints (v5)

use axum::{response::Json, routing::post, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonPassResource {
    pub series: Vec<SeasonPassSeriesResource>,
    pub monitoring_options: MonitoringOptionsResource,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonPassSeriesResource {
    pub id: i32,
    pub monitored: Option<bool>,
    pub seasons: Vec<SeasonResource>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonResource {
    pub season_number: i32,
    pub monitored: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringOptionsResource {
    pub ignore_episodes_with_files: bool,
    pub ignore_episodes_without_files: bool,
    pub monitor: Option<String>,
}

pub async fn update_season_pass(Json(_body): Json<SeasonPassResource>) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", post(update_season_pass))
}
