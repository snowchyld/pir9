//! Season Pass API endpoints (v5)

use axum::{response::Json, routing::post, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;
use sqlx;

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

pub async fn update_season_pass(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Json(body): Json<SeasonPassResource>,
) -> Json<serde_json::Value> {
    let pool = state.db.pool();

    for series_entry in &body.series {
        // Update series monitored status if specified
        if let Some(monitored) = series_entry.monitored {
            let _ = sqlx::query("UPDATE series SET monitored = $1 WHERE id = $2")
                .bind(monitored)
                .bind(series_entry.id as i64)
                .execute(pool)
                .await;
        }

        // Update individual season monitoring
        for season in &series_entry.seasons {
            let _ = sqlx::query(
                "UPDATE episodes SET monitored = $1 WHERE series_id = $2 AND season_number = $3",
            )
            .bind(season.monitored)
            .bind(series_entry.id as i64)
            .bind(season.season_number)
            .execute(pool)
            .await;
        }
    }

    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", post(update_season_pass))
}
