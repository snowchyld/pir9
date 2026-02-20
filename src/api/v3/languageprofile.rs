//! Language Profile API endpoints (legacy, for backwards compatibility)

use axum::{extract::Path, response::Json, routing::get, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageProfileResource {
    pub id: i32,
    pub name: String,
    pub upgrade_allowed: bool,
    pub cutoff: LanguageResource,
    pub languages: Vec<LanguageProfileItemResource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageProfileItemResource {
    pub language: LanguageResource,
    pub allowed: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

/// GET /api/v3/languageprofile
pub async fn get_language_profiles() -> Json<Vec<LanguageProfileResource>> {
    Json(vec![
        LanguageProfileResource {
            id: 1,
            name: "English".to_string(),
            upgrade_allowed: true,
            cutoff: LanguageResource { id: 1, name: "English".to_string() },
            languages: vec![
                LanguageProfileItemResource {
                    language: LanguageResource { id: 1, name: "English".to_string() },
                    allowed: true,
                },
            ],
        },
    ])
}

/// GET /api/v3/languageprofile/:id
pub async fn get_language_profile(Path(id): Path<i32>) -> Json<Option<LanguageProfileResource>> {
    if id == 1 {
        Json(Some(LanguageProfileResource {
            id: 1,
            name: "English".to_string(),
            upgrade_allowed: true,
            cutoff: LanguageResource { id: 1, name: "English".to_string() },
            languages: vec![
                LanguageProfileItemResource {
                    language: LanguageResource { id: 1, name: "English".to_string() },
                    allowed: true,
                },
            ],
        }))
    } else {
        Json(None)
    }
}

/// GET /api/v3/languageprofile/schema
pub async fn get_language_profile_schema() -> Json<LanguageProfileResource> {
    Json(LanguageProfileResource {
        id: 0,
        name: "".to_string(),
        upgrade_allowed: true,
        cutoff: LanguageResource { id: 1, name: "English".to_string() },
        languages: vec![
            LanguageProfileItemResource {
                language: LanguageResource { id: 1, name: "English".to_string() },
                allowed: true,
            },
        ],
    })
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_language_profiles))
        .route("/{id}", get(get_language_profile))
        .route("/schema", get(get_language_profile_schema))
}
