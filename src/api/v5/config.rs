//! Configuration API endpoints

use crate::web::AppState;
use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared mutable configuration state
static CONFIG_STATE: once_cell::sync::Lazy<RwLock<HostConfigResource>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HostConfigResource::default()));

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/host", get(get_host_config).put(update_host_config))
        .route("/naming", get(get_naming_config).put(update_naming_config))
        .route(
            "/mediamanagement",
            get(get_media_management_config).put(update_media_management_config),
        )
        .route(
            "/mediaManagement",
            get(get_media_management_config).put(update_media_management_config),
        )
        .route("/ui", get(get_ui_config).put(update_ui_config))
}

async fn get_host_config(State(state): State<Arc<AppState>>) -> Json<HostConfigResource> {
    let config = CONFIG_STATE.read().await;
    let mut result = config.clone();

    // Only set defaults from AppState if not already configured
    if result.port == 0 {
        result.port = state.config.server.port;
    }
    if result.bind_address.is_empty() || result.bind_address == "*" {
        result.bind_address = state.config.server.bind_address.clone();
    }
    if result.api_key.is_empty() || result.api_key == "pir9-api-key" {
        result.api_key = state.config.security.secret_key.chars().take(32).collect();
    }

    // Force authentication disabled
    result.authentication_method = "none".to_string();
    result.authentication_required = "disabled".to_string();

    // Never return password fields in GET
    result.password = None;
    result.password_confirmation = None;

    Json(result)
}

async fn update_host_config(
    State(state): State<Arc<AppState>>,
    Json(mut config): Json<HostConfigResource>,
) -> Json<HostConfigResource> {
    // Hash the password if provided
    if let Some(ref password) = config.password {
        if !password.is_empty() {
            config.password_hash = Some(hash_password(password));
        }
    }

    // Store the updated config in memory
    {
        let mut stored_config = CONFIG_STATE.write().await;
        *stored_config = config.clone();
    }

    // Persist relevant fields to the config file (read-modify-write)
    let config_path = crate::core::configuration::AppConfig::config_file_path();
    let mut app_config = state.config.clone();
    app_config.server.port = config.port;
    if !config.bind_address.is_empty() && config.bind_address != "*" {
        app_config.server.bind_address = config.bind_address.clone();
    }
    app_config.server.enable_ssl = config.enable_ssl;
    if let Err(e) = app_config.save(&config_path) {
        tracing::warn!("Failed to persist config to {:?}: {}", config_path, e);
    }

    // Clear sensitive fields in response
    config.password = None;
    config.password_confirmation = None;

    Json(config)
}

async fn get_naming_config() -> Json<NamingConfig> {
    Json(NamingConfig::default())
}

async fn update_naming_config(Json(config): Json<NamingConfig>) -> Json<NamingConfig> {
    Json(config)
}

async fn get_media_management_config() -> Json<MediaManagementConfig> {
    Json(MediaManagementConfig::default())
}

async fn update_media_management_config(
    Json(config): Json<MediaManagementConfig>,
) -> Json<MediaManagementConfig> {
    Json(config)
}

async fn get_ui_config() -> Json<UiConfig> {
    Json(UiConfig::default())
}

async fn update_ui_config(Json(config): Json<UiConfig>) -> Json<UiConfig> {
    Json(config)
}

/// Hash password using bcrypt
fn hash_password(password: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verify password against hash
#[allow(dead_code)]
pub fn verify_password(password: &str, hash: &str) -> bool {
    hash_password(password) == hash
}

/// Get stored credentials for authentication
#[allow(dead_code)]
pub async fn get_auth_credentials() -> Option<(String, String)> {
    let config = CONFIG_STATE.read().await;
    if config.authentication_method == "none" {
        return None;
    }

    match (&config.username, &config.password_hash) {
        (Some(username), Some(hash)) if !username.is_empty() => {
            Some((username.clone(), hash.clone()))
        }
        _ => None,
    }
}

/// Check if authentication is enabled (always disabled)
#[allow(dead_code)]
pub async fn is_auth_enabled() -> bool {
    false
}

/// Full host configuration resource (matches Sonarr API)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostConfigResource {
    pub id: i32,
    pub bind_address: String,
    pub port: u16,
    pub ssl_port: u16,
    pub enable_ssl: bool,
    pub launch_browser: bool,
    pub authentication_method: String,
    pub authentication_required: String,
    pub analytics_enabled: bool,
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_confirmation: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub log_level: String,
    pub console_log_level: String,
    pub branch: String,
    pub api_key: String,
    pub ssl_cert_path: Option<String>,
    pub ssl_cert_password: Option<String>,
    pub url_base: String,
    pub instance_name: String,
    pub application_url: Option<String>,
    pub update_automatically: bool,
    pub update_mechanism: String,
    pub update_script_path: Option<String>,
    pub proxy_enabled: bool,
    pub proxy_type: String,
    pub proxy_hostname: Option<String>,
    pub proxy_port: u16,
    pub proxy_username: Option<String>,
    pub proxy_password: Option<String>,
    pub proxy_bypass_filter: Option<String>,
    pub proxy_bypass_local_addresses: bool,
    pub certificate_validation: String,
    pub backup_folder: String,
    pub backup_interval: i32,
    pub backup_retention: i32,
}

impl Default for HostConfigResource {
    fn default() -> Self {
        Self {
            id: 1,
            bind_address: "*".to_string(),
            port: 8989,
            ssl_port: 9898,
            enable_ssl: false,
            launch_browser: true,
            authentication_method: "none".to_string(),
            authentication_required: "disabled".to_string(),
            analytics_enabled: false,
            username: None,
            password: None,
            password_confirmation: None,
            password_hash: None,
            log_level: "info".to_string(),
            console_log_level: "info".to_string(),
            branch: "main".to_string(),
            api_key: "pir9-api-key".to_string(),
            ssl_cert_path: None,
            ssl_cert_password: None,
            url_base: String::new(),
            instance_name: "pir9".to_string(),
            application_url: None,
            update_automatically: false,
            update_mechanism: "builtIn".to_string(),
            update_script_path: None,
            proxy_enabled: false,
            proxy_type: "http".to_string(),
            proxy_hostname: None,
            proxy_port: 8080,
            proxy_username: None,
            proxy_password: None,
            proxy_bypass_filter: None,
            proxy_bypass_local_addresses: true,
            certificate_validation: "enabled".to_string(),
            backup_folder: "Backups".to_string(),
            backup_interval: 7,
            backup_retention: 28,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingConfig {
    pub id: i32,
    pub rename_episodes: bool,
    pub replace_illegal_characters: bool,
    pub colon_replacement_format: i32,
    pub multi_episode_style: i32,
    pub standard_episode_format: String,
    pub daily_episode_format: String,
    pub anime_episode_format: String,
    pub series_folder_format: String,
    pub season_folder_format: String,
    pub specials_folder_format: String,
}

impl Default for NamingConfig {
    fn default() -> Self {
        Self {
            id: 1,
            rename_episodes: true,
            replace_illegal_characters: true,
            colon_replacement_format: 4,
            multi_episode_style: 0,
            standard_episode_format: "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Full}]".to_string(),
            daily_episode_format: "{Series Title} - {Air-Date} - {Episode Title} [{Quality Full}]".to_string(),
            anime_episode_format: "{Series Title} - S{season:00}E{episode:00} - {absolute:000} - {Episode Title} [{Quality Full}]".to_string(),
            series_folder_format: "{Series Title}".to_string(),
            season_folder_format: "Season {season:00}".to_string(),
            specials_folder_format: "Specials".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaManagementConfig {
    pub id: i32,
    pub auto_unmonitor_previously_downloaded_episodes: bool,
    pub recycle_bin: Option<String>,
    pub recycle_bin_cleanup_days: i32,
    pub download_propers_and_repacks: String,
    pub create_empty_series_folders: bool,
    pub delete_empty_folders: bool,
    pub file_date: String,
    pub rescan_after_refresh: String,
    pub set_permissions_linux: bool,
    pub chmod_folder: String,
    pub chown_group: String,
    pub skip_free_space_check_when_importing: bool,
    pub minimum_free_space_when_importing: i32,
    pub copy_using_hardlinks: bool,
    pub import_extra_files: bool,
    pub extra_file_extensions: String,
    pub enable_media_info: bool,
}

impl Default for MediaManagementConfig {
    fn default() -> Self {
        Self {
            id: 1,
            auto_unmonitor_previously_downloaded_episodes: false,
            recycle_bin: None,
            recycle_bin_cleanup_days: 7,
            download_propers_and_repacks: "doNotUpgrade".to_string(),
            create_empty_series_folders: false,
            delete_empty_folders: true,
            file_date: "none".to_string(),
            rescan_after_refresh: "always".to_string(),
            set_permissions_linux: false,
            chmod_folder: "755".to_string(),
            chown_group: String::new(),
            skip_free_space_check_when_importing: false,
            minimum_free_space_when_importing: 100,
            copy_using_hardlinks: true,
            import_extra_files: false,
            extra_file_extensions: "srt,nfo".to_string(),
            enable_media_info: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiConfig {
    pub id: i32,
    pub first_day_of_week: i32,
    pub calendar_week_column_header: String,
    pub short_date_format: String,
    pub long_date_format: String,
    pub time_format: String,
    pub show_relative_dates: bool,
    pub enable_color_impaired_mode: bool,
    pub theme: String,
    pub ui_language: i32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            id: 1,
            first_day_of_week: 0,
            calendar_week_column_header: "ddd M/D".to_string(),
            short_date_format: "MMM D YYYY".to_string(),
            long_date_format: "dddd, MMMM D YYYY".to_string(),
            time_format: "h(:mm)a".to_string(),
            show_relative_dates: true,
            enable_color_impaired_mode: false,
            theme: "auto".to_string(),
            ui_language: 1,
        }
    }
}
