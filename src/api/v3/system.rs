//! System API endpoints

use axum::{response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::{DateTime, Utc};

use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemResource {
    pub app_name: String,
    pub instance_name: String,
    pub version: String,
    pub build_time: DateTime<Utc>,
    pub is_debug: bool,
    pub is_production: bool,
    pub is_admin: bool,
    pub is_user_interactive: bool,
    pub startup_path: String,
    pub app_data: String,
    pub os_name: String,
    pub os_version: String,
    pub is_net_core: bool,
    pub is_linux: bool,
    pub is_osx: bool,
    pub is_windows: bool,
    pub is_docker: bool,
    pub mode: String,
    pub branch: String,
    pub authentication: String,
    pub database_type: String,
    pub database_version: String,
    pub migration_version: i32,
    pub url_base: String,
    pub runtime_version: String,
    pub runtime_name: String,
    pub start_time: DateTime<Utc>,
    pub package_version: String,
    pub package_author: String,
    pub package_update_mechanism: String,
}

/// Scheduled task resource
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTaskResource {
    pub id: i32,
    pub name: String,
    pub task_name: String,
    pub interval: i32,
    pub last_execution: Option<DateTime<Utc>>,
    pub last_start_time: Option<DateTime<Utc>>,
    pub next_execution: DateTime<Utc>,
    pub last_duration: String,
}

/// GET /api/v3/system/task
pub async fn get_tasks() -> Json<Vec<ScheduledTaskResource>> {
    let now = Utc::now();
    Json(vec![
        ScheduledTaskResource {
            id: 1,
            name: "Application Check Update".to_string(),
            task_name: "ApplicationCheckUpdate".to_string(),
            interval: 360,
            last_execution: Some(now),
            last_start_time: Some(now),
            next_execution: now + chrono::Duration::minutes(360),
            last_duration: "00:00:00.1234567".to_string(),
        },
        ScheduledTaskResource {
            id: 2,
            name: "Backup".to_string(),
            task_name: "Backup".to_string(),
            interval: 10080,
            last_execution: Some(now),
            last_start_time: Some(now),
            next_execution: now + chrono::Duration::minutes(10080),
            last_duration: "00:00:01.2345678".to_string(),
        },
        ScheduledTaskResource {
            id: 3,
            name: "Housekeeping".to_string(),
            task_name: "Housekeeping".to_string(),
            interval: 1440,
            last_execution: Some(now),
            last_start_time: Some(now),
            next_execution: now + chrono::Duration::minutes(1440),
            last_duration: "00:00:00.5678901".to_string(),
        },
        ScheduledTaskResource {
            id: 4,
            name: "Refresh Series".to_string(),
            task_name: "RefreshSeries".to_string(),
            interval: 720,
            last_execution: Some(now),
            last_start_time: Some(now),
            next_execution: now + chrono::Duration::minutes(720),
            last_duration: "00:00:05.1234567".to_string(),
        },
        ScheduledTaskResource {
            id: 5,
            name: "RSS Sync".to_string(),
            task_name: "RssSync".to_string(),
            interval: 15,
            last_execution: Some(now),
            last_start_time: Some(now),
            next_execution: now + chrono::Duration::minutes(15),
            last_duration: "00:00:02.3456789".to_string(),
        },
    ])
}

/// GET /api/v3/system/status
pub async fn get_status(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<SystemResource> {
    let db_type = state.config.database.database_type.clone();
    let is_docker = std::path::Path::new("/.dockerenv").exists()
        || std::env::var("DOCKER").is_ok();
    let (os_name, os_version) = get_os_info();

    Json(SystemResource {
        app_name: "Pir9".to_string(),
        instance_name: "Pir9".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_time: Utc::now(),
        is_debug: cfg!(debug_assertions),
        is_production: !cfg!(debug_assertions),
        is_admin: false,
        is_user_interactive: false,
        startup_path: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        app_data: "./config".to_string(),
        os_name,
        os_version,
        is_net_core: false,
        is_linux: cfg!(target_os = "linux"),
        is_osx: cfg!(target_os = "macos"),
        is_windows: cfg!(target_os = "windows"),
        is_docker,
        mode: "console".to_string(),
        branch: "develop".to_string(),
        authentication: "none".to_string(),
        database_type: db_type,
        database_version: String::new(),
        migration_version: 1,
        url_base: "".to_string(),
        runtime_version: env!("RUSTC_VERSION").to_string(),
        runtime_name: "Rust".to_string(),
        start_time: Utc::now(),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        package_author: "pir9".to_string(),
        package_update_mechanism: "builtIn".to_string(),
    })
}

/// Read OS name and version from /etc/os-release (Linux) or fall back to consts
fn get_os_info() -> (String, String) {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        let mut name = None;
        let mut version = None;
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                name = Some(val.trim_matches('"').to_string());
            } else if let Some(val) = line.strip_prefix("VERSION_ID=") {
                version = Some(val.trim_matches('"').to_string());
            }
        }
        (
            name.unwrap_or_else(|| std::env::consts::OS.to_string()),
            version.unwrap_or_default(),
        )
    } else {
        (std::env::consts::OS.to_string(), String::new())
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(get_status))
        .route("/task", get(get_tasks))
}
