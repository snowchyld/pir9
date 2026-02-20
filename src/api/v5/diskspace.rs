//! Disk Space API endpoints (v5)

use axum::{response::Json, routing::get, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpaceResource {
    pub path: String,
    pub label: String,
    pub free_space: i64,
    pub total_space: i64,
}

pub async fn get_disk_space() -> Json<Vec<DiskSpaceResource>> {
    Json(vec![DiskSpaceResource {
        path: "/".to_string(),
        label: "Root".to_string(),
        free_space: 100_000_000_000,
        total_space: 500_000_000_000,
    }])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_disk_space))
}
