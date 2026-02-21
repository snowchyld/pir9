//! Configuration API endpoints

use axum::{extract::State, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

// Host Config
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostConfigResource {
    pub id: i32,
    pub bind_address: String,
    pub port: i32,
    pub ssl_port: i32,
    pub enable_ssl: bool,
    pub launch_browser: bool,
    pub authentication_method: String,
    pub authentication_required: String,
    pub analytics_enabled: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub password_confirmation: Option<String>,
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
    pub proxy_port: i32,
    pub proxy_username: Option<String>,
    pub proxy_password: Option<String>,
    pub proxy_bypass_filter: Option<String>,
    pub proxy_bypass_local_addresses: bool,
    pub certificate_validation: String,
    pub backup_folder: String,
    pub backup_interval: i32,
    pub backup_retention: i32,
}

// UI Config
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiConfigResource {
    pub id: i32,
    pub first_day_of_week: i32,
    pub calendar_week_column_header: String,
    pub short_date_format: String,
    pub long_date_format: String,
    pub time_format: String,
    pub show_relative_dates: bool,
    pub enable_color_impaired_mode: bool,
    pub ui_language: i32,
    pub theme: String,
}

// Naming Config
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingConfigResource {
    pub id: i32,
    pub rename_episodes: bool,
    pub replace_illegal_characters: bool,
    pub multi_episode_style: i32,
    pub standard_episode_format: String,
    pub daily_episode_format: String,
    pub anime_episode_format: String,
    pub series_folder_format: String,
    pub season_folder_format: String,
    pub specials_folder_format: String,
    pub include_series_title: Option<bool>,
    pub include_episode_title: Option<bool>,
    pub include_quality: Option<bool>,
    pub replace_spaces: Option<bool>,
    pub separator: Option<String>,
    pub number_style: Option<String>,
}

// Media Management Config
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaManagementConfigResource {
    pub id: i32,
    pub auto_unmonitor_previously_downloaded_episodes: bool,
    pub recycle_bin: String,
    pub recycle_bin_cleanup_days: i32,
    pub download_propers_and_repacks: String,
    pub create_empty_series_folders: bool,
    pub delete_empty_folders: bool,
    pub file_date: String,
    pub rescan_after_refresh: String,
    pub set_permissions_linux: bool,
    pub chmod_folder: String,
    pub chown_group: String,
    pub episode_title_required: String,
    pub skip_free_space_check_when_importing: bool,
    pub minimum_free_space_when_importing: i32,
    pub copy_using_hardlinks: bool,
    pub use_script_import: bool,
    pub script_import_path: Option<String>,
    pub import_extra_files: bool,
    pub extra_file_extensions: String,
    pub enable_media_info: bool,
}

// Indexer Config
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerConfigResource {
    pub id: i32,
    pub minimum_age: i32,
    pub retention: i32,
    pub maximum_size: i32,
    pub rss_sync_interval: i32,
}

// Download Client Config
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadClientConfigResource {
    pub id: i32,
    pub download_client_working_folders: String,
    pub enable_completed_download_handling: bool,
    pub auto_redownload_failed: bool,
    pub auto_redownload_failed_from_interactive_search: bool,
}

/// GET /api/v3/config/host
/// Authentication is always disabled
pub async fn get_host_config() -> Json<HostConfigResource> {
    Json(HostConfigResource {
        id: 1,
        bind_address: "*".to_string(),
        port: 8989,
        ssl_port: 9898,
        enable_ssl: false,
        launch_browser: true,
        authentication_method: "none".to_string(), // Always disabled
        authentication_required: "disabled".to_string(), // Always disabled
        analytics_enabled: false,
        username: None,
        password: None,
        password_confirmation: None,
        log_level: "info".to_string(),
        console_log_level: "info".to_string(),
        branch: "main".to_string(),
        api_key: "pir9-api-key".to_string(),
        ssl_cert_path: None,
        ssl_cert_password: None,
        url_base: "".to_string(),
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
    })
}

/// PUT /api/v3/config/host
pub async fn update_host_config(Json(body): Json<HostConfigResource>) -> Json<HostConfigResource> {
    Json(body)
}

/// GET /api/v3/config/ui
pub async fn get_ui_config() -> Json<UiConfigResource> {
    Json(UiConfigResource {
        id: 1,
        first_day_of_week: 0,
        calendar_week_column_header: "ddd M/D".to_string(),
        short_date_format: "MMM D YYYY".to_string(),
        long_date_format: "dddd, MMMM D YYYY".to_string(),
        time_format: "h(:mm)a".to_string(),
        show_relative_dates: true,
        enable_color_impaired_mode: false,
        ui_language: 1,
        theme: "auto".to_string(),
    })
}

/// PUT /api/v3/config/ui
pub async fn update_ui_config(Json(body): Json<UiConfigResource>) -> Json<UiConfigResource> {
    Json(body)
}

/// GET /api/v3/config/naming
pub async fn get_naming_config(State(state): State<Arc<AppState>>) -> Json<NamingConfigResource> {
    let media = state.config.read().media.clone();
    Json(NamingConfigResource {
        id: 1,
        rename_episodes: media.rename_episodes,
        replace_illegal_characters: media.replace_illegal_chars,
        multi_episode_style: media.multi_episode_style,
        standard_episode_format: media.episode_naming_pattern.clone(),
        daily_episode_format: media.daily_episode_format.clone(),
        anime_episode_format: media.anime_episode_format.clone(),
        series_folder_format: media.series_folder_format.clone(),
        season_folder_format: media.season_folder_format.clone(),
        specials_folder_format: media.specials_folder_format.clone(),
        include_series_title: None,
        include_episode_title: None,
        include_quality: None,
        replace_spaces: None,
        separator: None,
        number_style: None,
    })
}

/// PUT /api/v3/config/naming
pub async fn update_naming_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NamingConfigResource>,
) -> Json<NamingConfigResource> {
    // Persist to config file and update in-memory config
    let config_path = crate::core::configuration::AppConfig::config_file_path();
    {
        let mut app_config = state.config.write();
        app_config.media.rename_episodes = body.rename_episodes;
        app_config.media.replace_illegal_chars = body.replace_illegal_characters;
        app_config.media.multi_episode_style = body.multi_episode_style;
        app_config.media.episode_naming_pattern = body.standard_episode_format.clone();
        app_config.media.daily_episode_format = body.daily_episode_format.clone();
        app_config.media.anime_episode_format = body.anime_episode_format.clone();
        app_config.media.series_folder_format = body.series_folder_format.clone();
        app_config.media.season_folder_format = body.season_folder_format.clone();
        app_config.media.specials_folder_format = body.specials_folder_format.clone();

        if let Err(e) = app_config.save(&config_path) {
            tracing::warn!(
                "Failed to persist v3 naming config to {:?}: {}",
                config_path,
                e
            );
        }
    }

    Json(body)
}

/// GET /api/v3/config/mediamanagement
pub async fn get_media_management_config() -> Json<MediaManagementConfigResource> {
    Json(MediaManagementConfigResource {
        id: 1,
        auto_unmonitor_previously_downloaded_episodes: false,
        recycle_bin: "".to_string(),
        recycle_bin_cleanup_days: 7,
        download_propers_and_repacks: "preferAndUpgrade".to_string(),
        create_empty_series_folders: false,
        delete_empty_folders: false,
        file_date: "none".to_string(),
        rescan_after_refresh: "always".to_string(),
        set_permissions_linux: false,
        chmod_folder: "755".to_string(),
        chown_group: "".to_string(),
        episode_title_required: "always".to_string(),
        skip_free_space_check_when_importing: false,
        minimum_free_space_when_importing: 100,
        copy_using_hardlinks: true,
        use_script_import: false,
        script_import_path: None,
        import_extra_files: false,
        extra_file_extensions: "srt".to_string(),
        enable_media_info: true,
    })
}

/// PUT /api/v3/config/mediamanagement
pub async fn update_media_management_config(
    Json(body): Json<MediaManagementConfigResource>,
) -> Json<MediaManagementConfigResource> {
    Json(body)
}

/// GET /api/v3/config/indexer
pub async fn get_indexer_config() -> Json<IndexerConfigResource> {
    Json(IndexerConfigResource {
        id: 1,
        minimum_age: 0,
        retention: 0,
        maximum_size: 0,
        rss_sync_interval: 15,
    })
}

/// PUT /api/v3/config/indexer
pub async fn update_indexer_config(
    Json(body): Json<IndexerConfigResource>,
) -> Json<IndexerConfigResource> {
    Json(body)
}

/// GET /api/v3/config/downloadclient
pub async fn get_download_client_config() -> Json<DownloadClientConfigResource> {
    Json(DownloadClientConfigResource {
        id: 1,
        download_client_working_folders: "_UNPACK_|_FAILED_".to_string(),
        enable_completed_download_handling: true,
        auto_redownload_failed: true,
        auto_redownload_failed_from_interactive_search: false,
    })
}

/// PUT /api/v3/config/downloadclient
pub async fn update_download_client_config(
    Json(body): Json<DownloadClientConfigResource>,
) -> Json<DownloadClientConfigResource> {
    Json(body)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/host", get(get_host_config).put(update_host_config))
        .route("/ui", get(get_ui_config).put(update_ui_config))
        .route("/naming", get(get_naming_config).put(update_naming_config))
        .route(
            "/mediaManagement",
            get(get_media_management_config).put(update_media_management_config),
        )
        .route(
            "/mediamanagement",
            get(get_media_management_config).put(update_media_management_config),
        ) // lowercase alias
        .route(
            "/indexer",
            get(get_indexer_config).put(update_indexer_config),
        )
        .route(
            "/downloadClient",
            get(get_download_client_config).put(update_download_client_config),
        )
        .route(
            "/downloadclient",
            get(get_download_client_config).put(update_download_client_config),
        ) // lowercase alias
}
