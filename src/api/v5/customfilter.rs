//! Custom Filter API endpoints (v5)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::CustomFilterDbModel;
use crate::core::datastore::repositories::CustomFilterRepository;
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFilterResource {
    #[serde(default)]
    pub id: i64,
    #[serde(rename = "type")]
    pub filter_type: String,
    pub label: String,
    #[serde(default)]
    pub filters: Vec<serde_json::Value>,
}

pub async fn get_custom_filters(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CustomFilterResource>>, StatusCode> {
    let repo = CustomFilterRepository::new(state.db.clone());
    let items = repo
        .get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items.iter().map(db_to_resource).collect()))
}

pub async fn get_custom_filter(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<CustomFilterResource>, StatusCode> {
    let repo = CustomFilterRepository::new(state.db.clone());
    let item = repo
        .get_by_id(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(db_to_resource(&item)))
}

pub async fn create_custom_filter(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CustomFilterResource>,
) -> Result<impl IntoResponse, StatusCode> {
    let repo = CustomFilterRepository::new(state.db.clone());
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

pub async fn update_custom_filter(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<CustomFilterResource>,
) -> Result<Json<CustomFilterResource>, StatusCode> {
    let repo = CustomFilterRepository::new(state.db.clone());
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

pub async fn delete_custom_filter(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> StatusCode {
    let repo = CustomFilterRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn db_to_resource(model: &CustomFilterDbModel) -> CustomFilterResource {
    let filters: Vec<serde_json::Value> = serde_json::from_str(&model.filters).unwrap_or_default();
    CustomFilterResource {
        id: model.id,
        filter_type: model.filter_type.clone(),
        label: model.label.clone(),
        filters,
    }
}

fn resource_to_db(resource: &CustomFilterResource, id: Option<i64>) -> CustomFilterDbModel {
    CustomFilterDbModel {
        id: id.unwrap_or(0),
        filter_type: resource.filter_type.clone(),
        label: resource.label.clone(),
        filters: serde_json::to_string(&resource.filters).unwrap_or_else(|_| "[]".to_string()),
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_custom_filters).post(create_custom_filter))
        .route(
            "/{id}",
            get(get_custom_filter)
                .put(update_custom_filter)
                .delete(delete_custom_filter),
        )
}
