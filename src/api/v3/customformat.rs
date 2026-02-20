//! Custom Format API endpoints

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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomFormatResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub include_custom_format_when_renaming: bool,
    #[serde(default)]
    pub specifications: Vec<CustomFormatSpecificationResource>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomFormatSpecificationResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    pub implementation: String,
    #[serde(default)]
    pub implementation_name: String,
    pub info_link: Option<String>,
    #[serde(default)]
    pub negate: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub fields: Vec<FieldResource>,
    #[serde(default)]
    pub presets: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FieldResource {
    #[serde(default)]
    pub order: i32,
    pub name: String,
    #[serde(default)]
    pub label: String,
    pub unit: Option<String>,
    pub help_text: Option<String>,
    pub help_text_warning: Option<String>,
    pub help_link: Option<String>,
    pub value: Option<serde_json::Value>,
    #[serde(rename = "type", default = "default_field_type")]
    pub field_type: String,
    #[serde(default)]
    pub advanced: bool,
    pub select_options: Option<Vec<SelectOption>>,
    pub select_options_provider_action: Option<String>,
    pub section: Option<String>,
    pub hidden: Option<String>,
    pub privacy: Option<String>,
    pub placeholder: Option<String>,
    pub is_float: Option<bool>,
}

fn default_field_type() -> String {
    "textbox".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SelectOption {
    pub value: serde_json::Value,
    pub name: String,
    pub order: i32,
    pub hint: Option<String>,
}

fn db_to_resource(model: &CustomFormatDbModel) -> CustomFormatResource {
    let specifications: Vec<CustomFormatSpecificationResource> =
        serde_json::from_str(&model.specifications).unwrap_or_default();
    CustomFormatResource {
        id: model.id as i32,
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

/// GET /api/v3/customformat
pub async fn get_custom_formats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CustomFormatResource>>, StatusCode> {
    let repo = CustomFormatRepository::new(state.db.clone());
    let items = repo.get_all().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(items.iter().map(db_to_resource).collect()))
}

/// GET /api/v3/customformat/:id
pub async fn get_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<CustomFormatResource>, StatusCode> {
    let repo = CustomFormatRepository::new(state.db.clone());
    let item = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(db_to_resource(&item)))
}

/// POST /api/v3/customformat
pub async fn create_custom_format(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CustomFormatResource>,
) -> Result<impl IntoResponse, StatusCode> {
    let repo = CustomFormatRepository::new(state.db.clone());
    let model = resource_to_db(&body, None);
    let id = repo.insert(&model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let created = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(db_to_resource(&created))))
}

/// PUT /api/v3/customformat/:id
pub async fn update_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<CustomFormatResource>,
) -> Result<Json<CustomFormatResource>, StatusCode> {
    let repo = CustomFormatRepository::new(state.db.clone());
    let _existing = repo.get_by_id(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let model = resource_to_db(&body, Some(id));
    repo.update(&model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(db_to_resource(&model)))
}

/// DELETE /api/v3/customformat/:id
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

/// GET /api/v3/customformat/schema
pub async fn get_custom_format_schema() -> Json<Vec<CustomFormatSpecificationResource>> {
    Json(vec![
        create_release_title_spec(),
        create_release_group_spec(),
        create_edition_spec(),
        create_language_spec(),
        create_indexer_flag_spec(),
        create_source_spec(),
        create_resolution_spec(),
        create_quality_modifier_spec(),
        create_size_spec(),
    ])
}

fn make_field(order: i32, name: &str, label: &str, field_type: &str, value: Option<serde_json::Value>, help_text: Option<&str>) -> FieldResource {
    FieldResource {
        order,
        name: name.to_string(),
        label: label.to_string(),
        unit: None,
        help_text: help_text.map(|s| s.to_string()),
        help_text_warning: None,
        help_link: None,
        value,
        field_type: field_type.to_string(),
        advanced: false,
        select_options: None,
        select_options_provider_action: None,
        section: None,
        hidden: None,
        privacy: None,
        placeholder: None,
        is_float: None,
    }
}

fn create_release_title_spec() -> CustomFormatSpecificationResource {
    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "ReleaseTitleSpecification".to_string(),
        implementation_name: "Release Title".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![
            make_field(0, "value", "Regular Expression", "textbox", None, Some("Custom Format RegEx is Case Insensitive")),
        ],
        presets: vec![],
    }
}

fn create_release_group_spec() -> CustomFormatSpecificationResource {
    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "ReleaseGroupSpecification".to_string(),
        implementation_name: "Release Group".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![
            make_field(0, "value", "Regular Expression", "textbox", None, Some("Custom Format RegEx is Case Insensitive")),
        ],
        presets: vec![],
    }
}

fn create_edition_spec() -> CustomFormatSpecificationResource {
    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "EditionSpecification".to_string(),
        implementation_name: "Edition".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![
            make_field(0, "value", "Regular Expression", "textbox", None, Some("Custom Format RegEx is Case Insensitive")),
        ],
        presets: vec![],
    }
}

fn create_language_spec() -> CustomFormatSpecificationResource {
    let mut field = make_field(0, "value", "Language", "select", Some(serde_json::json!(1)), None);
    field.select_options = Some(vec![
        SelectOption { value: serde_json::json!(1), name: "English".to_string(), order: 0, hint: None },
        SelectOption { value: serde_json::json!(2), name: "French".to_string(), order: 1, hint: None },
        SelectOption { value: serde_json::json!(3), name: "Spanish".to_string(), order: 2, hint: None },
        SelectOption { value: serde_json::json!(4), name: "German".to_string(), order: 3, hint: None },
        SelectOption { value: serde_json::json!(5), name: "Italian".to_string(), order: 4, hint: None },
        SelectOption { value: serde_json::json!(6), name: "Danish".to_string(), order: 5, hint: None },
        SelectOption { value: serde_json::json!(7), name: "Dutch".to_string(), order: 6, hint: None },
        SelectOption { value: serde_json::json!(8), name: "Japanese".to_string(), order: 7, hint: None },
        SelectOption { value: serde_json::json!(9), name: "Icelandic".to_string(), order: 8, hint: None },
        SelectOption { value: serde_json::json!(10), name: "Chinese".to_string(), order: 9, hint: None },
        SelectOption { value: serde_json::json!(11), name: "Russian".to_string(), order: 10, hint: None },
        SelectOption { value: serde_json::json!(12), name: "Polish".to_string(), order: 11, hint: None },
        SelectOption { value: serde_json::json!(13), name: "Vietnamese".to_string(), order: 12, hint: None },
        SelectOption { value: serde_json::json!(14), name: "Swedish".to_string(), order: 13, hint: None },
        SelectOption { value: serde_json::json!(15), name: "Norwegian".to_string(), order: 14, hint: None },
        SelectOption { value: serde_json::json!(16), name: "Finnish".to_string(), order: 15, hint: None },
        SelectOption { value: serde_json::json!(17), name: "Turkish".to_string(), order: 16, hint: None },
        SelectOption { value: serde_json::json!(18), name: "Portuguese".to_string(), order: 17, hint: None },
        SelectOption { value: serde_json::json!(19), name: "Flemish".to_string(), order: 18, hint: None },
        SelectOption { value: serde_json::json!(20), name: "Greek".to_string(), order: 19, hint: None },
        SelectOption { value: serde_json::json!(21), name: "Korean".to_string(), order: 20, hint: None },
        SelectOption { value: serde_json::json!(22), name: "Hungarian".to_string(), order: 21, hint: None },
        SelectOption { value: serde_json::json!(23), name: "Hebrew".to_string(), order: 22, hint: None },
        SelectOption { value: serde_json::json!(24), name: "Lithuanian".to_string(), order: 23, hint: None },
        SelectOption { value: serde_json::json!(25), name: "Czech".to_string(), order: 24, hint: None },
        SelectOption { value: serde_json::json!(26), name: "Hindi".to_string(), order: 25, hint: None },
        SelectOption { value: serde_json::json!(27), name: "Romanian".to_string(), order: 26, hint: None },
        SelectOption { value: serde_json::json!(28), name: "Thai".to_string(), order: 27, hint: None },
        SelectOption { value: serde_json::json!(29), name: "Bulgarian".to_string(), order: 28, hint: None },
        SelectOption { value: serde_json::json!(0), name: "Original".to_string(), order: 29, hint: None },
        SelectOption { value: serde_json::json!(-2), name: "Any".to_string(), order: 30, hint: None },
    ]);

    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "LanguageSpecification".to_string(),
        implementation_name: "Language".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![field],
        presets: vec![],
    }
}

fn create_indexer_flag_spec() -> CustomFormatSpecificationResource {
    let mut field = make_field(0, "value", "Flag", "select", Some(serde_json::json!(1)), None);
    field.select_options = Some(vec![
        SelectOption { value: serde_json::json!(1), name: "Freeleech".to_string(), order: 0, hint: None },
        SelectOption { value: serde_json::json!(2), name: "Halfleech".to_string(), order: 1, hint: None },
        SelectOption { value: serde_json::json!(4), name: "DoubleUpload".to_string(), order: 2, hint: None },
        SelectOption { value: serde_json::json!(8), name: "Internal".to_string(), order: 3, hint: None },
        SelectOption { value: serde_json::json!(16), name: "Scene".to_string(), order: 4, hint: None },
        SelectOption { value: serde_json::json!(32), name: "PTP Golden Popcorn".to_string(), order: 5, hint: None },
        SelectOption { value: serde_json::json!(64), name: "PTP Approved".to_string(), order: 6, hint: None },
    ]);

    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "IndexerFlagSpecification".to_string(),
        implementation_name: "Indexer Flag".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![field],
        presets: vec![],
    }
}

fn create_source_spec() -> CustomFormatSpecificationResource {
    let mut field = make_field(0, "value", "Source", "select", Some(serde_json::json!(1)), None);
    field.select_options = Some(vec![
        SelectOption { value: serde_json::json!(0), name: "Unknown".to_string(), order: 0, hint: None },
        SelectOption { value: serde_json::json!(1), name: "Television".to_string(), order: 1, hint: None },
        SelectOption { value: serde_json::json!(2), name: "TelevisionRaw".to_string(), order: 2, hint: None },
        SelectOption { value: serde_json::json!(3), name: "Web".to_string(), order: 3, hint: None },
        SelectOption { value: serde_json::json!(4), name: "WebRip".to_string(), order: 4, hint: None },
        SelectOption { value: serde_json::json!(5), name: "DVD".to_string(), order: 5, hint: None },
        SelectOption { value: serde_json::json!(6), name: "Bluray".to_string(), order: 6, hint: None },
        SelectOption { value: serde_json::json!(7), name: "BlurayRaw".to_string(), order: 7, hint: None },
    ]);

    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "SourceSpecification".to_string(),
        implementation_name: "Source".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![field],
        presets: vec![],
    }
}

fn create_resolution_spec() -> CustomFormatSpecificationResource {
    let mut field = make_field(0, "value", "Resolution", "select", Some(serde_json::json!(1080)), None);
    field.select_options = Some(vec![
        SelectOption { value: serde_json::json!(0), name: "Unknown".to_string(), order: 0, hint: None },
        SelectOption { value: serde_json::json!(360), name: "360p".to_string(), order: 1, hint: None },
        SelectOption { value: serde_json::json!(480), name: "480p".to_string(), order: 2, hint: None },
        SelectOption { value: serde_json::json!(540), name: "540p".to_string(), order: 3, hint: None },
        SelectOption { value: serde_json::json!(576), name: "576p".to_string(), order: 4, hint: None },
        SelectOption { value: serde_json::json!(720), name: "720p".to_string(), order: 5, hint: None },
        SelectOption { value: serde_json::json!(1080), name: "1080p".to_string(), order: 6, hint: None },
        SelectOption { value: serde_json::json!(2160), name: "2160p".to_string(), order: 7, hint: None },
    ]);

    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "ResolutionSpecification".to_string(),
        implementation_name: "Resolution".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![field],
        presets: vec![],
    }
}

fn create_quality_modifier_spec() -> CustomFormatSpecificationResource {
    let mut field = make_field(0, "value", "Modifier", "select", Some(serde_json::json!(1)), None);
    field.select_options = Some(vec![
        SelectOption { value: serde_json::json!(0), name: "None".to_string(), order: 0, hint: None },
        SelectOption { value: serde_json::json!(1), name: "Regional".to_string(), order: 1, hint: None },
        SelectOption { value: serde_json::json!(2), name: "Screener".to_string(), order: 2, hint: None },
        SelectOption { value: serde_json::json!(3), name: "RAWHD".to_string(), order: 3, hint: None },
        SelectOption { value: serde_json::json!(4), name: "BRDISK".to_string(), order: 4, hint: None },
        SelectOption { value: serde_json::json!(5), name: "REMUX".to_string(), order: 5, hint: None },
    ]);

    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "QualityModifierSpecification".to_string(),
        implementation_name: "Quality Modifier".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![field],
        presets: vec![],
    }
}

fn create_size_spec() -> CustomFormatSpecificationResource {
    CustomFormatSpecificationResource {
        id: 0,
        name: String::new(),
        implementation: "SizeSpecification".to_string(),
        implementation_name: "Size".to_string(),
        info_link: None,
        negate: false,
        required: false,
        fields: vec![
            make_field(0, "min", "Minimum Size (GB)", "number", Some(serde_json::json!(0)), None),
            make_field(1, "max", "Maximum Size (GB)", "number", Some(serde_json::json!(100)), None),
        ],
        presets: vec![],
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
