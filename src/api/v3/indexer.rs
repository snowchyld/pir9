//! Indexer API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::IndexerDbModel;
use crate::core::datastore::repositories::IndexerRepository;
use crate::core::indexers::clients::create_client_from_model;
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndexerResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    pub fields: Vec<FieldResource>,
    pub implementation_name: String,
    pub implementation: String,
    pub config_contract: String,
    pub info_link: Option<String>,
    pub message: Option<ProviderMessage>,
    #[serde(default)]
    pub tags: Vec<i32>,
    #[serde(default)]
    pub presets: Vec<serde_json::Value>,
    #[serde(default = "default_true")]
    pub enable_rss: bool,
    #[serde(default = "default_true")]
    pub enable_automatic_search: bool,
    #[serde(default = "default_true")]
    pub enable_interactive_search: bool,
    #[serde(default = "default_true")]
    pub supports_rss: bool,
    #[serde(default = "default_true")]
    pub supports_search: bool,
    pub protocol: String,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub season_search_maximum_single_episode_age: i32,
    #[serde(default)]
    pub download_client_id: i32,
}

fn default_true() -> bool {
    true
}

fn default_priority() -> i32 {
    25
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
#[serde(rename_all = "camelCase")]
pub struct SelectOption {
    pub value: i32,
    pub name: String,
    pub order: i32,
    pub hint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMessage {
    pub message: String,
    #[serde(rename = "type")]
    pub message_type: String,
}

/// Convert DB model to API resource
fn db_to_resource(model: &IndexerDbModel) -> IndexerResource {
    let settings: serde_json::Value = serde_json::from_str(&model.settings)
        .unwrap_or(serde_json::json!({}));

    let tags: Vec<i32> = serde_json::from_str(&model.tags).unwrap_or_default();

    let fields = settings_to_fields(&model.implementation, &settings);

    let protocol = match model.protocol {
        1 => "usenet",
        2 => "torrent",
        _ => "unknown",
    };

    IndexerResource {
        id: model.id as i32,
        name: model.name.clone(),
        fields,
        implementation_name: model.implementation.clone(),
        implementation: model.implementation.clone(),
        config_contract: model.config_contract.clone(),
        info_link: Some(format!("https://wiki.servarr.com/sonarr/supported#{}", model.implementation.to_lowercase())),
        message: None,
        tags,
        presets: vec![],
        enable_rss: model.enable_rss,
        enable_automatic_search: model.enable_automatic_search,
        enable_interactive_search: model.enable_interactive_search,
        supports_rss: true,
        supports_search: true,
        protocol: protocol.to_string(),
        priority: model.priority,
        season_search_maximum_single_episode_age: 0,
        download_client_id: model.download_client_id as i32,
    }
}

/// Convert API resource to DB model
fn resource_to_db(resource: &IndexerResource, id: Option<i64>) -> IndexerDbModel {
    let settings = fields_to_settings(&resource.fields);

    let protocol = match resource.protocol.as_str() {
        "usenet" => 1,
        "torrent" => 2,
        _ => 0,
    };

    IndexerDbModel {
        id: id.unwrap_or(0),
        name: resource.name.clone(),
        implementation: resource.implementation.clone(),
        config_contract: resource.config_contract.clone(),
        enable_rss: resource.enable_rss,
        enable_automatic_search: resource.enable_automatic_search,
        enable_interactive_search: resource.enable_interactive_search,
        protocol,
        priority: resource.priority,
        download_client_id: resource.download_client_id as i64,
        settings: serde_json::to_string(&settings).unwrap_or_default(),
        tags: serde_json::to_string(&resource.tags).unwrap_or_default(),
    }
}

/// Convert settings JSON to fields array
fn settings_to_fields(implementation: &str, settings: &serde_json::Value) -> Vec<FieldResource> {
    let mut fields = Vec::new();

    match implementation {
        "Newznab" | "Torznab" => {
            fields.push(make_field(0, "baseUrl", "URL", "textbox",
                settings.get("baseUrl").or(settings.get("url")).cloned(),
                Some("The URL of the indexer")));
            fields.push(make_field(1, "apiPath", "API Path", "textbox",
                settings.get("apiPath").cloned().or(Some(serde_json::json!("/api"))),
                Some("Path to the API, usually /api")));
            fields.push(make_field(2, "apiKey", "API Key", "textbox",
                settings.get("apiKey").cloned(),
                Some("API key from your indexer")));
            fields.push(make_field(3, "categories", "Categories", "textbox",
                settings.get("categories").cloned().or(Some(serde_json::json!("5000,5010,5020,5030,5040,5045"))),
                Some("Comma-separated list of category IDs")));
            fields.push(make_field(4, "animeCategories", "Anime Categories", "textbox",
                settings.get("animeCategories").cloned().or(Some(serde_json::json!("5070"))),
                Some("Comma-separated list of anime category IDs")));
        }
        _ => {
            // Generic: dump all settings as fields
            if let Some(obj) = settings.as_object() {
                let mut order = 0;
                for (key, value) in obj {
                    fields.push(make_field(order, key, key, "textbox", Some(value.clone()), None));
                    order += 1;
                }
            }
        }
    }

    fields
}

/// Create a field resource
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
        privacy: if name == "apiKey" { Some("apiKey".to_string()) } else { None },
        placeholder: None,
        is_float: None,
    }
}

/// Convert fields array back to settings JSON
fn fields_to_settings(fields: &[FieldResource]) -> serde_json::Value {
    let mut settings = serde_json::Map::new();

    for field in fields {
        if let Some(value) = &field.value {
            settings.insert(field.name.clone(), value.clone());
        }
    }

    serde_json::Value::Object(settings)
}

/// Error type for indexer operations
#[derive(Debug)]
pub enum IndexerError {
    NotFound,
    TestFailed(String),
    Internal(String),
}

impl IntoResponse for IndexerError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            IndexerError::NotFound => (StatusCode::NOT_FOUND, "Indexer not found".to_string()),
            IndexerError::TestFailed(msg) => (StatusCode::BAD_REQUEST, msg),
            IndexerError::Internal(msg) => {
                tracing::error!("Indexer error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

/// GET /api/v3/indexer
pub async fn get_indexers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<IndexerResource>>, IndexerError> {
    let repo = IndexerRepository::new(state.db.clone());

    let indexers = repo.get_all().await
        .map_err(|e| IndexerError::Internal(format!("Failed to fetch indexers: {}", e)))?;

    let resources: Vec<IndexerResource> = indexers.iter()
        .map(db_to_resource)
        .collect();

    Ok(Json(resources))
}

/// GET /api/v3/indexer/:id
pub async fn get_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<IndexerResource>, IndexerError> {
    let repo = IndexerRepository::new(state.db.clone());

    let indexer = repo.get_by_id(id as i64).await
        .map_err(|e| IndexerError::Internal(format!("Failed to fetch indexer: {}", e)))?
        .ok_or(IndexerError::NotFound)?;

    Ok(Json(db_to_resource(&indexer)))
}

/// POST /api/v3/indexer
pub async fn create_indexer(
    State(state): State<Arc<AppState>>,
    Json(body): Json<IndexerResource>,
) -> Result<Json<IndexerResource>, IndexerError> {
    let repo = IndexerRepository::new(state.db.clone());

    let db_model = resource_to_db(&body, None);

    let id = repo.insert(&db_model).await
        .map_err(|e| IndexerError::Internal(format!("Failed to create indexer: {}", e)))?;

    let created = repo.get_by_id(id).await
        .map_err(|e| IndexerError::Internal(format!("Failed to fetch created indexer: {}", e)))?
        .ok_or(IndexerError::Internal("Created indexer not found".to_string()))?;

    tracing::info!("Created indexer: {} ({})", created.name, created.implementation);

    crate::core::logging::log_info(
        "IndexerCreated",
        &format!("Created indexer: {}", created.name)
    ).await;

    Ok(Json(db_to_resource(&created)))
}

/// PUT /api/v3/indexer/:id
pub async fn update_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<IndexerResource>,
) -> Result<Json<IndexerResource>, IndexerError> {
    let repo = IndexerRepository::new(state.db.clone());

    // Verify it exists
    repo.get_by_id(id as i64).await
        .map_err(|e| IndexerError::Internal(format!("Failed to fetch indexer: {}", e)))?
        .ok_or(IndexerError::NotFound)?;

    let db_model = resource_to_db(&body, Some(id as i64));

    repo.update(&db_model).await
        .map_err(|e| IndexerError::Internal(format!("Failed to update indexer: {}", e)))?;

    let updated = repo.get_by_id(id as i64).await
        .map_err(|e| IndexerError::Internal(format!("Failed to fetch updated indexer: {}", e)))?
        .ok_or(IndexerError::Internal("Updated indexer not found".to_string()))?;

    tracing::info!("Updated indexer: {} (id={})", updated.name, id);

    Ok(Json(db_to_resource(&updated)))
}

/// DELETE /api/v3/indexer/:id
pub async fn delete_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, IndexerError> {
    let repo = IndexerRepository::new(state.db.clone());

    let indexer = repo.get_by_id(id as i64).await
        .map_err(|e| IndexerError::Internal(format!("Failed to fetch indexer: {}", e)))?
        .ok_or(IndexerError::NotFound)?;

    repo.delete(id as i64).await
        .map_err(|e| IndexerError::Internal(format!("Failed to delete indexer: {}", e)))?;

    tracing::info!("Deleted indexer: {} (id={})", indexer.name, id);

    crate::core::logging::log_info(
        "IndexerDeleted",
        &format!("Deleted indexer: {}", indexer.name)
    ).await;

    Ok(Json(serde_json::json!({})))
}

/// POST /api/v3/indexer/test
/// Test an indexer connection
pub async fn test_indexer(
    Json(body): Json<IndexerResource>,
) -> Result<Json<serde_json::Value>, IndexerError> {
    let db_model = resource_to_db(&body, None);

    let client = create_client_from_model(&db_model)
        .map_err(|e| IndexerError::TestFailed(format!("Failed to create client: {}", e)))?;

    // Test the connection by fetching capabilities
    let caps = client.test().await
        .map_err(|e| IndexerError::TestFailed(format!("Connection test failed: {}", e)))?;

    tracing::info!(
        "Indexer test successful: {} (tv_search: {}, rss: {})",
        body.name,
        caps.tv_search_available,
        caps.search_available
    );

    Ok(Json(serde_json::json!({
        "isValid": true,
        "message": format!("Successfully connected to {}", body.name)
    })))
}

/// POST /api/v3/indexer/testall
/// Test all configured indexers
pub async fn test_all_indexers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, IndexerError> {
    let repo = IndexerRepository::new(state.db.clone());

    let indexers = repo.get_all().await
        .map_err(|e| IndexerError::Internal(format!("Failed to fetch indexers: {}", e)))?;

    let mut results = Vec::new();

    for db_indexer in &indexers {
        let test_result = match create_client_from_model(db_indexer) {
            Ok(client) => {
                match client.test().await {
                    Ok(_) => serde_json::json!({
                        "id": db_indexer.id,
                        "name": db_indexer.name,
                        "isValid": true
                    }),
                    Err(e) => serde_json::json!({
                        "id": db_indexer.id,
                        "name": db_indexer.name,
                        "isValid": false,
                        "validationFailures": [{
                            "propertyName": "",
                            "errorMessage": e.to_string()
                        }]
                    }),
                }
            }
            Err(e) => serde_json::json!({
                "id": db_indexer.id,
                "name": db_indexer.name,
                "isValid": false,
                "validationFailures": [{
                    "propertyName": "",
                    "errorMessage": e.to_string()
                }]
            }),
        };

        results.push(test_result);
    }

    Ok(Json(results))
}

/// GET /api/v3/indexer/schema
/// Get available indexer types (schemas)
pub async fn get_indexer_schema() -> Json<Vec<IndexerResource>> {
    let schemas = vec![
        create_newznab_schema(),
        create_torznab_schema(),
    ];

    Json(schemas)
}

/// Create schema for Newznab indexer
fn create_newznab_schema() -> IndexerResource {
    IndexerResource {
        id: 0,
        name: "".to_string(),
        fields: vec![
            make_field(0, "baseUrl", "URL", "textbox", None, Some("The URL of the Newznab indexer")),
            make_field(1, "apiPath", "API Path", "textbox", Some(serde_json::json!("/api")), Some("Path to the API")),
            make_field(2, "apiKey", "API Key", "textbox", None, Some("API key from your indexer")),
            make_field(3, "categories", "Categories", "textbox", Some(serde_json::json!("5000,5010,5020,5030,5040,5045")), Some("Comma-separated category IDs")),
            make_field(4, "animeCategories", "Anime Categories", "textbox", Some(serde_json::json!("5070")), Some("Comma-separated anime category IDs")),
            make_field(5, "additionalParameters", "Additional Parameters", "textbox", None, Some("Extra parameters to add to search URL")),
        ],
        implementation_name: "Newznab".to_string(),
        implementation: "Newznab".to_string(),
        config_contract: "NewznabSettings".to_string(),
        info_link: Some("https://wiki.servarr.com/sonarr/supported#newznab".to_string()),
        message: None,
        tags: vec![],
        presets: vec![],
        enable_rss: true,
        enable_automatic_search: true,
        enable_interactive_search: true,
        supports_rss: true,
        supports_search: true,
        protocol: "usenet".to_string(),
        priority: 25,
        season_search_maximum_single_episode_age: 0,
        download_client_id: 0,
    }
}

/// Create schema for Torznab indexer
fn create_torznab_schema() -> IndexerResource {
    IndexerResource {
        id: 0,
        name: "".to_string(),
        fields: vec![
            make_field(0, "baseUrl", "URL", "textbox", None, Some("The URL of the Torznab indexer")),
            make_field(1, "apiPath", "API Path", "textbox", Some(serde_json::json!("/api")), Some("Path to the API")),
            make_field(2, "apiKey", "API Key", "textbox", None, Some("API key from your indexer")),
            make_field(3, "categories", "Categories", "textbox", Some(serde_json::json!("5000,5010,5020,5030,5040,5045")), Some("Comma-separated category IDs")),
            make_field(4, "animeCategories", "Anime Categories", "textbox", Some(serde_json::json!("5070")), Some("Comma-separated anime category IDs")),
            make_field(5, "minimumSeeders", "Minimum Seeders", "textbox", Some(serde_json::json!(1)), Some("Minimum number of seeders")),
            make_field(6, "seedCriteria", "Seed Ratio", "textbox", None, Some("Seed ratio requirements")),
        ],
        implementation_name: "Torznab".to_string(),
        implementation: "Torznab".to_string(),
        config_contract: "TorznabSettings".to_string(),
        info_link: Some("https://wiki.servarr.com/sonarr/supported#torznab".to_string()),
        message: None,
        tags: vec![],
        presets: vec![],
        enable_rss: true,
        enable_automatic_search: true,
        enable_interactive_search: true,
        supports_rss: true,
        supports_search: true,
        protocol: "torrent".to_string(),
        priority: 25,
        season_search_maximum_single_episode_age: 0,
        download_client_id: 0,
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_indexers).post(create_indexer))
        .route(
            "/{id}",
            get(get_indexer).put(update_indexer).delete(delete_indexer),
        )
        .route("/test", post(test_indexer))
        .route("/testall", post(test_all_indexers))
        .route("/schema", get(get_indexer_schema))
}
