#![allow(dead_code, unused_imports)]
//! System API endpoints

use axum::{
    extract::{Path, Query},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::{CommandRepository, RootFolderRepository};
use crate::core::scanner::consumer::{RunningJobInfo, ScanProgressInfo};
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(get_status))
        .route("/health", get(get_health))
        .route("/diskspace", get(get_disk_space))
        .route("/task/running", get(get_running_tasks))
        .route("/task/scan/{id}", delete(cancel_scan_job))
        .route("/task/imdb", delete(cancel_imdb_sync))
        .route("/backup", get(list_backups).post(create_backup))
        .route("/backup/restore", post(restore_backup))
        .route("/logs", get(get_logs))
        .route("/update", get(get_update_info).post(trigger_update))
        .route("/restart", post(restart))
        .route("/shutdown", post(shutdown))
        .route("/queue/redis", get(get_redis_queue))
        .route("/queue/redis/prune", post(prune_redis_queue))
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
                    worker_id: None,
                    progress: None,
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
                crate::core::messaging::ScanType::DownloadedMovieScan => "Import Movie".to_string(),
                crate::core::messaging::ScanType::RescanPodcast => "Scan Podcasts".to_string(),
                crate::core::messaging::ScanType::RescanMusic => "Scan Music".to_string(),
                crate::core::messaging::ScanType::RescanAudiobook => "Scan Audiobooks".to_string(),
            };
            let total = job.entity_ids.len();
            let has_worker = job.worker_id.is_some();
            let status = if has_worker { "started" } else { "queued" };
            let detail = if let Some(ref prog) = job.progress {
                Some(format!(
                    "{} {}/{} ({:.1}%)",
                    match prog.stage.as_str() {
                        "scanning" => "Discovering files...",
                        "probing" => "Probing",
                        "hashing" => "Hashing",
                        "enriching" => "Enriching",
                        "copying" => "Importing",
                        _ => &prog.stage,
                    },
                    prog.files_processed,
                    prog.files_total,
                    prog.percent
                ))
            } else if job.results_received > 0 {
                if total > 1 {
                    Some(format!("{}/{} done", job.results_received, total))
                } else {
                    Some("Processing...".to_string())
                }
            } else if has_worker {
                Some("Scanning...".to_string())
            } else {
                Some("Waiting for worker...".to_string())
            };
            tasks.push(RunningTask {
                id: job.job_id,
                task_type: "scan".to_string(),
                name,
                status: status.to_string(),
                started: job.started_at,
                message: None,
                detail,
                worker_id: job.worker_id,
                progress: job.progress,
            });
        }
    }

    // IMDB sync status from pir9-imdb microservice
    if state.imdb_client.is_enabled() {
        if let Ok(resp) = state.imdb_client.get_sync_status().await {
            if resp.status == 200 {
                if let Ok(sync_status) = serde_json::from_value::<ImdbSyncStatus>(resp.body) {
                    if sync_status.is_running {
                        // Find the active dataset for detail text
                        let datasets = [
                            &sync_status.title_basics,
                            &sync_status.title_episodes,
                            &sync_status.title_ratings,
                            &sync_status.name_basics,
                            &sync_status.title_principals,
                        ];
                        let active = datasets
                            .iter()
                            .find_map(|d| d.as_ref().filter(|ds| ds.is_running));
                        let detail = active.map(|ds| {
                            let name = ds
                                .dataset_name
                                .trim_end_matches(".tsv.gz")
                                .replace('.', " ");
                            if ds.rows_processed > 0 {
                                format!(
                                    "{}: {} rows processed ({} inserted, {} updated)",
                                    name, ds.rows_processed, ds.rows_inserted, ds.rows_updated
                                )
                            } else {
                                format!("{}: starting...", name)
                            }
                        });
                        let started = active.map(|ds| ds.started_at.clone());
                        tasks.push(RunningTask {
                            id: "imdb-sync".to_string(),
                            task_type: "imdb".to_string(),
                            name: "IMDB Sync".to_string(),
                            status: "started".to_string(),
                            started,
                            message: None,
                            detail,
                            worker_id: None,
                            progress: None,
                        });
                    }
                }
            }
        }
    }

    Json(tasks)
}

/// Deserialization types for IMDB sync status response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImdbSyncStatus {
    #[serde(default)]
    is_running: bool,
    title_basics: Option<ImdbDatasetStatus>,
    title_episodes: Option<ImdbDatasetStatus>,
    title_ratings: Option<ImdbDatasetStatus>,
    name_basics: Option<ImdbDatasetStatus>,
    title_principals: Option<ImdbDatasetStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImdbDatasetStatus {
    dataset_name: String,
    rows_processed: i64,
    rows_inserted: i64,
    rows_updated: i64,
    started_at: String,
    #[serde(default)]
    is_running: bool,
}

/// DELETE /api/v5/system/task/imdb - Cancel IMDB sync
async fn cancel_imdb_sync(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::http::StatusCode {
    if !state.imdb_client.is_enabled() {
        return axum::http::StatusCode::NOT_FOUND;
    }
    match state.imdb_client.cancel_sync().await {
        Ok(resp) if resp.status == 200 => {
            tracing::info!("Cancelled IMDB sync via system task API");
            axum::http::StatusCode::OK
        }
        _ => axum::http::StatusCode::NOT_FOUND,
    }
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
    pub worker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<ScanProgressInfo>,
}

/// DELETE /api/v5/system/task/scan/{id} - Cancel a running scan job
async fn cancel_scan_job(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> axum::http::StatusCode {
    if let Some(consumer) = state.scan_result_consumer.get() {
        if consumer.cancel_job(&job_id).await {
            tracing::info!("Cancelled scan job via API: {}", job_id);
            return axum::http::StatusCode::OK;
        }
    }
    axum::http::StatusCode::NOT_FOUND
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
        .get("https://api.github.com/repos/snowchyld/pir9/releases/latest")
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

// ============================================================================
// Redis Stream Queue Endpoints
// ============================================================================

/// GET /api/v5/system/queue/redis — View Redis stream status
#[cfg(feature = "redis-events")]
async fn get_redis_queue(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<RedisQueueStatus> {
    use crate::core::messaging::{
        REDIS_JOB_STREAM, REDIS_RESULT_STREAM, REDIS_SERVER_GROUP, REDIS_WORKER_GROUP,
    };

    let Some(ref hybrid_bus) = state.hybrid_event_bus else {
        return Json(RedisQueueStatus {
            enabled: false,
            ..Default::default()
        });
    };

    let Some(redis_url) = hybrid_bus.redis_url() else {
        return Json(RedisQueueStatus {
            enabled: false,
            ..Default::default()
        });
    };
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            return Json(RedisQueueStatus {
                enabled: true,
                error: Some(format!("Failed to connect: {}", e)),
                ..Default::default()
            })
        }
    };

    let mut conn = match redis::aio::ConnectionManager::new(client).await {
        Ok(c) => c,
        Err(e) => {
            return Json(RedisQueueStatus {
                enabled: true,
                error: Some(format!("Connection failed: {}", e)),
                ..Default::default()
            })
        }
    };

    let jobs = query_stream_info(&mut conn, REDIS_JOB_STREAM, REDIS_WORKER_GROUP).await;
    let results = query_stream_info(&mut conn, REDIS_RESULT_STREAM, REDIS_SERVER_GROUP).await;

    Json(RedisQueueStatus {
        enabled: true,
        error: None,
        jobs,
        results,
    })
}

#[cfg(not(feature = "redis-events"))]
async fn get_redis_queue() -> Json<RedisQueueStatus> {
    Json(RedisQueueStatus {
        enabled: false,
        ..Default::default()
    })
}

/// POST /api/v5/system/queue/redis/prune — ACK all pending entries and trim streams
#[cfg(feature = "redis-events")]
async fn prune_redis_queue(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<RedisQueuePruneResult> {
    use crate::core::messaging::{
        REDIS_JOB_STREAM, REDIS_RESULT_STREAM, REDIS_SERVER_GROUP, REDIS_WORKER_GROUP,
    };

    let Some(ref hybrid_bus) = state.hybrid_event_bus else {
        return Json(RedisQueuePruneResult {
            success: false,
            error: Some("Redis not enabled".to_string()),
            ..Default::default()
        });
    };

    let Some(redis_url) = hybrid_bus.redis_url() else {
        return Json(RedisQueuePruneResult {
            success: false,
            error: Some("Redis not connected".to_string()),
            ..Default::default()
        });
    };
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            return Json(RedisQueuePruneResult {
                success: false,
                error: Some(format!("Failed to connect: {}", e)),
                ..Default::default()
            })
        }
    };

    let mut conn = match redis::aio::ConnectionManager::new(client).await {
        Ok(c) => c,
        Err(e) => {
            return Json(RedisQueuePruneResult {
                success: false,
                error: Some(format!("Connection failed: {}", e)),
                ..Default::default()
            })
        }
    };

    let jobs_acked = ack_all_pending(&mut conn, REDIS_JOB_STREAM, REDIS_WORKER_GROUP).await;
    let results_acked = ack_all_pending(&mut conn, REDIS_RESULT_STREAM, REDIS_SERVER_GROUP).await;

    // Trim streams to a reasonable length
    let _: redis::RedisResult<i64> = redis::cmd("XTRIM")
        .arg(REDIS_JOB_STREAM)
        .arg("MAXLEN")
        .arg("~")
        .arg(100)
        .query_async(&mut conn)
        .await;
    let _: redis::RedisResult<i64> = redis::cmd("XTRIM")
        .arg(REDIS_RESULT_STREAM)
        .arg("MAXLEN")
        .arg("~")
        .arg(100)
        .query_async(&mut conn)
        .await;

    Json(RedisQueuePruneResult {
        success: true,
        error: None,
        jobs_acked,
        results_acked,
    })
}

#[cfg(not(feature = "redis-events"))]
async fn prune_redis_queue() -> Json<RedisQueuePruneResult> {
    Json(RedisQueuePruneResult {
        success: false,
        error: Some("Redis not enabled".to_string()),
        ..Default::default()
    })
}

/// Query XLEN + XPENDING + XINFO CONSUMERS for a stream
#[cfg(feature = "redis-events")]
async fn query_stream_info(
    conn: &mut redis::aio::ConnectionManager,
    stream: &str,
    group: &str,
) -> RedisStreamInfo {
    let length: i64 = redis::cmd("XLEN")
        .arg(stream)
        .query_async(conn)
        .await
        .unwrap_or(0);

    // XPENDING <stream> <group> returns [total_pending, min_id, max_id, [[consumer, count], ...]]
    let pending_info: redis::RedisResult<redis::Value> = redis::cmd("XPENDING")
        .arg(stream)
        .arg(group)
        .query_async(conn)
        .await;

    let (total_pending, consumers) = match pending_info {
        Ok(redis::Value::Array(ref arr)) if arr.len() >= 4 => {
            let total: i64 = redis::FromRedisValue::from_redis_value(arr[0].clone()).unwrap_or(0);
            let consumer_list = match &arr[3] {
                redis::Value::Array(pairs) => pairs
                    .iter()
                    .filter_map(|pair| {
                        if let redis::Value::Array(kv) = pair {
                            if kv.len() >= 2 {
                                let name: String =
                                    redis::FromRedisValue::from_redis_value(kv[0].clone()).ok()?;
                                let count_str: String =
                                    redis::FromRedisValue::from_redis_value(kv[1].clone()).ok()?;
                                let count: i64 = count_str.parse().unwrap_or(0);
                                return Some(RedisConsumerInfo {
                                    name,
                                    pending: count,
                                });
                            }
                        }
                        None
                    })
                    .collect(),
                _ => vec![],
            };
            (total, consumer_list)
        }
        _ => (0, vec![]),
    };

    RedisStreamInfo {
        stream: stream.to_string(),
        group: group.to_string(),
        length,
        total_pending,
        consumers,
    }
}

/// ACK all pending entries in a stream group
#[cfg(feature = "redis-events")]
async fn ack_all_pending(
    conn: &mut redis::aio::ConnectionManager,
    stream: &str,
    group: &str,
) -> i64 {
    // Get all pending entry IDs
    let pending: redis::RedisResult<redis::Value> = redis::cmd("XPENDING")
        .arg(stream)
        .arg(group)
        .arg("-")
        .arg("+")
        .arg(1000)
        .query_async(conn)
        .await;

    let ids: Vec<String> = match pending {
        Ok(redis::Value::Array(entries)) => entries
            .iter()
            .filter_map(|entry| {
                if let redis::Value::Array(fields) = entry {
                    if !fields.is_empty() {
                        return redis::FromRedisValue::from_redis_value(fields[0].clone()).ok();
                    }
                }
                None
            })
            .collect(),
        _ => vec![],
    };

    if ids.is_empty() {
        return 0;
    }

    let mut cmd = redis::cmd("XACK");
    cmd.arg(stream).arg(group);
    for id in &ids {
        cmd.arg(id.as_str());
    }

    let result: redis::RedisResult<i64> = cmd.query_async(conn).await;
    result.unwrap_or(0)
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct RedisQueueStatus {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    jobs: RedisStreamInfo,
    results: RedisStreamInfo,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct RedisStreamInfo {
    stream: String,
    group: String,
    length: i64,
    total_pending: i64,
    consumers: Vec<RedisConsumerInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RedisConsumerInfo {
    name: String,
    pending: i64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct RedisQueuePruneResult {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    jobs_acked: i64,
    results_acked: i64,
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
