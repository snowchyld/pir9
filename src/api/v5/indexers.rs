//! Indexer API endpoints

use axum::{
    Router,
    routing::{get, post},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_indexers).post(create_indexer))
        .route("/{id}", get(get_indexer).put(update_indexer).delete(delete_indexer))
        .route("/{id}/test", post(test_indexer))
}

async fn list_indexers() -> Json<Vec<IndexerResponse>> {
    Json(vec![])
}

async fn get_indexer() -> Json<IndexerResponse> {
    Json(IndexerResponse::default())
}

async fn create_indexer(
    Json(create): Json<CreateIndexerRequest>,
) -> Json<IndexerResponse> {
    Json(IndexerResponse::default())
}

async fn update_indexer(
    Json(update): Json<UpdateIndexerRequest>,
) -> Json<IndexerResponse> {
    Json(IndexerResponse::default())
}

async fn delete_indexer() {}

async fn test_indexer() -> Json<TestResult> {
    Json(TestResult { success: true, message: None })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIndexerRequest {
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub fields: Vec<IndexerField>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIndexerRequest {
    pub name: Option<String>,
    pub enable_rss: Option<bool>,
    pub enable_automatic_search: Option<bool>,
    pub enable_interactive_search: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerField {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IndexerResponse {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub enable_rss: bool,
    pub enable_automatic_search: bool,
    pub enable_interactive_search: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub success: bool,
    pub message: Option<String>,
}
