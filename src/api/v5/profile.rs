//! Profile API endpoints (v5) - Quality, Delay, Release profiles

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::QualityProfileRepository;
use crate::web::AppState;

// Quality Profile
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityProfileResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    pub upgrade_allowed: bool,
    pub cutoff: i32,
    pub items: Vec<QualityProfileItemResource>,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub format_items: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityProfileItemResource {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub quality: Option<QualityResource>,
    pub items: Vec<QualityProfileItemResource>,
    pub allowed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

impl From<crate::core::datastore::models::QualityProfileDbModel> for QualityProfileResource {
    fn from(p: crate::core::datastore::models::QualityProfileDbModel) -> Self {
        let items: Vec<QualityProfileItemResource> =
            serde_json::from_str(&p.items).unwrap_or_default();
        let format_items: Vec<serde_json::Value> =
            serde_json::from_str(&p.format_items).unwrap_or_default();

        Self {
            id: p.id as i32,
            name: p.name,
            upgrade_allowed: p.upgrade_allowed,
            cutoff: p.cutoff,
            items,
            min_format_score: p.min_format_score,
            cutoff_format_score: p.cutoff_format_score,
            format_items,
        }
    }
}

pub async fn get_quality_profiles(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<QualityProfileResource>>, ProfileError> {
    let repo = QualityProfileRepository::new(state.db.clone());

    let profiles = repo
        .get_all()
        .await
        .map_err(|e| ProfileError::Internal(format!("Failed to fetch quality profiles: {}", e)))?;

    let resources: Vec<QualityProfileResource> = profiles.into_iter().map(Into::into).collect();
    Ok(Json(resources))
}

pub async fn get_quality_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<QualityProfileResource>, ProfileError> {
    let repo = QualityProfileRepository::new(state.db.clone());

    let profile = repo
        .get_by_id(id as i64)
        .await
        .map_err(|e| ProfileError::Internal(format!("Failed to fetch quality profile: {}", e)))?
        .ok_or(ProfileError::NotFound)?;

    Ok(Json(profile.into()))
}

pub async fn create_quality_profile(
    State(state): State<Arc<AppState>>,
    Json(body): Json<QualityProfileResource>,
) -> Result<Json<QualityProfileResource>, ProfileError> {
    if body.name.trim().is_empty() {
        return Err(ProfileError::Validation("Name is required".to_string()));
    }

    let repo = QualityProfileRepository::new(state.db.clone());

    let items_json = serde_json::to_string(&body.items)
        .map_err(|e| ProfileError::Internal(format!("Failed to serialize items: {}", e)))?;

    let id = repo
        .insert(&body.name, body.upgrade_allowed, body.cutoff, &items_json)
        .await
        .map_err(|e| ProfileError::Internal(format!("Failed to create quality profile: {}", e)))?;

    tracing::info!("Created quality profile: id={}, name={}", id, body.name);

    let profile = repo
        .get_by_id(id)
        .await
        .map_err(|e| ProfileError::Internal(format!("Failed to fetch quality profile: {}", e)))?
        .ok_or(ProfileError::NotFound)?;

    Ok(Json(profile.into()))
}

pub async fn update_quality_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<QualityProfileResource>,
) -> Result<Json<QualityProfileResource>, ProfileError> {
    if body.name.trim().is_empty() {
        return Err(ProfileError::Validation("Name is required".to_string()));
    }

    let repo = QualityProfileRepository::new(state.db.clone());

    // Verify profile exists
    repo.get_by_id(id as i64)
        .await
        .map_err(|e| ProfileError::Internal(format!("Failed to fetch quality profile: {}", e)))?
        .ok_or(ProfileError::NotFound)?;

    let items_json = serde_json::to_string(&body.items)
        .map_err(|e| ProfileError::Internal(format!("Failed to serialize items: {}", e)))?;

    repo.update(
        id as i64,
        &body.name,
        body.upgrade_allowed,
        body.cutoff,
        &items_json,
    )
    .await
    .map_err(|e| ProfileError::Internal(format!("Failed to update quality profile: {}", e)))?;

    tracing::info!("Updated quality profile: id={}, name={}", id, body.name);

    let profile = repo
        .get_by_id(id as i64)
        .await
        .map_err(|e| ProfileError::Internal(format!("Failed to fetch quality profile: {}", e)))?
        .ok_or(ProfileError::NotFound)?;

    Ok(Json(profile.into()))
}

pub async fn delete_quality_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, ProfileError> {
    let repo = QualityProfileRepository::new(state.db.clone());

    repo.delete(id as i64)
        .await
        .map_err(|e| ProfileError::Internal(format!("Failed to delete quality profile: {}", e)))?;

    tracing::info!("Deleted quality profile: id={}", id);

    Ok(Json(serde_json::json!({})))
}

// Delay Profile
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
    pub order: i32,
    pub tags: Vec<i32>,
}

pub async fn get_delay_profiles() -> Json<Vec<DelayProfileResource>> {
    // Default delay profile - in a full implementation would come from DB
    Json(vec![DelayProfileResource {
        id: 1,
        enable_usenet: true,
        enable_torrent: true,
        preferred_protocol: "usenet".to_string(),
        usenet_delay: 0,
        torrent_delay: 0,
        bypass_if_highest_quality: true,
        order: i32::MAX,
        tags: vec![],
    }])
}

pub async fn get_delay_profile(Path(id): Path<i32>) -> Json<Option<DelayProfileResource>> {
    if id == 1 {
        Json(Some(DelayProfileResource {
            id: 1,
            enable_usenet: true,
            enable_torrent: true,
            preferred_protocol: "usenet".to_string(),
            usenet_delay: 0,
            torrent_delay: 0,
            bypass_if_highest_quality: true,
            order: i32::MAX,
            tags: vec![],
        }))
    } else {
        Json(None)
    }
}

pub async fn create_delay_profile(
    Json(body): Json<DelayProfileResource>,
) -> Json<DelayProfileResource> {
    Json(body)
}

pub async fn update_delay_profile(
    Path(id): Path<i32>,
    Json(mut body): Json<DelayProfileResource>,
) -> Json<DelayProfileResource> {
    body.id = id;
    Json(body)
}

pub async fn delete_delay_profile(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

// Release Profile
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseProfileResource {
    #[serde(default)]
    pub id: i32,
    pub name: Option<String>,
    pub enabled: bool,
    pub required: Vec<String>,
    pub ignored: Vec<String>,
    pub indexer_id: i32,
    pub tags: Vec<i32>,
}

pub async fn get_release_profiles() -> Json<Vec<ReleaseProfileResource>> {
    Json(vec![])
}

pub async fn get_release_profile(Path(id): Path<i32>) -> Json<Option<ReleaseProfileResource>> {
    let _ = id;
    Json(None)
}

pub async fn create_release_profile(
    Json(body): Json<ReleaseProfileResource>,
) -> Json<ReleaseProfileResource> {
    Json(body)
}

pub async fn update_release_profile(
    Path(id): Path<i32>,
    Json(mut body): Json<ReleaseProfileResource>,
) -> Json<ReleaseProfileResource> {
    body.id = id;
    Json(body)
}

pub async fn delete_release_profile(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// Error type for profile operations
#[derive(Debug)]
pub enum ProfileError {
    NotFound,
    Validation(String),
    Internal(String),
}

impl IntoResponse for ProfileError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ProfileError::NotFound => (StatusCode::NOT_FOUND, "Profile not found".to_string()),
            ProfileError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            ProfileError::Internal(msg) => {
                tracing::error!("Profile error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

pub fn quality_profile_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_quality_profiles).post(create_quality_profile))
        .route(
            "/{id}",
            get(get_quality_profile)
                .put(update_quality_profile)
                .delete(delete_quality_profile),
        )
}

pub fn delay_profile_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_delay_profiles).post(create_delay_profile))
        .route(
            "/{id}",
            get(get_delay_profile)
                .put(update_delay_profile)
                .delete(delete_delay_profile),
        )
}

pub fn release_profile_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_release_profiles).post(create_release_profile))
        .route(
            "/{id}",
            get(get_release_profile)
                .put(update_release_profile)
                .delete(delete_release_profile),
        )
}
