//! Notification API endpoints

use axum::{
    extract::Path,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationResource {
    pub id: i32,
    pub name: String,
    pub fields: Vec<FieldResource>,
    pub implementation_name: String,
    pub implementation: String,
    pub config_contract: String,
    pub info_link: Option<String>,
    pub message: Option<ProviderMessage>,
    pub tags: Vec<i32>,
    pub presets: Vec<serde_json::Value>,
    pub link: Option<String>,
    pub on_grab: bool,
    pub on_download: bool,
    pub on_upgrade: bool,
    pub on_import_complete: bool,
    pub on_rename: bool,
    pub on_series_add: bool,
    pub on_series_delete: bool,
    pub on_episode_file_delete: bool,
    pub on_episode_file_delete_for_upgrade: bool,
    pub on_health_issue: bool,
    pub include_health_warnings: bool,
    pub on_health_restored: bool,
    pub on_application_update: bool,
    pub on_manual_interaction_required: bool,
    pub supports_on_grab: bool,
    pub supports_on_download: bool,
    pub supports_on_upgrade: bool,
    pub supports_on_import_complete: bool,
    pub supports_on_rename: bool,
    pub supports_on_series_add: bool,
    pub supports_on_series_delete: bool,
    pub supports_on_episode_file_delete: bool,
    pub supports_on_episode_file_delete_for_upgrade: bool,
    pub supports_on_health_issue: bool,
    pub supports_on_health_restored: bool,
    pub supports_on_application_update: bool,
    pub supports_on_manual_interaction_required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
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
    pub select_options: Option<Vec<serde_json::Value>>,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMessage {
    pub message: String,
    #[serde(rename = "type")]
    pub message_type: String,
}

/// GET /api/v3/notification
pub async fn get_notifications() -> Json<Vec<NotificationResource>> {
    Json(vec![])
}

/// GET /api/v3/notification/:id
pub async fn get_notification(Path(id): Path<i32>) -> Json<Option<NotificationResource>> {
    let _ = id;
    Json(None)
}

/// POST /api/v3/notification
pub async fn create_notification(
    Json(body): Json<NotificationResource>,
) -> Json<NotificationResource> {
    Json(body)
}

/// PUT /api/v3/notification/:id
pub async fn update_notification(
    Path(id): Path<i32>,
    Json(mut body): Json<NotificationResource>,
) -> Json<NotificationResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/notification/:id
pub async fn delete_notification(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// POST /api/v3/notification/test
pub async fn test_notification(Json(_body): Json<NotificationResource>) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

/// POST /api/v3/notification/testall
pub async fn test_all_notifications() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

/// GET /api/v3/notification/schema
pub async fn get_notification_schema() -> Json<Vec<NotificationResource>> {
    let schemas = vec![create_webhook_schema(), create_slack_schema()];
    Json(schemas)
}

/// Helper to create a field
fn make_field(
    order: i32,
    name: &str,
    label: &str,
    field_type: &str,
    value: Option<serde_json::Value>,
    help_text: Option<&str>,
) -> FieldResource {
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
        privacy: if field_type == "password" {
            Some("password".to_string())
        } else {
            None
        },
        placeholder: None,
        is_float: None,
    }
}

/// Create default notification resource
fn default_notification(
    implementation: &str,
    implementation_name: &str,
    config_contract: &str,
) -> NotificationResource {
    NotificationResource {
        id: 0,
        name: String::new(),
        fields: vec![],
        implementation_name: implementation_name.to_string(),
        implementation: implementation.to_string(),
        config_contract: config_contract.to_string(),
        info_link: None,
        message: None,
        tags: vec![],
        presets: vec![],
        link: None,
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
        include_health_warnings: true,
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
}

/// Webhook notification schema
fn create_webhook_schema() -> NotificationResource {
    let mut schema = default_notification("Webhook", "Webhook", "WebhookSettings");
    schema.fields = vec![
        make_field(
            0,
            "url",
            "URL",
            "textbox",
            None,
            Some("URL to POST webhook to"),
        ),
        make_field(
            1,
            "method",
            "Method",
            "select",
            Some(serde_json::json!(1)),
            Some("HTTP method"),
        ),
        make_field(
            2,
            "username",
            "Username",
            "textbox",
            None,
            Some("Basic auth username (optional)"),
        ),
        make_field(
            3,
            "password",
            "Password",
            "password",
            None,
            Some("Basic auth password (optional)"),
        ),
    ];
    schema
}

/// Slack notification schema
fn create_slack_schema() -> NotificationResource {
    let mut schema = default_notification("Slack", "Slack", "SlackSettings");
    schema.fields = vec![
        make_field(
            0,
            "webHookUrl",
            "Webhook URL",
            "textbox",
            None,
            Some("Slack incoming webhook URL"),
        ),
        make_field(
            1,
            "channel",
            "Channel",
            "textbox",
            None,
            Some("Override the default webhook channel"),
        ),
        make_field(
            2,
            "username",
            "Username",
            "textbox",
            None,
            Some("Override the default webhook username"),
        ),
    ];
    schema
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_notifications).post(create_notification))
        .route(
            "/{id}",
            get(get_notification)
                .put(update_notification)
                .delete(delete_notification),
        )
        .route("/test", post(test_notification))
        .route("/testall", post(test_all_notifications))
        .route("/schema", get(get_notification_schema))
}
