//! Download client API endpoints

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
        .route("/", get(list_download_clients).post(create_download_client))
        .route("/{id}", get(get_download_client).put(update_download_client).delete(delete_download_client))
        .route("/{id}/test", post(test_download_client))
}

async fn list_download_clients() -> Json<Vec<DownloadClientResponse>> {
    Json(vec![])
}

async fn get_download_client() -> Json<DownloadClientResponse> {
    Json(DownloadClientResponse::default())
}

async fn create_download_client(
    Json(create): Json<CreateDownloadClientRequest>,
) -> Json<DownloadClientResponse> {
    Json(DownloadClientResponse::default())
}

async fn update_download_client(
    Json(update): Json<UpdateDownloadClientRequest>,
) -> Json<DownloadClientResponse> {
    Json(DownloadClientResponse::default())
}

async fn delete_download_client() {}

async fn test_download_client() -> Json<TestResult> {
    Json(TestResult { success: true, message: None })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDownloadClientRequest {
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub fields: Vec<DownloadClientField>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDownloadClientRequest {
    pub name: Option<String>,
    pub enable: Option<bool>,
    pub priority: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadClientField {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DownloadClientResponse {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub enable: bool,
    pub priority: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub success: bool,
    pub message: Option<String>,
}
