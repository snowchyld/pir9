//! Notification provider implementations
//! Slack and Webhook providers

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::{NotificationEventType, NotificationField, NotificationPayload, NotificationProvider};
use crate::core::datastore::models::NotificationDbModel;

// ============================================================================
// Generic Webhook Provider
// ============================================================================

/// Generic HTTP webhook notification provider
pub struct WebhookProvider {
    url: String,
    method: WebhookMethod,
    headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum WebhookMethod {
    #[default]
    Post,
    Put,
    Get,
}

impl WebhookProvider {
    pub fn new(url: String) -> Self {
        Self {
            url,
            method: WebhookMethod::Post,
            headers: HashMap::new(),
        }
    }

    pub fn with_method(mut self, method: WebhookMethod) -> Self {
        self.method = method;
        self
    }

    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }
}

#[async_trait]
impl NotificationProvider for WebhookProvider {
    fn name(&self) -> &str {
        "Webhook"
    }

    fn implementation(&self) -> &str {
        "Webhook"
    }

    fn config_contract(&self) -> &str {
        "WebhookSettings"
    }

    fn info_link(&self) -> &str {
        "https://wiki.servarr.com/sonarr/settings#connect"
    }

    async fn test(&self) -> Result<()> {
        let test_payload = NotificationPayload {
            event_type: NotificationEventType::Test,
            title: "pir9 Test Notification".to_string(),
            message: "This is a test notification from pir9".to_string(),
            series_title: Some("Test Series".to_string()),
            series_id: None,
            episode_info: None,
            release_info: None,
            health_info: None,
        };
        self.send(&test_payload).await
    }

    async fn send(&self, payload: &NotificationPayload) -> Result<()> {
        let client = reqwest::Client::new();

        let mut request = match self.method {
            WebhookMethod::Post => client.post(&self.url),
            WebhookMethod::Put => client.put(&self.url),
            WebhookMethod::Get => client.get(&self.url),
        };

        // Add custom headers
        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        // For GET requests, we can't send a body
        let response = match self.method {
            WebhookMethod::Get => request
                .send()
                .await
                .context("Failed to send webhook GET request")?,
            _ => request
                .json(payload)
                .send()
                .await
                .context("Failed to send webhook notification")?,
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Webhook returned {}: {}", status, body));
        }

        Ok(())
    }

    fn get_fields(&self) -> Vec<NotificationField> {
        vec![
            NotificationField {
                order: 0,
                name: "url".to_string(),
                label: "URL".to_string(),
                value: Some(serde_json::Value::String(self.url.clone())),
                field_type: "textbox".to_string(),
                advanced: false,
                help_text: Some("URL to send notifications to".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
            NotificationField {
                order: 1,
                name: "method".to_string(),
                label: "Method".to_string(),
                value: Some(serde_json::Value::Number(
                    match self.method {
                        WebhookMethod::Post => 1,
                        WebhookMethod::Put => 2,
                        WebhookMethod::Get => 3,
                    }
                    .into(),
                )),
                field_type: "select".to_string(),
                advanced: false,
                help_text: Some("HTTP method to use".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
        ]
    }
}

// ============================================================================
// Slack Incoming Webhook Provider
// ============================================================================

/// Slack incoming webhook notification provider
pub struct SlackProvider {
    webhook_url: String,
    channel: Option<String>,
    username: Option<String>,
}

impl SlackProvider {
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            channel: None,
            username: None,
        }
    }

    pub fn with_channel(mut self, channel: Option<String>) -> Self {
        self.channel = channel;
        self
    }

    pub fn with_username(mut self, username: Option<String>) -> Self {
        self.username = username;
        self
    }
}

#[derive(Debug, Serialize)]
struct SlackWebhookPayload {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<SlackAttachment>,
}

#[derive(Debug, Serialize)]
struct SlackAttachment {
    fallback: String,
    title: String,
    text: String,
    color: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<SlackField>,
}

#[derive(Debug, Serialize)]
struct SlackField {
    title: String,
    value: String,
    short: bool,
}

#[async_trait]
impl NotificationProvider for SlackProvider {
    fn name(&self) -> &str {
        "Slack"
    }

    fn implementation(&self) -> &str {
        "Slack"
    }

    fn config_contract(&self) -> &str {
        "SlackSettings"
    }

    fn info_link(&self) -> &str {
        "https://wiki.servarr.com/sonarr/settings#connect"
    }

    async fn test(&self) -> Result<()> {
        let test_payload = NotificationPayload {
            event_type: NotificationEventType::Test,
            title: "pir9 Test Notification".to_string(),
            message: "This is a test notification from pir9".to_string(),
            series_title: Some("Test Series".to_string()),
            series_id: None,
            episode_info: None,
            release_info: None,
            health_info: None,
        };
        self.send(&test_payload).await
    }

    async fn send(&self, payload: &NotificationPayload) -> Result<()> {
        let mut fields = Vec::new();

        if let Some(series) = &payload.series_title {
            fields.push(SlackField {
                title: "Series".to_string(),
                value: series.clone(),
                short: true,
            });
        }

        if let Some(episode) = &payload.episode_info {
            fields.push(SlackField {
                title: "Episode".to_string(),
                value: format!(
                    "S{:02}E{:02}",
                    episode.season_number, episode.episode_number
                ),
                short: true,
            });
            if let Some(title) = &episode.title {
                fields.push(SlackField {
                    title: "Title".to_string(),
                    value: title.clone(),
                    short: true,
                });
            }
        }

        if let Some(release) = &payload.release_info {
            fields.push(SlackField {
                title: "Release".to_string(),
                value: release.release_title.clone(),
                short: false,
            });
            fields.push(SlackField {
                title: "Size".to_string(),
                value: format_size(release.size),
                short: true,
            });
        }

        let color = match payload.event_type {
            NotificationEventType::Grab => "#3498db",
            NotificationEventType::Download => "#2ecc71",
            NotificationEventType::Upgrade => "#9b59b6",
            NotificationEventType::HealthIssue => "#e74c3c",
            NotificationEventType::HealthRestored => "#2ecc71",
            NotificationEventType::SeriesDelete | NotificationEventType::EpisodeFileDelete => {
                "#e67e22"
            }
            _ => "#95a5a6",
        };

        let slack_payload = SlackWebhookPayload {
            text: String::new(),
            channel: self.channel.clone(),
            username: self.username.clone(),
            attachments: vec![SlackAttachment {
                fallback: format!("{}: {}", payload.title, payload.message),
                title: payload.title.clone(),
                text: payload.message.clone(),
                color: color.to_string(),
                fields,
            }],
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&self.webhook_url)
            .json(&slack_payload)
            .send()
            .await
            .context("Failed to send Slack notification")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Slack API returned {}: {}", status, body));
        }

        Ok(())
    }

    fn get_fields(&self) -> Vec<NotificationField> {
        vec![
            NotificationField {
                order: 0,
                name: "webHookUrl".to_string(),
                label: "Webhook URL".to_string(),
                value: Some(serde_json::Value::String(self.webhook_url.clone())),
                field_type: "textbox".to_string(),
                advanced: false,
                help_text: Some("Slack incoming webhook URL".to_string()),
                help_link: Some("https://api.slack.com/messaging/webhooks".to_string()),
                privacy: "apiKey".to_string(),
                is_float: false,
            },
            NotificationField {
                order: 1,
                name: "channel".to_string(),
                label: "Channel".to_string(),
                value: self.channel.clone().map(serde_json::Value::String),
                field_type: "textbox".to_string(),
                advanced: false,
                help_text: Some(
                    "Override the default webhook channel (e.g. #downloads)".to_string(),
                ),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
            NotificationField {
                order: 2,
                name: "username".to_string(),
                label: "Username".to_string(),
                value: self.username.clone().map(serde_json::Value::String),
                field_type: "textbox".to_string(),
                advanced: true,
                help_text: Some("Override the default webhook username".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
        ]
    }
}

// ============================================================================
// Provider Factory
// ============================================================================

/// Create a notification provider from a database model
pub fn create_provider_from_model(
    model: &NotificationDbModel,
) -> Result<Arc<dyn NotificationProvider>> {
    let settings: serde_json::Value =
        serde_json::from_str(&model.settings).unwrap_or_else(|_| serde_json::json!({}));

    match model.implementation.as_str() {
        "Webhook" => {
            let url = settings["url"]
                .as_str()
                .ok_or_else(|| anyhow!("Webhook URL is required"))?
                .to_string();

            let method = match settings["method"].as_i64().unwrap_or(1) {
                2 => WebhookMethod::Put,
                3 => WebhookMethod::Get,
                _ => WebhookMethod::Post,
            };

            Ok(Arc::new(WebhookProvider::new(url).with_method(method)))
        }
        "Slack" => {
            let webhook_url = settings["webHookUrl"]
                .as_str()
                .ok_or_else(|| anyhow!("Slack webhook URL is required"))?
                .to_string();

            let channel = settings["channel"].as_str().map(String::from);
            let username = settings["username"].as_str().map(String::from);

            Ok(Arc::new(
                SlackProvider::new(webhook_url)
                    .with_channel(channel)
                    .with_username(username),
            ))
        }
        _ => Err(anyhow!(
            "Unknown notification implementation: {}",
            model.implementation
        )),
    }
}

/// Get all available provider schemas for the API
pub fn get_provider_schemas() -> Vec<ProviderSchema> {
    vec![
        ProviderSchema {
            implementation: "Webhook".to_string(),
            implementation_name: "Webhook".to_string(),
            config_contract: "WebhookSettings".to_string(),
            info_link: "https://wiki.servarr.com/sonarr/settings#connect".to_string(),
            fields: WebhookProvider::new(String::new()).get_fields(),
        },
        ProviderSchema {
            implementation: "Slack".to_string(),
            implementation_name: "Slack".to_string(),
            config_contract: "SlackSettings".to_string(),
            info_link: "https://wiki.servarr.com/sonarr/settings#connect".to_string(),
            fields: SlackProvider::new(String::new()).get_fields(),
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSchema {
    pub implementation: String,
    pub implementation_name: String,
    pub config_contract: String,
    pub info_link: String,
    pub fields: Vec<NotificationField>,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn format_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
    }

    #[test]
    fn test_webhook_fields() {
        let provider = WebhookProvider::new("https://example.com/webhook".to_string());
        let fields = provider.get_fields();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "url");
    }
}
