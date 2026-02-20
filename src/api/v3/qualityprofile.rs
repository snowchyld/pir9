//! Quality Profile API endpoints

use axum::{extract::Path, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityProfileResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub upgrade_allowed: bool,
    #[serde(default = "default_cutoff")]
    pub cutoff: i32,
    #[serde(default)]
    pub items: Vec<QualityProfileItemResource>,
    #[serde(default)]
    pub min_format_score: i32,
    #[serde(default)]
    pub cutoff_format_score: i32,
    #[serde(default)]
    pub format_items: Vec<ProfileFormatItemResource>,
}

fn default_cutoff() -> i32 {
    1
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityProfileItemResource {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub quality: Option<QualityResource>,
    #[serde(default)]
    pub items: Vec<QualityProfileItemResource>,
    #[serde(default)]
    pub allowed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub resolution: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFormatItemResource {
    pub format: i32,
    pub name: String,
    #[serde(default)]
    pub score: i32,
}

/// GET /api/v3/qualityprofile
pub async fn get_quality_profiles() -> Json<Vec<QualityProfileResource>> {
    // Return default quality profiles
    Json(vec![
        create_any_profile(),
        create_sd_profile(),
        create_hd_720p_profile(),
        create_hd_1080p_profile(),
        create_ultra_hd_profile(),
    ])
}

/// GET /api/v3/qualityprofile/:id
pub async fn get_quality_profile(Path(id): Path<i32>) -> Json<Option<QualityProfileResource>> {
    let profiles = vec![
        create_any_profile(),
        create_sd_profile(),
        create_hd_720p_profile(),
        create_hd_1080p_profile(),
        create_ultra_hd_profile(),
    ];
    Json(profiles.into_iter().find(|p| p.id == id))
}

/// POST /api/v3/qualityprofile
pub async fn create_quality_profile(
    Json(mut body): Json<QualityProfileResource>,
) -> Json<QualityProfileResource> {
    body.id = 100; // Mock ID
    Json(body)
}

/// PUT /api/v3/qualityprofile/:id
pub async fn update_quality_profile(
    Path(id): Path<i32>,
    Json(mut body): Json<QualityProfileResource>,
) -> Json<QualityProfileResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/qualityprofile/:id
pub async fn delete_quality_profile(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// GET /api/v3/qualityprofile/schema
pub async fn get_quality_profile_schema() -> Json<QualityProfileResource> {
    Json(QualityProfileResource {
        id: 0,
        name: String::new(),
        upgrade_allowed: true,
        cutoff: 1,
        items: get_all_quality_items(),
        min_format_score: 0,
        cutoff_format_score: 0,
        format_items: vec![],
    })
}

fn get_all_quality_items() -> Vec<QualityProfileItemResource> {
    vec![
        QualityProfileItemResource {
            id: Some(1000),
            name: Some("WEB-DL".to_string()),
            quality: None,
            items: vec![
                quality_item(3, "WEBDL-480p", "web", 480),
                quality_item(4, "WEBDL-720p", "web", 720),
                quality_item(5, "WEBDL-1080p", "web", 1080),
                quality_item(18, "WEBDL-2160p", "web", 2160),
            ],
            allowed: true,
        },
        QualityProfileItemResource {
            id: Some(1001),
            name: Some("WEBRip".to_string()),
            quality: None,
            items: vec![
                quality_item(12, "WEBRip-480p", "webRip", 480),
                quality_item(14, "WEBRip-720p", "webRip", 720),
                quality_item(15, "WEBRip-1080p", "webRip", 1080),
                quality_item(17, "WEBRip-2160p", "webRip", 2160),
            ],
            allowed: true,
        },
        QualityProfileItemResource {
            id: Some(1002),
            name: Some("HDTV".to_string()),
            quality: None,
            items: vec![
                quality_item(4, "HDTV-720p", "television", 720),
                quality_item(9, "HDTV-1080p", "television", 1080),
                quality_item(16, "HDTV-2160p", "television", 2160),
            ],
            allowed: true,
        },
        QualityProfileItemResource {
            id: Some(1003),
            name: Some("Bluray".to_string()),
            quality: None,
            items: vec![
                quality_item(6, "Bluray-720p", "bluray", 720),
                quality_item(7, "Bluray-1080p", "bluray", 1080),
                quality_item(19, "Bluray-2160p", "bluray", 2160),
            ],
            allowed: true,
        },
        quality_item(1, "SDTV", "television", 480),
        quality_item(8, "DVD", "dvd", 480),
        quality_item(2, "DVD-R", "dvd", 480),
        quality_item(10, "Raw-HD", "televisionRaw", 1080),
    ]
}

fn quality_item(id: i32, name: &str, source: &str, resolution: i32) -> QualityProfileItemResource {
    QualityProfileItemResource {
        id: None,
        name: None,
        quality: Some(QualityResource {
            id,
            name: name.to_string(),
            source: source.to_string(),
            resolution,
        }),
        items: vec![],
        allowed: true,
    }
}

fn create_any_profile() -> QualityProfileResource {
    QualityProfileResource {
        id: 1,
        name: "Any".to_string(),
        upgrade_allowed: true,
        cutoff: 19,
        items: get_all_quality_items(),
        min_format_score: 0,
        cutoff_format_score: 0,
        format_items: vec![],
    }
}

fn create_sd_profile() -> QualityProfileResource {
    QualityProfileResource {
        id: 2,
        name: "SD".to_string(),
        upgrade_allowed: true,
        cutoff: 1,
        items: vec![
            quality_item(1, "SDTV", "television", 480),
            quality_item(8, "DVD", "dvd", 480),
        ],
        min_format_score: 0,
        cutoff_format_score: 0,
        format_items: vec![],
    }
}

fn create_hd_720p_profile() -> QualityProfileResource {
    QualityProfileResource {
        id: 3,
        name: "HD-720p".to_string(),
        upgrade_allowed: true,
        cutoff: 4,
        items: vec![
            quality_item(4, "HDTV-720p", "television", 720),
            quality_item(5, "WEBDL-720p", "web", 720),
            quality_item(14, "WEBRip-720p", "webRip", 720),
            quality_item(6, "Bluray-720p", "bluray", 720),
        ],
        min_format_score: 0,
        cutoff_format_score: 0,
        format_items: vec![],
    }
}

fn create_hd_1080p_profile() -> QualityProfileResource {
    QualityProfileResource {
        id: 4,
        name: "HD-1080p".to_string(),
        upgrade_allowed: true,
        cutoff: 9,
        items: vec![
            quality_item(9, "HDTV-1080p", "television", 1080),
            quality_item(5, "WEBDL-1080p", "web", 1080),
            quality_item(15, "WEBRip-1080p", "webRip", 1080),
            quality_item(7, "Bluray-1080p", "bluray", 1080),
        ],
        min_format_score: 0,
        cutoff_format_score: 0,
        format_items: vec![],
    }
}

fn create_ultra_hd_profile() -> QualityProfileResource {
    QualityProfileResource {
        id: 5,
        name: "Ultra-HD".to_string(),
        upgrade_allowed: true,
        cutoff: 19,
        items: vec![
            quality_item(16, "HDTV-2160p", "television", 2160),
            quality_item(18, "WEBDL-2160p", "web", 2160),
            quality_item(17, "WEBRip-2160p", "webRip", 2160),
            quality_item(19, "Bluray-2160p", "bluray", 2160),
        ],
        min_format_score: 0,
        cutoff_format_score: 0,
        format_items: vec![],
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_quality_profiles).post(create_quality_profile))
        .route(
            "/{id}",
            get(get_quality_profile)
                .put(update_quality_profile)
                .delete(delete_quality_profile),
        )
        .route("/schema", get(get_quality_profile_schema))
}
