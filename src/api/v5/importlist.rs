//! Import List API endpoints (v5)
//!
//! CRUD operations for import lists and manual sync trigger.

use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::ImportListDbModel;
use crate::core::datastore::repositories::ImportListRepository;
use crate::core::importlists;
use crate::web::AppState;

/// Import list API resource
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportListResource {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub list_type: String,
    pub list_url: Option<String>,
    pub root_folder_path: String,
    #[serde(default = "default_quality_profile_id")]
    pub quality_profile_id: i64,
    #[serde(default = "default_true")]
    pub monitored: bool,
    #[serde(default = "default_true")]
    pub search_on_add: bool,
    #[serde(default = "default_content_type")]
    pub content_type: String,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_hours: i32,
    pub last_synced_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub tags: Vec<i64>,
}

fn default_true() -> bool {
    true
}

fn default_quality_profile_id() -> i64 {
    1
}

fn default_content_type() -> String {
    "movie".to_string()
}

fn default_sync_interval() -> i32 {
    24
}

/// Convert database model to API resource
fn model_to_resource(model: &ImportListDbModel) -> ImportListResource {
    let tags: Vec<i64> = serde_json::from_str(&model.tags).unwrap_or_default();

    ImportListResource {
        id: model.id,
        name: model.name.clone(),
        enabled: model.enabled,
        list_type: model.list_type.clone(),
        list_url: model.list_url.clone(),
        root_folder_path: model.root_folder_path.clone(),
        quality_profile_id: model.quality_profile_id,
        monitored: model.monitored,
        search_on_add: model.search_on_add,
        content_type: model.content_type.clone(),
        sync_interval_hours: model.sync_interval_hours,
        last_synced_at: model.last_synced_at,
        tags,
    }
}

/// Convert API resource to database model
fn resource_to_model(resource: &ImportListResource) -> ImportListDbModel {
    let tags = serde_json::to_string(&resource.tags).unwrap_or_else(|_| "[]".to_string());

    ImportListDbModel {
        id: resource.id,
        name: resource.name.clone(),
        enabled: resource.enabled,
        list_type: resource.list_type.clone(),
        list_url: resource.list_url.clone(),
        root_folder_path: resource.root_folder_path.clone(),
        quality_profile_id: resource.quality_profile_id,
        monitored: resource.monitored,
        search_on_add: resource.search_on_add,
        content_type: resource.content_type.clone(),
        sync_interval_hours: resource.sync_interval_hours,
        last_synced_at: resource.last_synced_at,
        tags: tags.into(),
    }
}

/// GET /api/v5/importlist — list all import lists
pub async fn get_import_lists(State(state): State<Arc<AppState>>) -> Json<Vec<ImportListResource>> {
    let repo = ImportListRepository::new(state.db.clone());

    match repo.get_all().await {
        Ok(lists) => {
            let resources: Vec<ImportListResource> = lists.iter().map(model_to_resource).collect();
            Json(resources)
        }
        Err(e) => {
            tracing::error!("Failed to get import lists: {}", e);
            Json(vec![])
        }
    }
}

/// GET /api/v5/importlist/:id — get a single import list
pub async fn get_import_list(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<Option<ImportListResource>> {
    let repo = ImportListRepository::new(state.db.clone());

    match repo.get_by_id(id).await {
        Ok(Some(list)) => Json(Some(model_to_resource(&list))),
        Ok(None) => Json(None),
        Err(e) => {
            tracing::error!("Failed to get import list {}: {}", id, e);
            Json(None)
        }
    }
}

/// POST /api/v5/importlist — create a new import list
pub async fn create_import_list(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ImportListResource>,
) -> Json<ImportListResource> {
    let repo = ImportListRepository::new(state.db.clone());
    let model = resource_to_model(&body);

    match repo.insert(&model).await {
        Ok(id) => {
            let mut created = body;
            created.id = id;
            Json(created)
        }
        Err(e) => {
            tracing::error!("Failed to create import list: {}", e);
            Json(body)
        }
    }
}

/// PUT /api/v5/importlist/:id — update an import list
pub async fn update_import_list(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(mut body): Json<ImportListResource>,
) -> Json<ImportListResource> {
    body.id = id;
    let repo = ImportListRepository::new(state.db.clone());
    let model = resource_to_model(&body);

    match repo.update(&model).await {
        Ok(()) => Json(body),
        Err(e) => {
            tracing::error!("Failed to update import list {}: {}", id, e);
            Json(body)
        }
    }
}

/// DELETE /api/v5/importlist/:id — delete an import list
pub async fn delete_import_list(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let repo = ImportListRepository::new(state.db.clone());

    match repo.delete(id).await {
        Ok(()) => Json(serde_json::json!({})),
        Err(e) => {
            tracing::error!("Failed to delete import list {}: {}", id, e);
            Json(serde_json::json!({ "error": e.to_string() }))
        }
    }
}

/// Sync result for the API response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncResultResponse {
    list_id: i64,
    list_name: String,
    items_found: usize,
    items_added: usize,
    items_skipped: usize,
    items_excluded: usize,
    errors: Vec<String>,
}

/// POST /api/v5/importlist/:id/sync — trigger manual sync for a single list
pub async fn sync_import_list(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let repo = ImportListRepository::new(state.db.clone());

    let list = match repo.get_by_id(id).await {
        Ok(Some(l)) => l,
        Ok(None) => {
            return Json(serde_json::json!({ "error": "Import list not found" }));
        }
        Err(e) => {
            return Json(serde_json::json!({ "error": format!("Database error: {}", e) }));
        }
    };

    match importlists::sync_import_list(&list, &state.db, &state.imdb_client).await {
        Ok(result) => {
            // Update last_synced_at
            let _ = repo.update_last_synced(id).await;

            Json(serde_json::json!({
                "listId": result.list_id,
                "listName": result.list_name,
                "itemsFound": result.items_found,
                "itemsAdded": result.items_added,
                "itemsSkipped": result.items_skipped,
                "itemsExcluded": result.items_excluded,
                "errors": result.errors,
            }))
        }
        Err(e) => Json(serde_json::json!({ "error": format!("Sync failed: {}", e) })),
    }
}

/// POST /api/v5/importlist/sync — trigger sync for all enabled lists
pub async fn sync_all_import_lists(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<serde_json::Value>> {
    match importlists::sync_all_import_lists(&state.db, &state.imdb_client).await {
        Ok(results) => {
            let responses: Vec<serde_json::Value> = results
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "listId": r.list_id,
                        "listName": r.list_name,
                        "itemsFound": r.items_found,
                        "itemsAdded": r.items_added,
                        "itemsSkipped": r.items_skipped,
                        "itemsExcluded": r.items_excluded,
                        "errors": r.errors,
                    })
                })
                .collect();
            Json(responses)
        }
        Err(e) => {
            tracing::error!("Failed to sync all import lists: {}", e);
            Json(vec![serde_json::json!({ "error": e.to_string() })])
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_import_lists).post(create_import_list))
        .route(
            "/{id}",
            get(get_import_list)
                .put(update_import_list)
                .delete(delete_import_list),
        )
        .route("/{id}/sync", post(sync_import_list))
        .route("/sync", post(sync_all_import_lists))
}
