//! Download client API endpoints (v5)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::DownloadClientDbModel;
use crate::core::datastore::repositories::DownloadClientRepository;
use crate::core::download::clients::create_client_from_model;
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_download_clients).post(create_download_client))
        .route("/{id}", get(get_download_client).put(update_download_client).delete(delete_download_client))
        .route("/{id}/test", post(test_download_client))
}

async fn list_download_clients(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DownloadClientResponse>>, StatusCode> {
    let repo = DownloadClientRepository::new(state.db.clone());
    let clients = repo.get_all().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(clients.iter().map(db_to_response).collect()))
}

async fn get_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<DownloadClientResponse>, StatusCode> {
    let repo = DownloadClientRepository::new(state.db.clone());
    let client = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(db_to_response(&client)))
}

async fn create_download_client(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateDownloadClientRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let repo = DownloadClientRepository::new(state.db.clone());
    let settings = fields_to_settings(&body.fields);
    let tags = serde_json::to_string(&body.tags.unwrap_or_default()).unwrap_or_else(|_| "[]".to_string());

    let model = DownloadClientDbModel {
        id: 0,
        enable: body.enable.unwrap_or(true),
        protocol: body.protocol.unwrap_or(0),
        priority: body.priority.unwrap_or(1),
        name: body.name,
        implementation: body.implementation,
        config_contract: body.config_contract,
        settings,
        tags,
    };

    let id = repo.insert(&model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let created = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(db_to_response(&created))))
}

async fn update_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateDownloadClientRequest>,
) -> Result<Json<DownloadClientResponse>, StatusCode> {
    let repo = DownloadClientRepository::new(state.db.clone());
    let mut model = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(name) = body.name { model.name = name; }
    if let Some(enable) = body.enable { model.enable = enable; }
    if let Some(priority) = body.priority { model.priority = priority; }
    if let Some(fields) = body.fields { model.settings = fields_to_settings(&fields); }
    if let Some(tags) = body.tags {
        model.tags = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    }

    repo.update(&model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(db_to_response(&model)))
}

async fn delete_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> StatusCode {
    let repo = DownloadClientRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn test_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<TestResult> {
    let repo = DownloadClientRepository::new(state.db.clone());
    let model = match repo.get_by_id(id).await {
        Ok(Some(m)) => m,
        _ => return Json(TestResult { success: false, message: Some("Client not found".into()) }),
    };

    match create_client_from_model(&model) {
        Ok(client) => match client.test().await {
            Ok(()) => Json(TestResult { success: true, message: None }),
            Err(e) => Json(TestResult { success: false, message: Some(e.to_string()) }),
        },
        Err(e) => Json(TestResult { success: false, message: Some(e.to_string()) }),
    }
}

fn db_to_response(model: &DownloadClientDbModel) -> DownloadClientResponse {
    let settings: serde_json::Value = serde_json::from_str(&model.settings)
        .unwrap_or(serde_json::json!({}));
    let tags: Vec<i32> = serde_json::from_str(&model.tags).unwrap_or_default();
    let fields = settings_to_fields(&settings);

    DownloadClientResponse {
        id: model.id,
        name: model.name.clone(),
        implementation: model.implementation.clone(),
        config_contract: model.config_contract.clone(),
        enable: model.enable,
        protocol: model.protocol,
        priority: model.priority,
        fields,
        tags,
    }
}

fn settings_to_fields(settings: &serde_json::Value) -> Vec<DownloadClientField> {
    match settings.as_object() {
        Some(obj) => obj.iter().enumerate().map(|(i, (k, v))| {
            DownloadClientField {
                order: i as i32,
                name: k.clone(),
                value: v.clone(),
            }
        }).collect(),
        None => vec![],
    }
}

fn fields_to_settings(fields: &[DownloadClientField]) -> String {
    let mut map = serde_json::Map::new();
    for field in fields {
        map.insert(field.name.clone(), field.value.clone());
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_else(|_| "{}".to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDownloadClientRequest {
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub fields: Vec<DownloadClientField>,
    pub enable: Option<bool>,
    pub protocol: Option<i32>,
    pub priority: Option<i32>,
    pub tags: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDownloadClientRequest {
    pub name: Option<String>,
    pub enable: Option<bool>,
    pub priority: Option<i32>,
    pub fields: Option<Vec<DownloadClientField>>,
    pub tags: Option<Vec<i32>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadClientField {
    #[serde(default)]
    pub order: i32,
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadClientResponse {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub enable: bool,
    pub protocol: i32,
    pub priority: i32,
    pub fields: Vec<DownloadClientField>,
    pub tags: Vec<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub success: bool,
    pub message: Option<String>,
}
