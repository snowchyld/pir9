//! Localization API endpoint

use axum::{response::Json, routing::get, Router};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};

use crate::web::AppState;

/// English localization strings (loaded at compile time for now)
static EN_STRINGS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let json_str = include_str!("../localization/en.json");
    serde_json::from_str(json_str).unwrap_or_default()
});

#[derive(Debug, Serialize)]
pub struct LocalizationResource {
    pub strings: HashMap<String, String>,
}

/// GET /api/v3/localization
pub async fn get_localization() -> Json<LocalizationResource> {
    Json(LocalizationResource {
        strings: EN_STRINGS.clone(),
    })
}

/// GET /api/v3/localization/language
#[derive(Debug, Serialize)]
pub struct LocalizationLanguageResource {
    pub identifier: String,
}

pub async fn get_language() -> Json<LocalizationLanguageResource> {
    Json(LocalizationLanguageResource {
        identifier: "en".to_string(),
    })
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_localization))
        .route("/language", get(get_language))
}
