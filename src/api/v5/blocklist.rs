//! Blocklist API endpoints (v5)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::BlocklistDbModel;
use crate::core::datastore::repositories::BlocklistRepository;
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
    pub id: i64,
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
    pub source_title: String,
    pub languages: serde_json::Value,
    pub quality: serde_json::Value,
    pub custom_formats: serde_json::Value,
    pub custom_format_score: i32,
    pub date: String,
    pub protocol: i32,
    pub indexer: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocklistPagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i64,
    pub records: Vec<BlocklistResource>,
}

pub async fn get_blocklist(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BlocklistQuery>,
) -> Result<Json<BlocklistPagingResource>, StatusCode> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);
    let sort_key = query.sort_key.unwrap_or_else(|| "date".to_string());
    let sort_direction = query.sort_direction.unwrap_or_else(|| "descending".to_string());

    let repo = BlocklistRepository::new(state.db.clone());
    let (items, total) = repo.get_paged(page, page_size, &sort_key, &sort_direction).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(BlocklistPagingResource {
        page,
        page_size,
        sort_key,
        sort_direction,
        total_records: total,
        records: items.iter().map(db_to_resource).collect(),
    }))
}

pub async fn delete_blocklist_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> StatusCode {
    let repo = BlocklistRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkDeleteRequest {
    pub ids: Vec<i64>,
}

pub async fn delete_blocklist_bulk(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkDeleteRequest>,
) -> StatusCode {
    let repo = BlocklistRepository::new(state.db.clone());
    match repo.delete_bulk(&body.ids).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn db_to_resource(model: &BlocklistDbModel) -> BlocklistResource {
    let episode_ids: Vec<i64> = serde_json::from_str(&model.episode_ids).unwrap_or_default();
    let quality: serde_json::Value = serde_json::from_str(&model.quality)
        .unwrap_or(serde_json::json!({}));
    let languages: serde_json::Value = serde_json::from_str(&model.languages)
        .unwrap_or(serde_json::json!([]));
    let custom_formats: serde_json::Value = serde_json::from_str(&model.custom_formats)
        .unwrap_or(serde_json::json!([]));

    BlocklistResource {
        id: model.id,
        series_id: model.series_id,
        episode_ids,
        source_title: model.source_title.clone(),
        languages,
        quality,
        custom_formats,
        custom_format_score: model.custom_format_score,
        date: model.date.to_rfc3339(),
        protocol: model.protocol,
        indexer: model.indexer.clone(),
        message: model.message.clone(),
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_blocklist))
        .route("/{id}", delete(delete_blocklist_item))
        .route("/bulk", delete(delete_blocklist_bulk))
}
