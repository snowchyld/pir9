//! Remote Path Mapping API endpoints (v5)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::RemotePathMappingDbModel;
use crate::core::datastore::repositories::RemotePathMappingRepository;
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePathMappingResource {
    #[serde(default)]
    pub id: i64,
    pub host: String,
    pub remote_path: String,
    pub local_path: String,
}

pub async fn get_remote_path_mappings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RemotePathMappingResource>>, StatusCode> {
    let repo = RemotePathMappingRepository::new(state.db.clone());
    let items = repo
        .get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items.iter().map(db_to_resource).collect()))
}

pub async fn get_remote_path_mapping(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<RemotePathMappingResource>, StatusCode> {
    let repo = RemotePathMappingRepository::new(state.db.clone());
    let item = repo
        .get_by_id(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(db_to_resource(&item)))
}

pub async fn create_remote_path_mapping(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RemotePathMappingResource>,
) -> Result<impl IntoResponse, StatusCode> {
    let repo = RemotePathMappingRepository::new(state.db.clone());
    let model = resource_to_db(&body, None);
    let id = repo
        .insert(&model)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let created = repo
        .get_by_id(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(db_to_resource(&created))))
}

pub async fn update_remote_path_mapping(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<RemotePathMappingResource>,
) -> Result<Json<RemotePathMappingResource>, StatusCode> {
    let repo = RemotePathMappingRepository::new(state.db.clone());
    let _existing = repo
        .get_by_id(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let model = resource_to_db(&body, Some(id));
    repo.update(&model)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(db_to_resource(&model)))
}

pub async fn delete_remote_path_mapping(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> StatusCode {
    let repo = RemotePathMappingRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn db_to_resource(model: &RemotePathMappingDbModel) -> RemotePathMappingResource {
    RemotePathMappingResource {
        id: model.id,
        host: model.host.clone(),
        remote_path: model.remote_path.clone(),
        local_path: model.local_path.clone(),
    }
}

fn resource_to_db(
    resource: &RemotePathMappingResource,
    id: Option<i64>,
) -> RemotePathMappingDbModel {
    RemotePathMappingDbModel {
        id: id.unwrap_or(0),
        host: resource.host.clone(),
        remote_path: resource.remote_path.clone(),
        local_path: resource.local_path.clone(),
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/",
            get(get_remote_path_mappings).post(create_remote_path_mapping),
        )
        .route(
            "/{id}",
            get(get_remote_path_mapping)
                .put(update_remote_path_mapping)
                .delete(delete_remote_path_mapping),
        )
}
