//! System API endpoints

use axum::{
    Router,
    routing::{get, post},
    extract::Query,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(get_status))
        .route("/health", get(get_health))
        .route("/diskspace", get(get_disk_space))
        .route("/backup", get(list_backups).post(create_backup))
        .route("/backup/restore", post(restore_backup))
        .route("/logs", get(get_logs))
        .route("/update", get(get_update_info).post(trigger_update))
        .route("/restart", post(restart))
        .route("/shutdown", post(shutdown))
}

async fn get_status(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<SystemStatus> {
    let db_type = state.config.database.database_type.clone();
    let is_docker = std::path::Path::new("/.dockerenv").exists()
        || std::env::var("DOCKER").is_ok();

    Json(SystemStatus {
        app_name: "Pir9".to_string(),
        instance_name: "Pir9".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_time: chrono::Utc::now().to_rfc3339(),
        is_debug: cfg!(debug_assertions),
        is_production: !cfg!(debug_assertions),
        is_admin: false,
        is_user_interactive: false,
        startup_path: std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        app_data: "./config".to_string(),
        os_name: std::env::consts::OS.to_string(),
        os_version: "".to_string(),
        is_net_core: false,
        is_docker,
        is_linux: cfg!(target_os = "linux"),
        is_osx: cfg!(target_os = "macos"),
        is_windows: cfg!(target_os = "windows"),
        mode: "console".to_string(),
        branch: "develop".to_string(),
        authentication: "none".to_string(),
        database_type: db_type,
        database_version: String::new(),
        migration_version: 1,
        url_base: String::new(),
        runtime_version: "Rust".to_string(),
        runtime_name: "Rust".to_string(),
        start_time: chrono::Utc::now().to_rfc3339(),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        package_author: "pir9".to_string(),
        package_update_mechanism: "builtIn".to_string(),
    })
}

async fn get_health() -> Json<Vec<HealthCheck>> {
    Json(vec![
        HealthCheck {
            source: "DownloadClient".to_string(),
            health_type: HealthType::Ok,
            message: None,
            wiki_url: None,
        }
    ])
}

async fn get_disk_space() -> Json<Vec<DiskSpace>> {
    Json(vec![])
}

async fn list_backups() -> Json<Vec<Backup>> {
    Json(vec![])
}

async fn create_backup() -> Json<Backup> {
    Json(Backup {
        name: "backup.zip".to_string(),
        path: "/backups/backup.zip".to_string(),
        size: 0,
        time: chrono::Utc::now(),
    })
}

async fn restore_backup(
    Json(request): Json<RestoreBackupRequest>,
) -> Json<BackupActionResponse> {
    Json(BackupActionResponse { success: true })
}

async fn get_logs(
    Query(params): Query<LogQuery>,
) -> Json<Vec<LogEntry>> {
    Json(vec![])
}

async fn get_update_info() -> Json<UpdateInfo> {
    Json(UpdateInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        branch: "main".to_string(),
        update_available: false,
    })
}

async fn trigger_update() -> Json<UpdateActionResponse> {
    Json(UpdateActionResponse { success: true })
}

async fn restart() -> Json<SystemActionResponse> {
    Json(SystemActionResponse { success: true })
}

async fn shutdown() -> Json<SystemActionResponse> {
    Json(SystemActionResponse { success: true })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub app_name: String,
    pub instance_name: String,
    pub version: String,
    pub build_time: String,
    pub is_debug: bool,
    pub is_production: bool,
    pub is_admin: bool,
    pub is_user_interactive: bool,
    pub startup_path: String,
    pub app_data: String,
    pub os_name: String,
    pub os_version: String,
    pub is_net_core: bool,
    pub is_docker: bool,
    pub is_linux: bool,
    pub is_osx: bool,
    pub is_windows: bool,
    pub mode: String,
    pub branch: String,
    pub authentication: String,
    pub database_type: String,
    pub database_version: String,
    pub migration_version: i64,
    pub url_base: String,
    pub runtime_version: String,
    pub runtime_name: String,
    pub start_time: String,
    pub package_version: String,
    pub package_author: String,
    pub package_update_mechanism: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub source: String,
    pub health_type: HealthType,
    pub message: Option<String>,
    pub wiki_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthType {
    Ok,
    Notice,
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpace {
    pub path: String,
    pub label: String,
    pub free_space: i64,
    pub total_space: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub name: String,
    pub path: String,
    pub size: i64,
    pub time: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupRequest {
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupActionResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub level: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub logger: String,
    pub message: String,
    pub exception: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub branch: String,
    pub update_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateActionResponse {
    pub success: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemActionResponse {
    pub success: bool,
}
