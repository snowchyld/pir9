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
    // Return the available condition types for custom format specifications
    // These match Sonarr's custom format schema definitions
    Json(vec![
        serde_json::json!({
            "name": "ReleaseTitleSpecification",
            "implementation": "ReleaseTitleSpecification",
            "implementationName": "Release Title",
            "infoLink": "https://wiki.servarr.com/sonarr/settings#custom-formats",
            "negate": false,
            "required": false,
            "fields": [{
                "order": 0,
                "name": "value",
                "label": "Regular Expression",
                "type": "textbox",
                "advanced": false,
                "helpText": "Custom format made when this regex matches the release title",
            }]
        }),
        serde_json::json!({
            "name": "QualityModifierSpecification",
            "implementation": "QualityModifierSpecification",
            "implementationName": "Quality Modifier",
            "infoLink": "https://wiki.servarr.com/sonarr/settings#custom-formats",
            "negate": false,
            "required": false,
            "fields": [{
                "order": 0,
                "name": "value",
                "label": "Quality Modifier",
                "type": "select",
                "advanced": false,
                "selectOptions": [
                    {"value": 0, "name": "None"},
                    {"value": 1, "name": "Regional"},
                    {"value": 2, "name": "Screener"},
                    {"value": 3, "name": "RAWHD"},
                    {"value": 4, "name": "BRDISK"},
                    {"value": 5, "name": "REMUX"}
                ]
            }]
        }),
        serde_json::json!({
            "name": "SizeSpecification",
            "implementation": "SizeSpecification",
            "implementationName": "Size",
            "infoLink": "https://wiki.servarr.com/sonarr/settings#custom-formats",
            "negate": false,
            "required": false,
            "fields": [
                {"order": 0, "name": "min", "label": "Minimum Size (MB)", "type": "number", "advanced": false},
                {"order": 1, "name": "max", "label": "Maximum Size (MB)", "type": "number", "advanced": false}
            ]
        }),
        serde_json::json!({
            "name": "LanguageSpecification",
            "implementation": "LanguageSpecification",
            "implementationName": "Language",
            "infoLink": "https://wiki.servarr.com/sonarr/settings#custom-formats",
            "negate": false,
            "required": false,
            "fields": [{
                "order": 0,
                "name": "value",
                "label": "Language",
                "type": "select",
                "advanced": false,
                "selectOptions": [
                    {"value": 1, "name": "English"},
                    {"value": 2, "name": "French"},
                    {"value": 3, "name": "Spanish"},
                    {"value": 4, "name": "German"},
                    {"value": 5, "name": "Italian"},
                    {"value": 6, "name": "Danish"},
                    {"value": 7, "name": "Dutch"},
                    {"value": 8, "name": "Japanese"},
                    {"value": 9, "name": "Icelandic"},
                    {"value": 10, "name": "Chinese"},
                    {"value": 11, "name": "Russian"},
                    {"value": 12, "name": "Polish"},
                    {"value": 13, "name": "Vietnamese"},
                    {"value": 14, "name": "Swedish"},
                    {"value": 15, "name": "Norwegian"},
                    {"value": 16, "name": "Finnish"},
                    {"value": 17, "name": "Turkish"},
                    {"value": 18, "name": "Portuguese"},
                    {"value": 19, "name": "Flemish"},
                    {"value": 20, "name": "Greek"},
                    {"value": 21, "name": "Korean"},
                    {"value": 22, "name": "Hungarian"},
                    {"value": 23, "name": "Hebrew"},
                    {"value": 24, "name": "Lithuanian"},
                    {"value": 25, "name": "Czech"},
                    {"value": 26, "name": "Hindi"},
                    {"value": 27, "name": "Romanian"},
                    {"value": 28, "name": "Thai"},
                    {"value": 29, "name": "Bulgarian"}
                ]
            }]
        }),
        serde_json::json!({
            "name": "IndexerFlagSpecification",
            "implementation": "IndexerFlagSpecification",
            "implementationName": "Indexer Flag",
            "infoLink": "https://wiki.servarr.com/sonarr/settings#custom-formats",
            "negate": false,
            "required": false,
            "fields": [{
                "order": 0,
                "name": "value",
                "label": "Flag",
                "type": "select",
                "advanced": false,
                "selectOptions": [
                    {"value": 1, "name": "Freeleech"},
                    {"value": 2, "name": "Halfleech"},
                    {"value": 4, "name": "DoubleUpload"},
                    {"value": 8, "name": "Internal"},
                    {"value": 16, "name": "Scene"},
                    {"value": 32, "name": "Freeleech75"},
                    {"value": 64, "name": "Freeleech25"}
                ]
            }]
        }),
        serde_json::json!({
            "name": "SourceSpecification",
            "implementation": "SourceSpecification",
            "implementationName": "Source",
            "infoLink": "https://wiki.servarr.com/sonarr/settings#custom-formats",
            "negate": false,
            "required": false,
            "fields": [{
                "order": 0,
                "name": "value",
                "label": "Source",
                "type": "select",
                "advanced": false,
                "selectOptions": [
                    {"value": 1, "name": "Television"},
                    {"value": 2, "name": "TelevisionRaw"},
                    {"value": 3, "name": "Web"},
                    {"value": 4, "name": "WebRip"},
                    {"value": 5, "name": "DVD"},
                    {"value": 6, "name": "Bluray"},
                    {"value": 7, "name": "BlurayRaw"}
                ]
            }]
        }),
        serde_json::json!({
            "name": "ResolutionSpecification",
            "implementation": "ResolutionSpecification",
            "implementationName": "Resolution",
            "infoLink": "https://wiki.servarr.com/sonarr/settings#custom-formats",
            "negate": false,
            "required": false,
            "fields": [{
                "order": 0,
                "name": "value",
                "label": "Resolution",
                "type": "select",
                "advanced": false,
                "selectOptions": [
                    {"value": 360, "name": "360p"},
                    {"value": 480, "name": "480p"},
                    {"value": 540, "name": "540p"},
                    {"value": 576, "name": "576p"},
                    {"value": 720, "name": "720p"},
                    {"value": 1080, "name": "1080p"},
                    {"value": 2160, "name": "2160p"}
                ]
            }]
        }),
        serde_json::json!({
            "name": "ReleaseGroupSpecification",
            "implementation": "ReleaseGroupSpecification",
            "implementationName": "Release Group",
            "infoLink": "https://wiki.servarr.com/sonarr/settings#custom-formats",
            "negate": false,
            "required": false,
            "fields": [{
                "order": 0,
                "name": "value",
                "label": "Regular Expression",
                "type": "textbox",
                "advanced": false,
                "helpText": "Custom format made when release group matches regex",
            }]
        }),
    ])
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
