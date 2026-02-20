//! Notification/Connections API endpoints (v5)

use axum::{
    extract::{Path, State},
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::NotificationDbModel;
use crate::core::datastore::repositories::NotificationRepository;
use crate::core::notifications::{
    create_provider_from_model, get_provider_schemas, NotificationService,
};
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationResource {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub fields: Vec<FieldResource>,
    pub implementation_name: String,
    pub implementation: String,
    pub config_contract: String,
    #[serde(default)]
    pub info_link: Option<String>,
    #[serde(default)]
    pub tags: Vec<i64>,
    #[serde(default)]
    pub on_grab: bool,
    #[serde(default)]
    pub on_download: bool,
    #[serde(default)]
    pub on_upgrade: bool,
    #[serde(default)]
    pub on_import_complete: bool,
    #[serde(default)]
    pub on_rename: bool,
    #[serde(default)]
    pub on_series_add: bool,
    #[serde(default)]
    pub on_series_delete: bool,
    #[serde(default)]
    pub on_episode_file_delete: bool,
    #[serde(default)]
    pub on_episode_file_delete_for_upgrade: bool,
    #[serde(default)]
    pub on_health_issue: bool,
    #[serde(default)]
    pub include_health_warnings: bool,
    #[serde(default)]
    pub on_health_restored: bool,
    #[serde(default)]
    pub on_application_update: bool,
    #[serde(default)]
    pub on_manual_interaction_required: bool,
    #[serde(default)]
    pub supports_on_grab: bool,
    #[serde(default)]
    pub supports_on_download: bool,
    #[serde(default)]
    pub supports_on_upgrade: bool,
    #[serde(default)]
    pub supports_on_import_complete: bool,
    #[serde(default)]
    pub supports_on_rename: bool,
    #[serde(default)]
    pub supports_on_series_add: bool,
    #[serde(default)]
    pub supports_on_series_delete: bool,
    #[serde(default)]
    pub supports_on_episode_file_delete: bool,
    #[serde(default)]
    pub supports_on_episode_file_delete_for_upgrade: bool,
    #[serde(default)]
    pub supports_on_health_issue: bool,
    #[serde(default)]
    pub supports_on_health_restored: bool,
    #[serde(default)]
    pub supports_on_application_update: bool,
    #[serde(default)]
    pub supports_on_manual_interaction_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldResource {
    pub order: i32,
    pub name: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub advanced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_link: Option<String>,
    #[serde(default)]
    pub privacy: String,
    #[serde(default)]
    pub is_float: bool,
}

/// Convert database model to API resource
fn model_to_resource(model: &NotificationDbModel) -> NotificationResource {
    // Parse settings JSON to get field values
    let settings: serde_json::Value = serde_json::from_str(&model.settings)
        .unwrap_or_else(|_| serde_json::json!({}));

    // Parse tags JSON
    let tags: Vec<i64> = serde_json::from_str(&model.tags)
        .unwrap_or_default();

    // Build fields from settings
    let fields = settings_to_fields(&model.implementation, &settings);

    // Determine capabilities based on implementation
    let (supports_on_grab, supports_on_download, supports_on_upgrade, supports_on_rename,
         supports_on_series_add, supports_on_series_delete, supports_on_episode_file_delete,
         supports_on_episode_file_delete_for_upgrade, supports_on_health_issue,
         supports_on_health_restored, supports_on_application_update,
         supports_on_manual_interaction_required, supports_on_import_complete) =
        get_implementation_capabilities(&model.implementation);

    NotificationResource {
        id: model.id,
        name: model.name.clone(),
        fields,
        implementation_name: model.implementation.clone(),
        implementation: model.implementation.clone(),
        config_contract: model.config_contract.clone(),
        info_link: Some("https://wiki.servarr.com/sonarr/settings#connect".to_string()),
        tags,
        on_grab: model.on_grab,
        on_download: model.on_download,
        on_upgrade: model.on_upgrade,
        on_import_complete: model.on_download, // Same as download for now
        on_rename: model.on_rename,
        on_series_add: false, // Not in current DB schema
        on_series_delete: model.on_series_delete,
        on_episode_file_delete: model.on_episode_file_delete,
        on_episode_file_delete_for_upgrade: model.on_episode_file_delete_for_upgrade,
        on_health_issue: model.on_health_issue,
        include_health_warnings: model.include_health_warnings,
        on_health_restored: model.on_health_restored,
        on_application_update: model.on_application_update,
        on_manual_interaction_required: model.on_manual_interaction_required,
        supports_on_grab,
        supports_on_download,
        supports_on_upgrade,
        supports_on_import_complete,
        supports_on_rename,
        supports_on_series_add,
        supports_on_series_delete,
        supports_on_episode_file_delete,
        supports_on_episode_file_delete_for_upgrade,
        supports_on_health_issue,
        supports_on_health_restored,
        supports_on_application_update,
        supports_on_manual_interaction_required,
    }
}

/// Convert API resource to database model
fn resource_to_model(resource: &NotificationResource) -> NotificationDbModel {
    // Convert fields to settings JSON
    let settings = fields_to_settings(&resource.fields);
    let tags = serde_json::to_string(&resource.tags).unwrap_or_else(|_| "[]".to_string());

    NotificationDbModel {
        id: resource.id,
        name: resource.name.clone(),
        implementation: resource.implementation.clone(),
        config_contract: resource.config_contract.clone(),
        on_grab: resource.on_grab,
        on_download: resource.on_download,
        on_upgrade: resource.on_upgrade,
        on_rename: resource.on_rename,
        on_series_delete: resource.on_series_delete,
        on_episode_file_delete: resource.on_episode_file_delete,
        on_episode_file_delete_for_upgrade: resource.on_episode_file_delete_for_upgrade,
        on_health_issue: resource.on_health_issue,
        on_health_restored: resource.on_health_restored,
        on_manual_interaction_required: resource.on_manual_interaction_required,
        on_application_update: resource.on_application_update,
        include_health_warnings: resource.include_health_warnings,
        settings,
        tags,
    }
}

/// Build field resources from settings
fn settings_to_fields(implementation: &str, settings: &serde_json::Value) -> Vec<FieldResource> {
    match implementation {
        "Discord" => vec![
            FieldResource {
                order: 0,
                name: "webHookUrl".to_string(),
                label: "Webhook URL".to_string(),
                value: settings.get("webHookUrl").cloned(),
                field_type: "textbox".to_string(),
                advanced: false,
                help_text: Some("Discord webhook URL".to_string()),
                help_link: Some("https://support.discord.com/hc/en-us/articles/228383668".to_string()),
                privacy: "apiKey".to_string(),
                is_float: false,
            },
            FieldResource {
                order: 1,
                name: "username".to_string(),
                label: "Username".to_string(),
                value: settings.get("username").cloned(),
                field_type: "textbox".to_string(),
                advanced: true,
                help_text: Some("Override the default webhook username".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
            FieldResource {
                order: 2,
                name: "avatar".to_string(),
                label: "Avatar".to_string(),
                value: settings.get("avatar").cloned(),
                field_type: "textbox".to_string(),
                advanced: true,
                help_text: Some("Override the default webhook avatar URL".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
        ],
        "Webhook" => vec![
            FieldResource {
                order: 0,
                name: "url".to_string(),
                label: "URL".to_string(),
                value: settings.get("url").cloned(),
                field_type: "textbox".to_string(),
                advanced: false,
                help_text: Some("URL to send notifications to".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
            FieldResource {
                order: 1,
                name: "method".to_string(),
                label: "Method".to_string(),
                value: settings.get("method").cloned().or(Some(serde_json::json!(1))),
                field_type: "select".to_string(),
                advanced: false,
                help_text: Some("HTTP method to use".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
        ],
        "Email" => vec![
            FieldResource {
                order: 0,
                name: "server".to_string(),
                label: "SMTP Server".to_string(),
                value: settings.get("server").cloned(),
                field_type: "textbox".to_string(),
                advanced: false,
                help_text: Some("SMTP server hostname".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
            FieldResource {
                order: 1,
                name: "port".to_string(),
                label: "Port".to_string(),
                value: settings.get("port").cloned().or(Some(serde_json::json!(587))),
                field_type: "number".to_string(),
                advanced: false,
                help_text: Some("SMTP port".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
            FieldResource {
                order: 2,
                name: "username".to_string(),
                label: "Username".to_string(),
                value: settings.get("username").cloned(),
                field_type: "textbox".to_string(),
                advanced: false,
                help_text: Some("SMTP username".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
            FieldResource {
                order: 3,
                name: "password".to_string(),
                label: "Password".to_string(),
                value: None, // Never expose password
                field_type: "password".to_string(),
                advanced: false,
                help_text: Some("SMTP password".to_string()),
                help_link: None,
                privacy: "password".to_string(),
                is_float: false,
            },
            FieldResource {
                order: 4,
                name: "from".to_string(),
                label: "From Address".to_string(),
                value: settings.get("from").cloned(),
                field_type: "textbox".to_string(),
                advanced: false,
                help_text: Some("Email address to send from".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
            FieldResource {
                order: 5,
                name: "to".to_string(),
                label: "To Addresses".to_string(),
                value: settings.get("to").cloned(),
                field_type: "tag".to_string(),
                advanced: false,
                help_text: Some("Email addresses to send to".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
        ],
        _ => vec![],
    }
}

/// Convert field resources back to settings JSON
fn fields_to_settings(fields: &[FieldResource]) -> String {
    let mut settings = serde_json::Map::new();
    for field in fields {
        if let Some(value) = &field.value {
            settings.insert(field.name.clone(), value.clone());
        }
    }
    serde_json::to_string(&serde_json::Value::Object(settings))
        .unwrap_or_else(|_| "{}".to_string())
}

/// Get implementation capabilities
fn get_implementation_capabilities(implementation: &str) -> (bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool) {
    // All implementations support all events for now
    match implementation {
        "Discord" | "Webhook" | "Email" => (
            true,  // supports_on_grab
            true,  // supports_on_download
            true,  // supports_on_upgrade
            true,  // supports_on_rename
            true,  // supports_on_series_add
            true,  // supports_on_series_delete
            true,  // supports_on_episode_file_delete
            true,  // supports_on_episode_file_delete_for_upgrade
            true,  // supports_on_health_issue
            true,  // supports_on_health_restored
            true,  // supports_on_application_update
            true,  // supports_on_manual_interaction_required
            true,  // supports_on_import_complete
        ),
        _ => (false, false, false, false, false, false, false, false, false, false, false, false, false),
    }
}

pub async fn get_notifications(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<NotificationResource>> {
    let repo = NotificationRepository::new(state.db.clone());

    match repo.get_all().await {
        Ok(notifications) => {
            let resources: Vec<NotificationResource> = notifications
                .iter()
                .map(model_to_resource)
                .collect();
            Json(resources)
        }
        Err(e) => {
            tracing::error!("Failed to get notifications: {}", e);
            Json(vec![])
        }
    }
}

pub async fn get_notification(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<Option<NotificationResource>> {
    let repo = NotificationRepository::new(state.db.clone());

    match repo.get_by_id(id).await {
        Ok(Some(notification)) => Json(Some(model_to_resource(&notification))),
        Ok(None) => Json(None),
        Err(e) => {
            tracing::error!("Failed to get notification {}: {}", id, e);
            Json(None)
        }
    }
}

pub async fn create_notification(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NotificationResource>,
) -> Json<NotificationResource> {
    let repo = NotificationRepository::new(state.db.clone());
    let model = resource_to_model(&body);

    match repo.insert(&model).await {
        Ok(id) => {
            // Return the created notification with its new ID
            let mut created = body;
            created.id = id;
            Json(created)
        }
        Err(e) => {
            tracing::error!("Failed to create notification: {}", e);
            Json(body)
        }
    }
}

pub async fn update_notification(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(mut body): Json<NotificationResource>,
) -> Json<NotificationResource> {
    body.id = id;
    let repo = NotificationRepository::new(state.db.clone());
    let model = resource_to_model(&body);

    match repo.update(&model).await {
        Ok(()) => Json(body),
        Err(e) => {
            tracing::error!("Failed to update notification {}: {}", id, e);
            Json(body)
        }
    }
}

pub async fn delete_notification(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let repo = NotificationRepository::new(state.db.clone());

    match repo.delete(id).await {
        Ok(()) => Json(serde_json::json!({})),
        Err(e) => {
            tracing::error!("Failed to delete notification {}: {}", id, e);
            Json(serde_json::json!({ "error": e.to_string() }))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRequest {
    #[serde(flatten)]
    pub notification: NotificationResource,
}

pub async fn test_notification(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NotificationResource>,
) -> Json<serde_json::Value> {
    // Convert to model and create provider
    let model = resource_to_model(&body);

    match create_provider_from_model(&model) {
        Ok(provider) => match provider.test().await {
            Ok(()) => Json(serde_json::json!({
                "isValid": true,
                "failures": []
            })),
            Err(e) => Json(serde_json::json!({
                "isValid": false,
                "failures": [{
                    "propertyName": "",
                    "errorMessage": e.to_string(),
                    "severity": "error"
                }]
            })),
        },
        Err(e) => Json(serde_json::json!({
            "isValid": false,
            "failures": [{
                "propertyName": "implementation",
                "errorMessage": e.to_string(),
                "severity": "error"
            }]
        })),
    }
}

pub async fn test_all_notifications(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<serde_json::Value>> {
    let notification_service = NotificationService::new(state.db.clone(), state.event_bus.clone());
    let results = notification_service.test_all_notifications().await;

    let responses: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(name, result)| match result {
            Ok(()) => serde_json::json!({
                "name": name,
                "isValid": true,
                "failures": []
            }),
            Err(e) => serde_json::json!({
                "name": name,
                "isValid": false,
                "failures": [{
                    "propertyName": "",
                    "errorMessage": e.to_string(),
                    "severity": "error"
                }]
            }),
        })
        .collect();

    Json(responses)
}

pub async fn get_notification_schema() -> Json<Vec<NotificationResource>> {
    let schemas = get_provider_schemas();

    let resources: Vec<NotificationResource> = schemas
        .into_iter()
        .map(|schema| {
            let fields: Vec<FieldResource> = schema.fields.into_iter().map(|f| FieldResource {
                order: f.order,
                name: f.name,
                label: f.label,
                value: f.value,
                field_type: f.field_type,
                advanced: f.advanced,
                help_text: f.help_text,
                help_link: f.help_link,
                privacy: f.privacy,
                is_float: f.is_float,
            }).collect();

            NotificationResource {
                id: 0,
                name: String::new(),
                fields,
                implementation_name: schema.implementation_name,
                implementation: schema.implementation.clone(),
                config_contract: schema.config_contract,
                info_link: Some(schema.info_link),
                tags: vec![],
                on_grab: false,
                on_download: false,
                on_upgrade: false,
                on_import_complete: false,
                on_rename: false,
                on_series_add: false,
                on_series_delete: false,
                on_episode_file_delete: false,
                on_episode_file_delete_for_upgrade: false,
                on_health_issue: false,
                include_health_warnings: false,
                on_health_restored: false,
                on_application_update: false,
                on_manual_interaction_required: false,
                supports_on_grab: true,
                supports_on_download: true,
                supports_on_upgrade: true,
                supports_on_import_complete: true,
                supports_on_rename: true,
                supports_on_series_add: true,
                supports_on_series_delete: true,
                supports_on_episode_file_delete: true,
                supports_on_episode_file_delete_for_upgrade: true,
                supports_on_health_issue: true,
                supports_on_health_restored: true,
                supports_on_application_update: true,
                supports_on_manual_interaction_required: true,
            }
        })
        .collect();

    Json(resources)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_notifications).post(create_notification))
        .route("/{id}", get(get_notification).put(update_notification).delete(delete_notification))
        .route("/test", post(test_notification))
        .route("/testall", post(test_all_notifications))
        .route("/schema", get(get_notification_schema))
}
