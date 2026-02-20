//! Download Client API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::DownloadClientDbModel;
use crate::core::datastore::repositories::DownloadClientRepository;
use crate::core::download::clients::create_client_from_model;
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DownloadClientResource {
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
    pub enable: bool,
    pub protocol: String,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default = "default_true")]
    pub remove_completed_downloads: bool,
    #[serde(default = "default_true")]
    pub remove_failed_downloads: bool,
}

fn default_true() -> bool {
    true
}

fn default_priority() -> i32 {
    1
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
fn db_to_resource(model: &DownloadClientDbModel) -> DownloadClientResource {
    // Parse settings JSON to extract fields
    let settings: serde_json::Value =
        serde_json::from_str(&model.settings).unwrap_or(serde_json::json!({}));

    let tags: Vec<i32> = serde_json::from_str(&model.tags).unwrap_or_default();

    // Convert settings to fields array
    let fields = settings_to_fields(&model.implementation, &settings);

    let protocol = match model.protocol {
        1 => "usenet",
        2 => "torrent",
        _ => "unknown",
    };

    DownloadClientResource {
        id: model.id as i32,
        name: model.name.clone(),
        fields,
        implementation_name: model.implementation.clone(),
        implementation: model.implementation.clone(),
        config_contract: model.config_contract.clone(),
        info_link: None,
        message: None,
        tags,
        presets: vec![],
        enable: model.enable,
        protocol: protocol.to_string(),
        priority: model.priority,
        remove_completed_downloads: true,
        remove_failed_downloads: true,
    }
}

/// Convert API resource to DB model
fn resource_to_db(resource: &DownloadClientResource, id: Option<i64>) -> DownloadClientDbModel {
    // Convert fields array back to settings JSON
    let settings = fields_to_settings(&resource.fields);

    let protocol = match resource.protocol.as_str() {
        "usenet" => 1,
        "torrent" => 2,
        _ => 0,
    };

    DownloadClientDbModel {
        id: id.unwrap_or(0),
        enable: resource.enable,
        protocol,
        priority: resource.priority,
        name: resource.name.clone(),
        implementation: resource.implementation.clone(),
        config_contract: resource.config_contract.clone(),
        settings: serde_json::to_string(&settings).unwrap_or_default(),
        tags: serde_json::to_string(&resource.tags).unwrap_or_default(),
    }
}

/// Convert settings JSON to fields array for API response
fn settings_to_fields(implementation: &str, settings: &serde_json::Value) -> Vec<FieldResource> {
    let mut fields = Vec::new();
    let mut order = 0;

    // Common fields based on implementation type
    match implementation {
        "QBittorrent" => {
            fields.push(make_field(
                order,
                "host",
                "Host",
                "textbox",
                settings.get("host").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "port",
                "Port",
                "textbox",
                settings.get("port").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "useSsl",
                "Use SSL",
                "checkbox",
                settings.get("useSsl").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "urlBase",
                "URL Base",
                "textbox",
                settings.get("urlBase").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "username",
                "Username",
                "textbox",
                settings.get("username").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "password",
                "Password",
                "password",
                settings.get("password").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "category",
                "Series Category",
                "textbox",
                settings.get("category").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "movieCategory",
                "Movie Category",
                "textbox",
                settings.get("movieCategory").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "animeCategory",
                "Anime Category",
                "textbox",
                settings.get("animeCategory").cloned(),
            ));
        }
        "Sabnzbd" | "SABnzbd" => {
            fields.push(make_field(
                order,
                "host",
                "Host",
                "textbox",
                settings.get("host").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "port",
                "Port",
                "textbox",
                settings.get("port").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "useSsl",
                "Use SSL",
                "checkbox",
                settings.get("useSsl").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "urlBase",
                "URL Base",
                "textbox",
                settings.get("urlBase").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "apiKey",
                "API Key",
                "textbox",
                settings.get("apiKey").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "category",
                "Series Category",
                "textbox",
                settings.get("category").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "movieCategory",
                "Movie Category",
                "textbox",
                settings.get("movieCategory").cloned(),
            ));
            order += 1;
            fields.push(make_field(
                order,
                "animeCategory",
                "Anime Category",
                "textbox",
                settings.get("animeCategory").cloned(),
            ));
        }
        _ => {
            // Generic: just dump all settings as fields
            if let Some(obj) = settings.as_object() {
                for (key, value) in obj {
                    fields.push(make_field(order, key, key, "textbox", Some(value.clone())));
                    order += 1;
                }
            }
        }
    }

    fields
}

/// Create a field resource
fn make_field(
    order: i32,
    name: &str,
    label: &str,
    field_type: &str,
    value: Option<serde_json::Value>,
) -> FieldResource {
    FieldResource {
        order,
        name: name.to_string(),
        label: label.to_string(),
        unit: None,
        help_text: None,
        help_text_warning: None,
        help_link: None,
        value,
        field_type: field_type.to_string(),
        advanced: false,
        select_options: None,
        select_options_provider_action: None,
        section: None,
        hidden: None,
        privacy: if field_type == "password" {
            Some("password".to_string())
        } else {
            None
        },
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

/// Error type for download client operations
#[derive(Debug)]
pub enum DownloadClientError {
    NotFound,
    TestFailed(String),
    Internal(String),
}

impl IntoResponse for DownloadClientError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            DownloadClientError::NotFound => (
                StatusCode::NOT_FOUND,
                "Download client not found".to_string(),
            ),
            DownloadClientError::TestFailed(msg) => (StatusCode::BAD_REQUEST, msg),
            DownloadClientError::Internal(msg) => {
                tracing::error!("Download client error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

/// GET /api/v3/downloadclient
pub async fn get_download_clients(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DownloadClientResource>>, DownloadClientError> {
    let repo = DownloadClientRepository::new(state.db.clone());

    let clients = repo.get_all().await.map_err(|e| {
        DownloadClientError::Internal(format!("Failed to fetch download clients: {}", e))
    })?;

    let resources: Vec<DownloadClientResource> = clients.iter().map(db_to_resource).collect();

    Ok(Json(resources))
}

/// GET /api/v3/downloadclient/:id
pub async fn get_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<DownloadClientResource>, DownloadClientError> {
    let repo = DownloadClientRepository::new(state.db.clone());

    let client = repo
        .get_by_id(id as i64)
        .await
        .map_err(|e| {
            DownloadClientError::Internal(format!("Failed to fetch download client: {}", e))
        })?
        .ok_or(DownloadClientError::NotFound)?;

    Ok(Json(db_to_resource(&client)))
}

/// POST /api/v3/downloadclient
pub async fn create_download_client(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DownloadClientResource>,
) -> Result<Json<DownloadClientResource>, DownloadClientError> {
    let repo = DownloadClientRepository::new(state.db.clone());

    let db_model = resource_to_db(&body, None);

    let id = repo.insert(&db_model).await.map_err(|e| {
        DownloadClientError::Internal(format!("Failed to create download client: {}", e))
    })?;

    // Fetch the created client
    let created = repo
        .get_by_id(id)
        .await
        .map_err(|e| {
            DownloadClientError::Internal(format!("Failed to fetch created client: {}", e))
        })?
        .ok_or(DownloadClientError::Internal(
            "Created client not found".to_string(),
        ))?;

    tracing::info!(
        "Created download client: {} ({})",
        created.name,
        created.implementation
    );

    crate::core::logging::log_info(
        "DownloadClientCreated",
        &format!("Created download client: {}", created.name),
    )
    .await;

    Ok(Json(db_to_resource(&created)))
}

/// PUT /api/v3/downloadclient/:id
pub async fn update_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<DownloadClientResource>,
) -> Result<Json<DownloadClientResource>, DownloadClientError> {
    let repo = DownloadClientRepository::new(state.db.clone());

    // Verify it exists
    repo.get_by_id(id as i64)
        .await
        .map_err(|e| {
            DownloadClientError::Internal(format!("Failed to fetch download client: {}", e))
        })?
        .ok_or(DownloadClientError::NotFound)?;

    let db_model = resource_to_db(&body, Some(id as i64));

    repo.update(&db_model).await.map_err(|e| {
        DownloadClientError::Internal(format!("Failed to update download client: {}", e))
    })?;

    // Fetch the updated client
    let updated = repo
        .get_by_id(id as i64)
        .await
        .map_err(|e| {
            DownloadClientError::Internal(format!("Failed to fetch updated client: {}", e))
        })?
        .ok_or(DownloadClientError::Internal(
            "Updated client not found".to_string(),
        ))?;

    tracing::info!("Updated download client: {} (id={})", updated.name, id);

    Ok(Json(db_to_resource(&updated)))
}

/// DELETE /api/v3/downloadclient/:id
pub async fn delete_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, DownloadClientError> {
    let repo = DownloadClientRepository::new(state.db.clone());

    // Get client name for logging
    let client = repo
        .get_by_id(id as i64)
        .await
        .map_err(|e| {
            DownloadClientError::Internal(format!("Failed to fetch download client: {}", e))
        })?
        .ok_or(DownloadClientError::NotFound)?;

    repo.delete(id as i64).await.map_err(|e| {
        DownloadClientError::Internal(format!("Failed to delete download client: {}", e))
    })?;

    tracing::info!("Deleted download client: {} (id={})", client.name, id);

    crate::core::logging::log_info(
        "DownloadClientDeleted",
        &format!("Deleted download client: {}", client.name),
    )
    .await;

    Ok(Json(serde_json::json!({})))
}

/// POST /api/v3/downloadclient/test
/// Test a download client connection
pub async fn test_download_client(
    Json(body): Json<DownloadClientResource>,
) -> Result<Json<serde_json::Value>, DownloadClientError> {
    // Log the settings being tested (without passwords)
    tracing::debug!(
        "Testing download client: implementation={}, fields={:?}",
        body.implementation,
        body.fields
            .iter()
            .filter(|f| f.field_type != "password")
            .map(|f| format!("{}={:?}", f.name, f.value))
            .collect::<Vec<_>>()
    );

    // Convert to DB model temporarily for client creation
    let db_model = resource_to_db(&body, None);

    // Create the actual client and test connection
    let client = create_client_from_model(&db_model).map_err(|e| {
        tracing::warn!("Failed to create download client for test: {}", e);
        DownloadClientError::TestFailed(format!("Configuration error: {}", e))
    })?;

    // Test the connection
    client.test().await.map_err(|e| {
        tracing::warn!("Download client test failed: {}", e);
        DownloadClientError::TestFailed(e.to_string())
    })?;

    // Get version info as extra validation
    let version = client
        .get_version()
        .await
        .unwrap_or_else(|_| "unknown".to_string());

    tracing::info!(
        "Download client test successful: {} (version: {})",
        body.name,
        version
    );

    Ok(Json(serde_json::json!({
        "isValid": true,
        "message": format!("Successfully connected to {} version {}", body.implementation, version)
    })))
}

/// POST /api/v3/downloadclient/testall
/// Test all configured download clients
pub async fn test_all_download_clients(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, DownloadClientError> {
    let repo = DownloadClientRepository::new(state.db.clone());

    let clients = repo.get_all().await.map_err(|e| {
        DownloadClientError::Internal(format!("Failed to fetch download clients: {}", e))
    })?;

    let mut results = Vec::new();

    for db_client in &clients {
        let test_result = match create_client_from_model(db_client) {
            Ok(client) => match client.test().await {
                Ok(_) => serde_json::json!({
                    "id": db_client.id,
                    "name": db_client.name,
                    "isValid": true
                }),
                Err(e) => serde_json::json!({
                    "id": db_client.id,
                    "name": db_client.name,
                    "isValid": false,
                    "validationFailures": [{
                        "propertyName": "",
                        "errorMessage": e.to_string()
                    }]
                }),
            },
            Err(e) => serde_json::json!({
                "id": db_client.id,
                "name": db_client.name,
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

/// GET /api/v3/downloadclient/schema
/// Get available download client types (schemas)
pub async fn get_download_client_schema() -> Json<Vec<DownloadClientResource>> {
    // Return schemas for supported download clients
    let schemas = vec![create_qbittorrent_schema(), create_sabnzbd_schema()];

    Json(schemas)
}

/// Create schema for qBittorrent
fn create_qbittorrent_schema() -> DownloadClientResource {
    DownloadClientResource {
        id: 0,
        name: "".to_string(),
        fields: vec![
            make_field(
                0,
                "host",
                "Host",
                "textbox",
                Some(serde_json::json!("localhost")),
            ),
            make_field(1, "port", "Port", "textbox", Some(serde_json::json!(8080))),
            make_field(
                2,
                "useSsl",
                "Use SSL",
                "checkbox",
                Some(serde_json::json!(false)),
            ),
            make_field(3, "urlBase", "URL Base", "textbox", None),
            make_field(4, "username", "Username", "textbox", None),
            make_field(5, "password", "Password", "password", None),
            make_field(
                6,
                "category",
                "Series Category",
                "textbox",
                Some(serde_json::json!("sonarr")),
            ),
            make_field(
                7,
                "movieCategory",
                "Movie Category",
                "textbox",
                Some(serde_json::json!("radarr")),
            ),
            make_field(
                8,
                "animeCategory",
                "Anime Category",
                "textbox",
                Some(serde_json::json!("anime")),
            ),
            make_field(
                9,
                "recentTvPriority",
                "Recent Priority",
                "select",
                Some(serde_json::json!(0)),
            ),
            make_field(
                10,
                "olderTvPriority",
                "Older Priority",
                "select",
                Some(serde_json::json!(0)),
            ),
            make_field(
                11,
                "initialState",
                "Initial State",
                "select",
                Some(serde_json::json!(0)),
            ),
            make_field(
                12,
                "sequentialOrder",
                "Sequential Order",
                "checkbox",
                Some(serde_json::json!(false)),
            ),
            make_field(
                13,
                "firstAndLast",
                "First and Last",
                "checkbox",
                Some(serde_json::json!(false)),
            ),
        ],
        implementation_name: "qBittorrent".to_string(),
        implementation: "QBittorrent".to_string(),
        config_contract: "QBittorrentSettings".to_string(),
        info_link: Some("https://wiki.servarr.com/sonarr/supported#qbittorrent".to_string()),
        message: None,
        tags: vec![],
        presets: vec![],
        enable: true,
        protocol: "torrent".to_string(),
        priority: 1,
        remove_completed_downloads: true,
        remove_failed_downloads: true,
    }
}

/// Create schema for SABnzbd
fn create_sabnzbd_schema() -> DownloadClientResource {
    DownloadClientResource {
        id: 0,
        name: "".to_string(),
        fields: vec![
            make_field(
                0,
                "host",
                "Host",
                "textbox",
                Some(serde_json::json!("localhost")),
            ),
            make_field(1, "port", "Port", "textbox", Some(serde_json::json!(8080))),
            make_field(
                2,
                "useSsl",
                "Use SSL",
                "checkbox",
                Some(serde_json::json!(false)),
            ),
            make_field(3, "urlBase", "URL Base", "textbox", None),
            make_field(4, "apiKey", "API Key", "textbox", None),
            make_field(
                5,
                "category",
                "Series Category",
                "textbox",
                Some(serde_json::json!("tv")),
            ),
            make_field(
                6,
                "movieCategory",
                "Movie Category",
                "textbox",
                Some(serde_json::json!("radarr")),
            ),
            make_field(
                7,
                "animeCategory",
                "Anime Category",
                "textbox",
                Some(serde_json::json!("anime")),
            ),
            make_field(
                8,
                "recentTvPriority",
                "Recent Priority",
                "select",
                Some(serde_json::json!(-100)),
            ),
            make_field(
                9,
                "olderTvPriority",
                "Older Priority",
                "select",
                Some(serde_json::json!(-100)),
            ),
        ],
        implementation_name: "SABnzbd".to_string(),
        implementation: "Sabnzbd".to_string(),
        config_contract: "SabnzbdSettings".to_string(),
        info_link: Some("https://wiki.servarr.com/sonarr/supported#sabnzbd".to_string()),
        message: None,
        tags: vec![],
        presets: vec![],
        enable: true,
        protocol: "usenet".to_string(),
        priority: 1,
        remove_completed_downloads: true,
        remove_failed_downloads: true,
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_download_clients).post(create_download_client))
        .route(
            "/{id}",
            get(get_download_client)
                .put(update_download_client)
                .delete(delete_download_client),
        )
        .route("/test", post(test_download_client))
        .route("/testall", post(test_all_download_clients))
        .route("/schema", get(get_download_client_schema))
}
