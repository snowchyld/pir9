//! Delay Profile API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::DelayProfileDbModel;
use crate::core::datastore::repositories::DelayProfileRepository;
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelayProfileResource {
    #[serde(default)]
    pub id: i32,
    pub enable_usenet: bool,
    pub enable_torrent: bool,
    pub preferred_protocol: String,
    pub usenet_delay: i32,
    pub torrent_delay: i32,
    pub bypass_if_highest_quality: bool,
    #[serde(default)]
    pub bypass_if_above_custom_format_score: bool,
    #[serde(default)]
    pub minimum_custom_format_score: i32,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub tags: Vec<i32>,
}

fn protocol_to_string(protocol: i32) -> String {
    match protocol {
        1 => "usenet".to_string(),
        2 => "torrent".to_string(),
        _ => "unknown".to_string(),
    }
}

fn protocol_from_string(protocol: &str) -> i32 {
    match protocol {
        "usenet" => 1,
        "torrent" => 2,
        _ => 0,
    }
}

fn db_to_resource(model: &DelayProfileDbModel) -> DelayProfileResource {
    let tags: Vec<i32> = serde_json::from_str(&model.tags).unwrap_or_default();
    DelayProfileResource {
        id: model.id as i32,
        enable_usenet: model.enable_usenet,
        enable_torrent: model.enable_torrent,
        preferred_protocol: protocol_to_string(model.preferred_protocol),
        usenet_delay: model.usenet_delay,
        torrent_delay: model.torrent_delay,
        bypass_if_highest_quality: model.bypass_if_highest_quality,
        bypass_if_above_custom_format_score: model.bypass_if_above_custom_format_score > 0,
        minimum_custom_format_score: model.bypass_if_above_custom_format_score,
        order: model.id as i32,
        tags,
    }
}

fn resource_to_db(resource: &DelayProfileResource, id: Option<i64>) -> DelayProfileDbModel {
    DelayProfileDbModel {
        id: id.unwrap_or(0),
        enable_usenet: resource.enable_usenet,
        enable_torrent: resource.enable_torrent,
        preferred_protocol: protocol_from_string(&resource.preferred_protocol),
        usenet_delay: resource.usenet_delay,
        torrent_delay: resource.torrent_delay,
        bypass_if_highest_quality: resource.bypass_if_highest_quality,
        bypass_if_above_custom_format_score: resource.minimum_custom_format_score,
        tags: serde_json::to_string(&resource.tags).unwrap_or_else(|_| "[]".to_string()),
    }
}

pub async fn get_delay_profiles(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DelayProfileResource>>, StatusCode> {
    let repo = DelayProfileRepository::new(state.db.clone());
    let items = repo.get_all().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items.iter().map(db_to_resource).collect()))
}

pub async fn get_delay_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<DelayProfileResource>, StatusCode> {
    let repo = DelayProfileRepository::new(state.db.clone());
    let item = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(db_to_resource(&item)))
}

pub async fn create_delay_profile(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DelayProfileResource>,
) -> Result<impl IntoResponse, StatusCode> {
    let repo = DelayProfileRepository::new(state.db.clone());
    let model = resource_to_db(&body, None);
    let id = repo.insert(&model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let created = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(db_to_resource(&created))))
}

pub async fn update_delay_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<DelayProfileResource>,
) -> Result<Json<DelayProfileResource>, StatusCode> {
    let repo = DelayProfileRepository::new(state.db.clone());
    let _existing = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let model = resource_to_db(&body, Some(id));
    repo.update(&model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(db_to_resource(&model)))
}

pub async fn delete_delay_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> StatusCode {
    let repo = DelayProfileRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub async fn reorder_delay_profile(
    State(state): State<Arc<AppState>>,
    Path(_id): Path<i64>,
) -> Result<Json<Vec<DelayProfileResource>>, StatusCode> {
    let repo = DelayProfileRepository::new(state.db.clone());
    let items = repo.get_all().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items.iter().map(db_to_resource).collect()))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_delay_profiles).post(create_delay_profile))
        .route("/{id}", get(get_delay_profile).put(update_delay_profile).delete(delete_delay_profile))
        .route("/reorder/{id}", put(reorder_delay_profile))
}
