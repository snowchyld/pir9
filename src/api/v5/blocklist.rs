//! Blocklist API endpoints (v5)

use axum::{
    extract::{Path, Query},
    response::Json,
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocklistQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocklistResource {
    pub id: i32,
    pub series_id: i32,
    pub episode_ids: Vec<i32>,
    pub source_title: String,
    pub languages: Vec<LanguageResource>,
    pub quality: QualityModel,
    pub date: String,
    pub protocol: String,
    pub indexer: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocklistPagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i32,
    pub records: Vec<BlocklistResource>,
}

pub async fn get_blocklist(query: Query<BlocklistQuery>) -> Json<BlocklistPagingResource> {
    Json(BlocklistPagingResource {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_key: query.sort_key.clone().unwrap_or("date".to_string()),
        sort_direction: query.sort_direction.clone().unwrap_or("descending".to_string()),
        total_records: 0,
        records: vec![],
    })
}

pub async fn delete_blocklist_item(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

pub async fn delete_blocklist_bulk(Json(_body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_blocklist))
        .route("/{id}", delete(delete_blocklist_item))
        .route("/bulk", delete(delete_blocklist_bulk))
}
