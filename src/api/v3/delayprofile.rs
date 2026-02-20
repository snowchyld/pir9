//! Delay Profile API endpoints

use axum::{
    extract::Path,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelayProfileResource {
    pub id: i32,
    pub enable_usenet: bool,
    pub enable_torrent: bool,
    pub preferred_protocol: String,
    pub usenet_delay: i32,
    pub torrent_delay: i32,
    pub bypass_if_highest_quality: bool,
    pub bypass_if_above_custom_format_score: bool,
    pub minimum_custom_format_score: i32,
    pub order: i32,
    pub tags: Vec<i32>,
}

/// GET /api/v3/delayprofile
pub async fn get_delay_profiles() -> Json<Vec<DelayProfileResource>> {
    Json(vec![
        DelayProfileResource {
            id: 1,
            enable_usenet: true,
            enable_torrent: true,
            preferred_protocol: "usenet".to_string(),
            usenet_delay: 0,
            torrent_delay: 0,
            bypass_if_highest_quality: true,
            bypass_if_above_custom_format_score: false,
            minimum_custom_format_score: 0,
            order: i32::MAX,
            tags: vec![],
        },
    ])
}

/// GET /api/v3/delayprofile/:id
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
            bypass_if_above_custom_format_score: false,
            minimum_custom_format_score: 0,
            order: i32::MAX,
            tags: vec![],
        }))
    } else {
        Json(None)
    }
}

/// POST /api/v3/delayprofile
pub async fn create_delay_profile(
    Json(body): Json<DelayProfileResource>,
) -> Json<DelayProfileResource> {
    Json(body)
}

/// PUT /api/v3/delayprofile/:id
pub async fn update_delay_profile(
    Path(id): Path<i32>,
    Json(mut body): Json<DelayProfileResource>,
) -> Json<DelayProfileResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/delayprofile/:id
pub async fn delete_delay_profile(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// PUT /api/v3/delayprofile/reorder/:id
pub async fn reorder_delay_profile(
    Path(id): Path<i32>,
) -> Json<Vec<DelayProfileResource>> {
    let _ = id;
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_delay_profiles).post(create_delay_profile))
        .route(
            "/{id}",
            get(get_delay_profile)
                .put(update_delay_profile)
                .delete(delete_delay_profile),
        )
        .route("/reorder/{id}", put(reorder_delay_profile))
}
