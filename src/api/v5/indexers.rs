//! Indexer API endpoints (v5)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::IndexerDbModel;
use crate::core::datastore::repositories::IndexerRepository;
use crate::core::indexers::create_client_from_model;
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_indexers).post(create_indexer))
        .route("/{id}", get(get_indexer).put(update_indexer).delete(delete_indexer))
        .route("/{id}/test", post(test_indexer))
}

async fn list_indexers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<IndexerResponse>>, StatusCode> {
    let repo = IndexerRepository::new(state.db.clone());
    let indexers = repo.get_all().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(indexers.iter().map(db_to_response).collect()))
}

async fn get_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<IndexerResponse>, StatusCode> {
    let repo = IndexerRepository::new(state.db.clone());
    let indexer = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(db_to_response(&indexer)))
}

async fn create_indexer(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateIndexerRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let repo = IndexerRepository::new(state.db.clone());
    let settings = fields_to_settings(&body.fields);
    let tags = serde_json::to_string(&body.tags.unwrap_or_default()).unwrap_or_else(|_| "[]".to_string());

    let model = IndexerDbModel {
        id: 0,
        name: body.name,
        implementation: body.implementation,
        config_contract: body.config_contract,
        enable_rss: body.enable_rss.unwrap_or(true),
        enable_automatic_search: body.enable_automatic_search.unwrap_or(true),
        enable_interactive_search: body.enable_interactive_search.unwrap_or(true),
        protocol: body.protocol.unwrap_or(0),
        priority: body.priority.unwrap_or(25),
        download_client_id: body.download_client_id.unwrap_or(0),
        settings,
        tags,
    };

    let id = repo.insert(&model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let created = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(db_to_response(&created))))
}

async fn update_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateIndexerRequest>,
) -> Result<Json<IndexerResponse>, StatusCode> {
    let repo = IndexerRepository::new(state.db.clone());
    let mut model = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(name) = body.name { model.name = name; }
    if let Some(v) = body.enable_rss { model.enable_rss = v; }
    if let Some(v) = body.enable_automatic_search { model.enable_automatic_search = v; }
    if let Some(v) = body.enable_interactive_search { model.enable_interactive_search = v; }
    if let Some(fields) = body.fields { model.settings = fields_to_settings(&fields); }
    if let Some(tags) = body.tags {
        model.tags = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    }

    repo.update(&model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(db_to_response(&model)))
}

async fn delete_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> StatusCode {
    let repo = IndexerRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn test_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<TestResult> {
    let repo = IndexerRepository::new(state.db.clone());
    let model = match repo.get_by_id(id).await {
        Ok(Some(m)) => m,
        _ => return Json(TestResult { success: false, message: Some("Indexer not found".into()) }),
    };

    match create_client_from_model(&model) {
        Ok(client) => match client.fetch_rss(Some(1)).await {
            Ok(_) => Json(TestResult { success: true, message: None }),
            Err(e) => Json(TestResult { success: false, message: Some(e.to_string()) }),
        },
        Err(e) => Json(TestResult { success: false, message: Some(e.to_string()) }),
    }
}

fn db_to_response(model: &IndexerDbModel) -> IndexerResponse {
    let settings: serde_json::Value = serde_json::from_str(&model.settings)
        .unwrap_or(serde_json::json!({}));
    let tags: Vec<i32> = serde_json::from_str(&model.tags).unwrap_or_default();
    let fields = settings_to_fields(&settings);

    IndexerResponse {
        id: model.id,
        name: model.name.clone(),
        implementation: model.implementation.clone(),
        config_contract: model.config_contract.clone(),
        enable_rss: model.enable_rss,
        enable_automatic_search: model.enable_automatic_search,
        enable_interactive_search: model.enable_interactive_search,
        protocol: model.protocol,
        priority: model.priority,
        fields,
        tags,
    }
}

fn settings_to_fields(settings: &serde_json::Value) -> Vec<IndexerField> {
    match settings.as_object() {
        Some(obj) => obj.iter().map(|(k, v)| {
            IndexerField { name: k.clone(), value: v.clone() }
        }).collect(),
        None => vec![],
    }
}

fn fields_to_settings(fields: &[IndexerField]) -> String {
    let mut map = serde_json::Map::new();
    for field in fields {
        map.insert(field.name.clone(), field.value.clone());
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_else(|_| "{}".to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIndexerRequest {
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub fields: Vec<IndexerField>,
    pub enable_rss: Option<bool>,
    pub enable_automatic_search: Option<bool>,
    pub enable_interactive_search: Option<bool>,
    pub protocol: Option<i32>,
    pub priority: Option<i32>,
    pub download_client_id: Option<i64>,
    pub tags: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIndexerRequest {
    pub name: Option<String>,
    pub enable_rss: Option<bool>,
    pub enable_automatic_search: Option<bool>,
    pub enable_interactive_search: Option<bool>,
    pub fields: Option<Vec<IndexerField>>,
    pub tags: Option<Vec<i32>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerField {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerResponse {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub config_contract: String,
    pub enable_rss: bool,
    pub enable_automatic_search: bool,
    pub enable_interactive_search: bool,
    pub protocol: i32,
    pub priority: i32,
    pub fields: Vec<IndexerField>,
    pub tags: Vec<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub success: bool,
    pub message: Option<String>,
}
