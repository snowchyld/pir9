//! Notification provider implementations
//! Discord and Webhook providers

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::{NotificationField, NotificationPayload, NotificationProvider, NotificationEventType};
use crate::core::datastore::models::NotificationDbModel;

// ============================================================================
// Discord Webhook Provider
// ============================================================================

/// Discord webhook notification provider
pub struct DiscordProvider {
    webhook_url: String,
    username: Option<String>,
    avatar_url: Option<String>,
}

impl DiscordProvider {
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            username: None,
            avatar_url: None,
        }
    }

    pub fn with_username(mut self, username: Option<String>) -> Self {
        self.username = username;
        self
    }

    pub fn with_avatar(mut self, avatar_url: Option<String>) -> Self {
        self.avatar_url = avatar_url;
        self
    }

    fn event_to_color(event_type: &NotificationEventType) -> u32 {
        match event_type {
            NotificationEventType::Grab => 0x3498db,  // Blue
            NotificationEventType::Download => 0x2ecc71,  // Green
            NotificationEventType::Upgrade => 0x9b59b6,  // Purple
            NotificationEventType::HealthIssue => 0xe74c3c,  // Red
            NotificationEventType::HealthRestored => 0x2ecc71,  // Green
            NotificationEventType::SeriesDelete => 0xe67e22,  // Orange
            NotificationEventType::EpisodeFileDelete => 0xe67e22,  // Orange
            NotificationEventType::Test => 0x95a5a6,  // Gray
            _ => 0x3498db,  // Blue default
        }
    }
}

#[derive(Debug, Serialize)]
struct DiscordWebhookPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    embeds: Vec<DiscordEmbed>,
}

#[derive(Debug, Serialize)]
struct DiscordEmbed {
    title: String,
    description: String,
    color: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    footer: Option<DiscordEmbedFooter>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<DiscordEmbedField>,
}

#[derive(Debug, Serialize)]
struct DiscordEmbedFooter {
    text: String,
}

#[derive(Debug, Serialize)]
struct DiscordEmbedField {
    name: String,
    value: String,
    inline: bool,
}

#[async_trait]
impl NotificationProvider for DiscordProvider {
    fn name(&self) -> &str {
        "Discord"
    }

    fn implementation(&self) -> &str {
        "Discord"
    }

    fn config_contract(&self) -> &str {
        "DiscordSettings"
    }

    fn info_link(&self) -> &str {
        "https://wiki.servarr.com/sonarr/settings#connect"
    }

    async fn test(&self) -> Result<()> {
        let test_payload = NotificationPayload {
            event_type: NotificationEventType::Test,
            title: "Pir9 Test Notification".to_string(),
            message: "This is a test notification from Pir9".to_string(),
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
            fields.push(DiscordEmbedField {
                name: "Series".to_string(),
                value: series.clone(),
                inline: true,
            });
        }

        if let Some(episode) = &payload.episode_info {
            fields.push(DiscordEmbedField {
                name: "Episode".to_string(),
                value: format!("S{:02}E{:02}", episode.season_number, episode.episode_number),
                inline: true,
            });
            if let Some(title) = &episode.title {
                fields.push(DiscordEmbedField {
                    name: "Title".to_string(),
                    value: title.clone(),
                    inline: true,
                });
            }
            if let Some(quality) = &episode.quality {
                fields.push(DiscordEmbedField {
                    name: "Quality".to_string(),
                    value: quality.clone(),
                    inline: true,
                });
            }
        }

        if let Some(release) = &payload.release_info {
            fields.push(DiscordEmbedField {
                name: "Release".to_string(),
                value: release.release_title.clone(),
                inline: false,
            });
            fields.push(DiscordEmbedField {
                name: "Indexer".to_string(),
                value: release.indexer.clone(),
                inline: true,
            });
            fields.push(DiscordEmbedField {
                name: "Size".to_string(),
                value: format_size(release.size),
                inline: true,
            });
        }

        let embed = DiscordEmbed {
            title: payload.title.clone(),
            description: payload.message.clone(),
            color: Self::event_to_color(&payload.event_type),
            footer: Some(DiscordEmbedFooter {
                text: "Pir9".to_string(),
            }),
            fields,
        };

        let discord_payload = DiscordWebhookPayload {
            username: self.username.clone(),
            avatar_url: self.avatar_url.clone(),
            embeds: vec![embed],
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&self.webhook_url)
            .json(&discord_payload)
            .send()
            .await
            .context("Failed to send Discord notification")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Discord API returned {}: {}", status, body));
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
                help_text: Some("Discord webhook URL".to_string()),
                help_link: Some("https://support.discord.com/hc/en-us/articles/228383668".to_string()),
                privacy: "apiKey".to_string(),
                is_float: false,
            },
            NotificationField {
                order: 1,
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
            NotificationField {
                order: 2,
                name: "avatar".to_string(),
                label: "Avatar".to_string(),
                value: self.avatar_url.clone().map(serde_json::Value::String),
                field_type: "textbox".to_string(),
                advanced: true,
                help_text: Some("Override the default webhook avatar URL".to_string()),
                help_link: None,
                privacy: "normal".to_string(),
                is_float: false,
            },
        ]
    }
}

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
            title: "Pir9 Test Notification".to_string(),
            message: "This is a test notification from Pir9".to_string(),
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
            WebhookMethod::Get => {
                request.send().await.context("Failed to send webhook GET request")?
            }
            _ => {
                request
                    .json(payload)
                    .send()
                    .await
                    .context("Failed to send webhook notification")?
            }
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
                    }.into()
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
// Provider Factory
// ============================================================================

/// Create a notification provider from a database model
pub fn create_provider_from_model(model: &NotificationDbModel) -> Result<Arc<dyn NotificationProvider>> {
    let settings: serde_json::Value = serde_json::from_str(&model.settings)
        .unwrap_or_else(|_| serde_json::json!({}));

    match model.implementation.as_str() {
        "Discord" => {
            let webhook_url = settings["webHookUrl"]
                .as_str()
                .ok_or_else(|| anyhow!("Discord webhook URL is required"))?
                .to_string();

            let username = settings["username"].as_str().map(String::from);
            let avatar_url = settings["avatar"].as_str().map(String::from);

            Ok(Arc::new(
                DiscordProvider::new(webhook_url)
                    .with_username(username)
                    .with_avatar(avatar_url)
            ))
        }
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

            Ok(Arc::new(
                WebhookProvider::new(url).with_method(method)
            ))
        }
        _ => Err(anyhow!("Unknown notification implementation: {}", model.implementation)),
    }
}

/// Get all available provider schemas for the API
pub fn get_provider_schemas() -> Vec<ProviderSchema> {
    vec![
        ProviderSchema {
            implementation: "Discord".to_string(),
            implementation_name: "Discord".to_string(),
            config_contract: "DiscordSettings".to_string(),
            info_link: "https://wiki.servarr.com/sonarr/settings#connect".to_string(),
            fields: DiscordProvider::new(String::new()).get_fields(),
        },
        ProviderSchema {
            implementation: "Webhook".to_string(),
            implementation_name: "Webhook".to_string(),
            config_contract: "WebhookSettings".to_string(),
            info_link: "https://wiki.servarr.com/sonarr/settings#connect".to_string(),
            fields: WebhookProvider::new(String::new()).get_fields(),
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
    fn test_discord_fields() {
        let provider = DiscordProvider::new("https://discord.com/api/webhooks/test".to_string());
        let fields = provider.get_fields();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "webHookUrl");
    }

    #[test]
    fn test_webhook_fields() {
        let provider = WebhookProvider::new("https://example.com/webhook".to_string());
        let fields = provider.get_fields();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "url");
    }
}
