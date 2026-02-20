//! Quality API endpoints (v5)

use axum::{
    extract::Path,
    response::Json,
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityDefinitionResource {
    pub id: i32,
    pub quality: QualityResource,
    pub title: String,
    pub weight: i32,
    pub min_size: Option<f64>,
    pub max_size: Option<f64>,
    pub preferred_size: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

fn get_default_quality_definitions() -> Vec<QualityDefinitionResource> {
    vec![
        QualityDefinitionResource {
            id: 1,
            quality: QualityResource {
                id: 0,
                name: "Unknown".to_string(),
                source: "unknown".to_string(),
                resolution: 0,
            },
            title: "Unknown".to_string(),
            weight: 1,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: Some(95.0),
        },
        QualityDefinitionResource {
            id: 2,
            quality: QualityResource {
                id: 1,
                name: "SDTV".to_string(),
                source: "television".to_string(),
                resolution: 480,
            },
            title: "SDTV".to_string(),
            weight: 2,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: Some(95.0),
        },
        QualityDefinitionResource {
            id: 4,
            quality: QualityResource {
                id: 4,
                name: "HDTV-720p".to_string(),
                source: "television".to_string(),
                resolution: 720,
            },
            title: "HDTV-720p".to_string(),
            weight: 4,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: Some(95.0),
        },
        QualityDefinitionResource {
            id: 5,
            quality: QualityResource {
                id: 9,
                name: "HDTV-1080p".to_string(),
                source: "television".to_string(),
                resolution: 1080,
            },
            title: "HDTV-1080p".to_string(),
            weight: 5,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: Some(95.0),
        },
        QualityDefinitionResource {
            id: 10,
            quality: QualityResource {
                id: 16,
                name: "HDTV-2160p".to_string(),
                source: "television".to_string(),
                resolution: 2160,
            },
            title: "HDTV-2160p".to_string(),
            weight: 10,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: Some(95.0),
        },
    ]
}

pub async fn get_quality_definitions() -> Json<Vec<QualityDefinitionResource>> {
    Json(get_default_quality_definitions())
}

pub async fn get_quality_definition(
    Path(id): Path<i32>,
) -> Json<Option<QualityDefinitionResource>> {
    let definitions = get_default_quality_definitions();
    Json(definitions.into_iter().find(|d| d.id == id))
}

pub async fn update_quality_definition(
    Path(id): Path<i32>,
    Json(mut body): Json<QualityDefinitionResource>,
) -> Json<QualityDefinitionResource> {
    body.id = id;
    Json(body)
}

pub async fn update_quality_definitions(
    Json(body): Json<Vec<QualityDefinitionResource>>,
) -> Json<Vec<QualityDefinitionResource>> {
    Json(body)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_quality_definitions))
        .route(
            "/{id}",
            get(get_quality_definition).put(update_quality_definition),
        )
        .route("/update", put(update_quality_definitions))
}
