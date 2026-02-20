//! Health API endpoints

use axum::{response::Json, routing::get, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResource {
    pub source: String,
    #[serde(rename = "type")]
    pub health_type: String,
    pub message: String,
    pub wiki_url: Option<String>,
}

/// GET /api/v3/health
pub async fn get_health() -> Json<Vec<HealthResource>> {
    // Return empty health issues (everything is healthy)
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_health))
}
