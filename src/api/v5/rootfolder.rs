//! Root Folder API endpoints (v5)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::RootFolderRepository;
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootFolderResource {
    pub id: i32,
    pub path: String,
    pub accessible: bool,
    pub free_space: i64,
    #[serde(default)]
    pub unmapped_folders: Vec<UnmappedFolderResource>,
}

/// Input for creating a root folder (id is optional/generated)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CreateRootFolderRequest {
    #[serde(default)]
    pub id: Option<i32>,
    pub path: String,
    #[serde(default)]
    pub accessible: Option<bool>,
    #[serde(default)]
    pub free_space: Option<i64>,
    #[serde(default)]
    pub unmapped_folders: Option<Vec<UnmappedFolderResource>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnmappedFolderResource {
    pub name: String,
    pub path: String,
    pub relative_path: String,
}

pub async fn get_root_folders(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RootFolderResource>>, RootFolderError> {
    let repo = RootFolderRepository::new(state.db.clone());

    let db_folders = repo
        .get_all()
        .await
        .map_err(|e| RootFolderError::Internal(format!("Failed to fetch root folders: {}", e)))?;

    // Get all series + movie paths to determine which folders are unmapped
    let series_paths = get_series_paths(&state.db).await.unwrap_or_default();
    let movie_paths = get_movie_paths(&state.db).await.unwrap_or_default();
    let mut mapped_paths = series_paths;
    mapped_paths.extend(movie_paths);
    let mapped_paths = Arc::new(mapped_paths);

    // Scan each root folder in spawn_blocking to avoid blocking the async
    // runtime on NFS/network filesystem I/O
    let mut tasks = Vec::new();
    for f in db_folders {
        let mapped = mapped_paths.clone();
        tasks.push(tokio::task::spawn_blocking(move || {
            let (accessible, free_space) = check_path_accessible(&f.path);
            let unmapped_folders = if accessible {
                scan_unmapped_folders(&f.path, &mapped)
            } else {
                vec![]
            };
            RootFolderResource {
                id: f.id as i32,
                path: f.path,
                accessible,
                free_space,
                unmapped_folders,
            }
        }));
    }

    let mut folders = Vec::new();
    for task in tasks {
        match task.await {
            Ok(resource) => folders.push(resource),
            Err(e) => tracing::warn!("Root folder scan task failed: {}", e),
        }
    }

    Ok(Json(folders))
}

pub async fn get_root_folder(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<RootFolderResource>, RootFolderError> {
    let repo = RootFolderRepository::new(state.db.clone());

    let folder = repo
        .get_by_id(id as i64)
        .await
        .map_err(|e| RootFolderError::Internal(format!("Failed to fetch root folder: {}", e)))?
        .ok_or(RootFolderError::NotFound)?;

    // Get series + movie paths for unmapped folder detection
    let series_paths = get_series_paths(&state.db).await.unwrap_or_default();
    let movie_paths = get_movie_paths(&state.db).await.unwrap_or_default();
    let mut mapped_paths = series_paths;
    mapped_paths.extend(movie_paths);

    let resource = tokio::task::spawn_blocking(move || {
        let (accessible, free_space) = check_path_accessible(&folder.path);
        let unmapped_folders = if accessible {
            scan_unmapped_folders(&folder.path, &mapped_paths)
        } else {
            vec![]
        };
        RootFolderResource {
            id: folder.id as i32,
            path: folder.path,
            accessible,
            free_space,
            unmapped_folders,
        }
    })
    .await
    .map_err(|e| RootFolderError::Internal(format!("Scan task failed: {}", e)))?;

    Ok(Json(resource))
}

pub async fn create_root_folder(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRootFolderRequest>,
) -> Result<Json<RootFolderResource>, RootFolderError> {
    // Validate path
    let path = body.path.trim().to_string();
    if path.is_empty() {
        return Err(RootFolderError::Validation("Path is required".to_string()));
    }

    // Check if path exists (spawn_blocking for NFS safety)
    let path_check = path.clone();
    let (accessible, free_space) =
        tokio::task::spawn_blocking(move || check_path_accessible(&path_check))
            .await
            .map_err(|e| RootFolderError::Internal(format!("Check task failed: {}", e)))?;

    if !accessible {
        return Err(RootFolderError::Validation(format!(
            "Path '{}' does not exist or is not accessible",
            path
        )));
    }

    // Insert into database
    let repo = RootFolderRepository::new(state.db.clone());
    let id = repo
        .insert(&path)
        .await
        .map_err(|e| RootFolderError::Internal(format!("Failed to create root folder: {}", e)))?;

    tracing::info!("Created root folder: id={}, path={}", id, path);

    // Scan for unmapped folders
    let series_paths = get_series_paths(&state.db).await.unwrap_or_default();
    let movie_paths = get_movie_paths(&state.db).await.unwrap_or_default();
    let mut mapped_paths = series_paths;
    mapped_paths.extend(movie_paths);
    let path_scan = path.clone();
    let unmapped_folders =
        tokio::task::spawn_blocking(move || scan_unmapped_folders(&path_scan, &mapped_paths))
            .await
            .unwrap_or_default();

    Ok(Json(RootFolderResource {
        id: id as i32,
        path,
        accessible,
        free_space,
        unmapped_folders,
    }))
}

fn check_path_accessible(path: &str) -> (bool, i64) {
    use std::path::Path;
    let p = Path::new(path);
    if p.exists() && p.is_dir() {
        let free_space = get_free_space(path).unwrap_or(0);
        (true, free_space)
    } else {
        (false, 0)
    }
}

/// Scan a root folder for unmapped subfolders (potential series to import)
fn scan_unmapped_folders(root_path: &str, series_paths: &[String]) -> Vec<UnmappedFolderResource> {
    use std::fs;
    use std::path::Path;

    let root = Path::new(root_path);
    let mut unmapped = Vec::new();

    // Read immediate subdirectories
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Only consider directories
            if !path.is_dir() {
                continue;
            }

            // Skip hidden directories (starting with .)
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if !n.starts_with('.') => n.to_string(),
                _ => continue,
            };

            // Skip common non-series folders
            let lower_name = name.to_lowercase();
            if lower_name == "lost+found"
                || lower_name == "@eadir"
                || lower_name == ".@__thumb"
                || lower_name == "#recycle"
                || lower_name.starts_with("$")
            {
                continue;
            }

            let full_path = path.to_string_lossy().to_string();

            // Check if this folder is already mapped to a series
            let is_mapped = series_paths.iter().any(|sp| {
                // Normalize paths for comparison
                let sp_normalized = sp.trim_end_matches('/');
                let fp_normalized = full_path.trim_end_matches('/');
                sp_normalized.eq_ignore_ascii_case(fp_normalized)
            });

            if !is_mapped {
                // Relative path is just the folder name for immediate subdirectories
                let relative_path = name.clone();
                unmapped.push(UnmappedFolderResource {
                    name: name.clone(),
                    path: full_path,
                    relative_path,
                });
            }
        }
    }

    // Sort by name for consistent ordering
    unmapped.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    unmapped
}

/// Get all series paths from the database
async fn get_series_paths(db: &crate::core::datastore::Database) -> anyhow::Result<Vec<String>> {
    use sqlx::Row;

    let pool = db.pool();
    let rows = sqlx::query("SELECT path FROM series")
        .fetch_all(pool)
        .await?;

    let paths: Vec<String> = rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("path").ok())
        .collect();

    Ok(paths)
}

/// Get all movie paths from the database
async fn get_movie_paths(db: &crate::core::datastore::Database) -> anyhow::Result<Vec<String>> {
    use sqlx::Row;

    let pool = db.pool();
    let rows = sqlx::query("SELECT path FROM movies")
        .fetch_all(pool)
        .await?;

    let paths: Vec<String> = rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("path").ok())
        .collect();

    Ok(paths)
}

/// Get free space for a path (platform-specific)
fn get_free_space(path: &str) -> Option<i64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::raw::c_char;

        #[repr(C)]
        struct StatVfs {
            f_bsize: u64,
            f_frsize: u64,
            f_blocks: u64,
            f_bfree: u64,
            f_bavail: u64,
            f_files: u64,
            f_ffree: u64,
            f_favail: u64,
            f_fsid: u64,
            f_flag: u64,
            f_namemax: u64,
            __f_spare: [i32; 6],
        }

        extern "C" {
            fn statvfs(path: *const c_char, buf: *mut StatVfs) -> i32;
        }

        let c_path = CString::new(path).ok()?;
        let mut stat: StatVfs = unsafe { std::mem::zeroed() };

        let result = unsafe { statvfs(c_path.as_ptr(), &mut stat) };
        if result == 0 {
            Some((stat.f_bavail * stat.f_frsize) as i64)
        } else {
            None
        }
    }

    #[cfg(not(unix))]
    {
        None
    }
}

pub async fn delete_root_folder(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, RootFolderError> {
    let repo = RootFolderRepository::new(state.db.clone());

    repo.delete(id as i64)
        .await
        .map_err(|e| RootFolderError::Internal(format!("Failed to delete root folder: {}", e)))?;

    tracing::info!("Deleted root folder: id={}", id);

    Ok(Json(serde_json::json!({})))
}

/// Error type for root folder operations
#[derive(Debug)]
pub enum RootFolderError {
    NotFound,
    Validation(String),
    Internal(String),
}

impl IntoResponse for RootFolderError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            RootFolderError::NotFound => {
                (StatusCode::NOT_FOUND, "Root folder not found".to_string())
            }
            RootFolderError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            RootFolderError::Internal(msg) => {
                tracing::error!("Root folder error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_root_folders).post(create_root_folder))
        .route("/{id}", get(get_root_folder).delete(delete_root_folder))
}
