//! Custom Format API endpoints (v5)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::CustomFormatDbModel;
use crate::core::datastore::repositories::CustomFormatRepository;
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFormatResource {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub include_custom_format_when_renaming: bool,
    pub specifications: Vec<serde_json::Value>,
}

pub async fn get_custom_formats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CustomFormatResource>>, StatusCode> {
    let repo = CustomFormatRepository::new(state.db.clone());
    let items = repo
        .get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items.iter().map(db_to_resource).collect()))
}

pub async fn get_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<CustomFormatResource>, StatusCode> {
    let repo = CustomFormatRepository::new(state.db.clone());
    let item = repo
        .get_by_id(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(db_to_resource(&item)))
}

pub async fn create_custom_format(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CustomFormatResource>,
) -> Result<impl IntoResponse, StatusCode> {
    let repo = CustomFormatRepository::new(state.db.clone());
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

pub async fn update_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<CustomFormatResource>,
) -> Result<Json<CustomFormatResource>, StatusCode> {
    let repo = CustomFormatRepository::new(state.db.clone());
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

pub async fn delete_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> StatusCode {
    let repo = CustomFormatRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub async fn get_custom_format_schema() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

fn db_to_resource(model: &CustomFormatDbModel) -> CustomFormatResource {
    let specifications: Vec<serde_json::Value> =
        serde_json::from_str(&model.specifications).unwrap_or_default();
    CustomFormatResource {
        id: model.id,
        name: model.name.clone(),
        include_custom_format_when_renaming: model.include_custom_format_when_renaming,
        specifications,
    }
}

fn resource_to_db(resource: &CustomFormatResource, id: Option<i64>) -> CustomFormatDbModel {
    CustomFormatDbModel {
        id: id.unwrap_or(0),
        name: resource.name.clone(),
        include_custom_format_when_renaming: resource.include_custom_format_when_renaming,
        specifications: serde_json::to_string(&resource.specifications)
            .unwrap_or_else(|_| "[]".to_string()),
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_custom_formats).post(create_custom_format))
        .route(
            "/{id}",
            get(get_custom_format)
                .put(update_custom_format)
                .delete(delete_custom_format),
        )
        .route("/schema", get(get_custom_format_schema))
}
