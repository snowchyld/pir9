#![allow(dead_code)]
//! Application configuration management
//! Handles loading and validation of configuration from files and environment

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use validator::Validate;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AppConfig {
    /// Server configuration
    #[validate(nested)]
    pub server: ServerConfig,

    /// Database configuration
    #[validate(nested)]
    pub database: DatabaseConfig,

    /// Security configuration
    #[validate(nested)]
    pub security: SecurityConfig,

    /// Download client configuration
    #[validate(nested)]
    pub download: DownloadConfig,

    /// Media management configuration
    #[validate(nested)]
    pub media: MediaConfig,

    /// Notification configuration
    pub notifications: NotificationConfig,

    /// Paths configuration
    pub paths: PathConfig,

    /// Redis configuration (optional, for distributed deployments)
    #[serde(default)]
    pub redis: Option<RedisConfig>,
}

/// Redis configuration for distributed deployments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL (e.g., "redis://localhost:6379")
    pub url: String,

    /// Use Redis for event bus (pub/sub)
    #[serde(default)]
    pub use_for_events: bool,

    /// Redis database number (0-15)
    #[serde(default)]
    pub database: u8,

    /// Connection timeout in seconds
    #[serde(default = "default_redis_timeout")]
    pub connection_timeout_secs: u64,
}

fn default_redis_timeout() -> u64 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ServerConfig {
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    #[validate(length(min = 1))]
    pub bind_address: String,

    pub enable_ssl: bool,
    pub ssl_cert_path: Option<PathBuf>,
    pub ssl_key_path: Option<PathBuf>,

    #[validate(range(min = 1))]
    pub request_timeout_secs: u64,

    #[validate(range(min = 1))]
    pub max_body_size_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DatabaseConfig {
    #[validate(length(min = 1))]
    pub database_type: String,

    pub connection_string: String,

    #[validate(range(min = 1))]
    pub max_connections: u32,

    #[validate(range(min = 1))]
    pub connection_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SecurityConfig {
    #[validate(length(min = 32))]
    pub secret_key: String,

    #[serde(default)]
    pub enable_authentication: bool,

    /// Authentication method: "none", "basic", "forms"
    #[serde(default = "default_auth_method")]
    pub authentication_method: String,

    /// When auth is required: "enabled", "disabledForLocalAddresses"
    #[serde(default = "default_auth_required")]
    pub authentication_required: String,

    /// Username for authentication
    #[serde(default)]
    pub username: Option<String>,

    /// Hashed password for authentication
    #[serde(default)]
    pub password_hash: Option<String>,

    #[validate(range(min = 1))]
    #[serde(default = "default_session_timeout")]
    pub session_timeout_hours: u64,

    #[validate(range(min = 1))]
    #[serde(default = "default_max_login_attempts")]
    pub max_login_attempts: u32,

    #[serde(default)]
    pub require_https: bool,
}

fn default_auth_method() -> String {
    "none".to_string()
}

fn default_auth_required() -> String {
    "disabled".to_string()
}

fn default_session_timeout() -> u64 {
    24
}

fn default_max_login_attempts() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DownloadConfig {
    #[validate(range(min = 1))]
    pub check_interval_secs: u64,

    #[validate(range(min = 1))]
    pub download_timeout_mins: u64,

    pub enable_completed_download_handling: bool,
    pub remove_completed_downloads: bool,
    pub remove_failed_downloads: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct MediaConfig {
    pub default_root_folder: PathBuf,

    pub rename_episodes: bool,
    pub replace_illegal_chars: bool,
    pub colon_replacement_format: String,

    #[validate(length(min = 1))]
    pub episode_naming_pattern: String,

    #[validate(length(min = 1))]
    pub season_folder_format: String,

    pub create_empty_series_folders: bool,
    pub delete_empty_folders: bool,
    pub skip_free_space_check: bool,

    #[validate(range(min = 1))]
    pub minimum_free_space_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub providers: Vec<NotificationProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotificationProvider {
    Webhook {
        url: String,
        method: String,
    },
    Email {
        smtp_server: String,
        port: u16,
        username: String,
        password: String,
    },
    Discord {
        webhook_url: String,
    },
    Slack {
        webhook_url: String,
    },
    Telegram {
        bot_token: String,
        chat_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub backup_dir: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                port: 8989,
                bind_address: "0.0.0.0".to_string(),
                enable_ssl: false,
                ssl_cert_path: None,
                ssl_key_path: None,
                request_timeout_secs: 30,
                max_body_size_mb: 100,
            },
            database: DatabaseConfig {
                database_type: "sqlite".to_string(),
                connection_string: "pir9.db".to_string(),
                max_connections: 10,
                connection_timeout_secs: 30,
            },
            security: SecurityConfig {
                secret_key: generate_secret_key(),
                enable_authentication: false,
                authentication_method: "none".to_string(),
                authentication_required: "disabled".to_string(),
                username: None,
                password_hash: None,
                session_timeout_hours: 24,
                max_login_attempts: 5,
                require_https: false,
            },
            download: DownloadConfig {
                check_interval_secs: 60,
                download_timeout_mins: 60,
                enable_completed_download_handling: true,
                remove_completed_downloads: false,
                remove_failed_downloads: true,
            },
            media: MediaConfig {
                default_root_folder: PathBuf::from("/data/tv"),
                rename_episodes: true,
                replace_illegal_chars: true,
                colon_replacement_format: "dash".to_string(),
                episode_naming_pattern:
                    "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Full}]"
                        .to_string(),
                season_folder_format: "Season {season:00}".to_string(),
                create_empty_series_folders: false,
                delete_empty_folders: true,
                skip_free_space_check: false,
                minimum_free_space_mb: 100,
            },
            notifications: NotificationConfig::default(),
            paths: PathConfig {
                config_dir: PathBuf::from("/config"),
                data_dir: PathBuf::from("/data"),
                log_dir: PathBuf::from("/logs"),
                backup_dir: PathBuf::from("/backups"),
            },
            redis: None,
        }
    }
}

impl AppConfig {
    /// Load configuration from file or create default
    pub fn load() -> Result<Self> {
        let config_paths = [
            PathBuf::from("/config/config.toml"),
            PathBuf::from("/config/pir9.toml"),
            PathBuf::from("config/config.toml"),
            dirs::config_dir()
                .map(|d| d.join("pir9/config.toml"))
                .unwrap_or_default(),
        ];

        for path in &config_paths {
            if path.exists() {
                let content = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read config from {:?}", path))?;

                let mut config: AppConfig = toml::from_str(&content)
                    .with_context(|| format!("Failed to parse config from {:?}", path))?;

                // Override with environment variables
                config.override_from_env()?;

                config
                    .validate()
                    .context("Configuration validation failed")?;

                return Ok(config);
            }
        }

        // Create default configuration
        let config = AppConfig::default();

        // Determine config directory - prefer /config for containers, fall back to local
        let config_dir = if PathBuf::from("/config").exists() {
            PathBuf::from("/config")
        } else {
            PathBuf::from("config")
        };

        // Ensure config directory exists
        std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        // Save default config
        let config_str =
            toml::to_string_pretty(&config).context("Failed to serialize default config")?;

        std::fs::write(config_dir.join("config.toml"), config_str)
            .context("Failed to write default config")?;

        Ok(config)
    }

    /// Override configuration values from environment variables
    fn override_from_env(&mut self) -> Result<()> {
        if let Ok(port) = std::env::var("PIR9_PORT") {
            self.server.port = port.parse().context("Invalid PIR9_PORT value")?;
        }

        if let Ok(bind) = std::env::var("PIR9_BIND") {
            self.server.bind_address = bind;
        }

        if let Ok(db_type) = std::env::var("PIR9_DB_TYPE") {
            self.database.database_type = db_type;
        }

        if let Ok(conn_str) = std::env::var("PIR9_DB_CONNECTION") {
            self.database.connection_string = conn_str;
        }

        if let Ok(secret) = std::env::var("PIR9_SECRET_KEY") {
            self.security.secret_key = secret;
        }

        // Redis configuration from environment
        if let Ok(redis_url) = std::env::var("PIR9_REDIS_URL") {
            let use_for_events = std::env::var("PIR9_REDIS_EVENTS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true);

            self.redis = Some(RedisConfig {
                url: redis_url,
                use_for_events,
                database: std::env::var("PIR9_REDIS_DB")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                connection_timeout_secs: default_redis_timeout(),
            });
        }

        Ok(())
    }

    /// Save configuration to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let config_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(path, config_str)
            .with_context(|| format!("Failed to write config to {:?}", path))?;

        Ok(())
    }

    /// Find the config file path using the same search order as load()
    pub fn config_file_path() -> PathBuf {
        let candidates = [
            PathBuf::from("/config/config.toml"),
            PathBuf::from("/config/pir9.toml"),
            PathBuf::from("config/config.toml"),
        ];

        for path in &candidates {
            if path.exists() {
                return path.clone();
            }
        }

        // Fall back to default location
        if PathBuf::from("/config").exists() {
            PathBuf::from("/config/config.toml")
        } else {
            PathBuf::from("config/config.toml")
        }
    }
}

fn generate_secret_key() -> String {
    use base64::{engine::general_purpose, Engine as _};
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..64).map(|_| rng.gen()).collect();
    general_purpose::STANDARD.encode(&bytes)
}

impl From<&AppConfig> for DatabaseConfig {
    fn from(config: &AppConfig) -> Self {
        config.database.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.port, 8989);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(!toml_str.is_empty());
    }
}
