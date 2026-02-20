//! Localization API endpoints (v5)

use axum::{response::Json, routing::get, Router};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

use crate::web::AppState;

static EN_STRINGS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let json_str = include_str!("../localization/en.json");
    serde_json::from_str(json_str).unwrap_or_default()
});

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationResource {
    pub strings: HashMap<String, String>,
}

/// GET /api/v5/localization
pub async fn get_localization() -> Json<LocalizationResource> {
    Json(LocalizationResource {
        strings: EN_STRINGS.clone(),
    })
}

/// GET /api/v5/localization/language
pub async fn get_localization_language() -> Json<Vec<LanguageResource>> {
    Json(vec![LanguageResource {
        identifier: "en".to_string(),
        name: "English".to_string(),
    }])
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub identifier: String,
    pub name: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_localization))
        .route("/language", get(get_localization_language))
}
