#![allow(dead_code, unused_imports)]
//! System API endpoints

use axum::{
    extract::Query,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::{CommandRepository, RootFolderRepository};
use crate::core::scanner::consumer::RunningJobInfo;
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(get_status))
        .route("/health", get(get_health))
        .route("/diskspace", get(get_disk_space))
        .route("/task/running", get(get_running_tasks))
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
    let db_type = state.config.read().database.database_type.clone();
    let is_docker = std::path::Path::new("/.dockerenv").exists() || std::env::var("DOCKER").is_ok();

    // Read OS pretty name from /etc/os-release (Linux), fall back to std::env::consts::OS
    let (os_name, os_version) = get_os_info();

    let db_version = sqlx::query_scalar::<_, String>("SHOW server_version")
        .fetch_one(state.db.pool())
        .await
        .unwrap_or_default();

    Json(SystemStatus {
        app_name: "pir9".to_string(),
        instance_name: "pir9".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_time: chrono::Utc::now().to_rfc3339(),
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
        is_docker,
        is_linux: cfg!(target_os = "linux"),
        is_osx: cfg!(target_os = "macos"),
        is_windows: cfg!(target_os = "windows"),
        mode: "console".to_string(),
        branch: "develop".to_string(),
        authentication: "none".to_string(),
        database_type: db_type,
        database_version: db_version,
        migration_version: 1,
        url_base: String::new(),
        runtime_version: env!("RUSTC_VERSION").to_string(),
        runtime_name: "Rust".to_string(),
        start_time: chrono::Utc::now().to_rfc3339(),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        package_author: "pir9".to_string(),
        package_update_mechanism: "builtIn".to_string(),
    })
}

/// GET /api/v5/system/task/running - Combined running commands + scan jobs
async fn get_running_tasks(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<Vec<RunningTask>> {
    let mut tasks = Vec::new();

    // Running commands from DB
    let repo = CommandRepository::new(state.db.clone());
    if let Ok(commands) = repo.get_all().await {
        for cmd in commands {
            if cmd.status == "queued" || cmd.status == "started" {
                tasks.push(RunningTask {
                    id: cmd.id.to_string(),
                    task_type: "command".to_string(),
                    name: cmd.name.clone(),
                    status: cmd.status.clone(),
                    started: cmd.started.map(|t| t.to_rfc3339()),
                    message: cmd.message,
                    detail: None,
                });
            }
        }
    }

    // Running scan jobs from worker consumer
    if let Some(consumer) = state.scan_result_consumer.get() {
        let jobs = consumer.get_running_jobs().await;
        for job in jobs {
            let name = match job.scan_type {
                crate::core::messaging::ScanType::RescanSeries => "Scan Series".to_string(),
                crate::core::messaging::ScanType::RescanMovie => "Scan Movie".to_string(),
                crate::core::messaging::ScanType::DownloadedEpisodesScan => {
                    "Scan Downloads".to_string()
                }
                crate::core::messaging::ScanType::RescanPodcast => "Scan Podcasts".to_string(),
                crate::core::messaging::ScanType::RescanMusic => "Scan Music".to_string(),
            };
            let total = job.entity_ids.len();
            let detail = if job.results_received > 0 {
                if total > 1 {
                    Some(format!("{}/{} done", job.results_received, total))
                } else {
                    Some("Processing...".to_string())
                }
            } else {
                Some("Scanning...".to_string())
            };
            tasks.push(RunningTask {
                id: job.job_id,
                task_type: "scan".to_string(),
                name,
                status: "started".to_string(),
                started: None,
                message: None,
                detail,
            });
        }
    }

    Json(tasks)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningTask {
    pub id: String,
    pub task_type: String,
    pub name: String,
    pub status: String,
    pub started: Option<String>,
    pub message: Option<String>,
    pub detail: Option<String>,
}

async fn get_health() -> Json<Vec<HealthCheck>> {
    Json(vec![HealthCheck {
        source: "DownloadClient".to_string(),
        health_type: HealthType::Ok,
        message: None,
        wiki_url: None,
    }])
}

async fn get_disk_space(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<Vec<DiskSpace>> {
    let mut disk_spaces = Vec::new();
    let mut seen_devs = std::collections::HashSet::new();

    // Query root folders from the database for configured paths
    let repo = RootFolderRepository::new(state.db.clone());
    if let Ok(folders) = repo.get_all().await {
        for folder in &folders {
            if let Some(ds) = get_statvfs_info(&folder.path) {
                let dev_key = (ds.total_space, ds.free_space);
                if seen_devs.insert(dev_key) {
                    disk_spaces.push(DiskSpace {
                        path: folder.path.clone(),
                        label: folder.path.clone(),
                        free_space: ds.free_space,
                        total_space: ds.total_space,
                    });
                }
            }
        }
    }

    // Always include root filesystem as fallback
    if let Some(ds) = get_statvfs_info("/") {
        let dev_key = (ds.total_space, ds.free_space);
        if seen_devs.insert(dev_key) {
            disk_spaces.push(DiskSpace {
                path: "/".to_string(),
                label: "/".to_string(),
                free_space: ds.free_space,
                total_space: ds.total_space,
            });
        }
    }

    Json(disk_spaces)
}

struct FsStats {
    free_space: i64,
    total_space: i64,
}

fn get_statvfs_info(path: &str) -> Option<FsStats> {
    use std::ffi::CString;
    let c_path = CString::new(path).ok()?;
    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &mut stat) == 0 {
            let block_size = stat.f_frsize as i64;
            Some(FsStats {
                free_space: stat.f_bavail as i64 * block_size,
                total_space: stat.f_blocks as i64 * block_size,
            })
        } else {
            None
        }
    }
}

async fn list_backups(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<Vec<Backup>> {
    let backup_dir = state.config.read().paths.backup_dir.clone();
    let mut backups = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&backup_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("zip")
                || path.extension().and_then(|e| e.to_str()) == Some("sql")
            {
                if let Ok(meta) = entry.metadata() {
                    let modified = meta
                        .modified()
                        .map(chrono::DateTime::<chrono::Utc>::from)
                        .unwrap_or_else(|_| chrono::Utc::now());
                    backups.push(Backup {
                        name: entry.file_name().to_string_lossy().to_string(),
                        path: path.to_string_lossy().to_string(),
                        size: meta.len() as i64,
                        time: modified,
                    });
                }
            }
        }
    }

    backups.sort_by(|a, b| b.time.cmp(&a.time));
    Json(backups)
}

async fn create_backup(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<Backup> {
    let (backup_dir, conn) = {
        let cfg = state.config.read();
        (
            cfg.paths.backup_dir.clone(),
            cfg.database.connection_string.clone(),
        )
    };
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let filename = format!("pir9_backup_{}.sql", timestamp);
    let filepath = backup_dir.join(&filename);

    // Ensure backup directory exists
    let _ = std::fs::create_dir_all(&backup_dir);

    // Run pg_dump if database connection string is available
    let conn = &conn;
    let result = tokio::process::Command::new("pg_dump")
        .arg(conn)
        .arg("--no-owner")
        .arg("--no-acl")
        .arg("-f")
        .arg(&filepath)
        .output()
        .await;

    let size = match result {
        Ok(output) if output.status.success() => {
            tracing::info!("Backup created: {}", filepath.display());
            std::fs::metadata(&filepath)
                .map(|m| m.len() as i64)
                .unwrap_or(0)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("pg_dump failed: {}", stderr);
            0
        }
        Err(e) => {
            tracing::error!("Failed to run pg_dump: {}", e);
            0
        }
    };

    Json(Backup {
        name: filename,
        path: filepath.to_string_lossy().to_string(),
        size,
        time: chrono::Utc::now(),
    })
}

async fn restore_backup(Json(_request): Json<RestoreBackupRequest>) -> Json<BackupActionResponse> {
    // Restore is complex and potentially destructive — left as a no-op
    Json(BackupActionResponse { success: true })
}

async fn get_logs(Query(_params): Query<LogQuery>) -> Json<Vec<LogEntry>> {
    Json(vec![])
}

async fn get_update_info() -> Json<UpdateInfo> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    // Check GitHub releases for a newer version
    let latest_version = check_latest_release().await;
    let update_available = latest_version
        .as_ref()
        .map(|latest| is_newer_version(&current_version, latest))
        .unwrap_or(false);

    Json(UpdateInfo {
        version: latest_version.unwrap_or_else(|| current_version.clone()),
        branch: "main".to_string(),
        update_available,
    })
}

/// Check GitHub releases for the latest version
async fn check_latest_release() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("pir9")
        .build()
        .ok()?;

    let resp = client
        .get("https://api.github.com/repos/pir9/pir9/releases/latest")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json["tag_name"].as_str()?;
    // Strip leading 'v' if present
    Some(tag.trim_start_matches('v').to_string())
}

/// Compare semver strings: true if `latest` is newer than `current`
fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v.split('.').filter_map(|s| s.parse().ok()).collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };

    let c = parse(current);
    let l = parse(latest);
    l > c
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
