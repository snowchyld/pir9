//! Root Folder API endpoints

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

/// Input for creating a root folder
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
    #[serde(default)]
    pub relative_path: String,
}

/// GET /api/v3/rootfolder
pub async fn get_root_folders(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RootFolderResource>>, RootFolderError> {
    let repo = RootFolderRepository::new(state.db.clone());

    let db_folders = repo.get_all().await
        .map_err(|e| RootFolderError::Internal(format!("Failed to fetch root folders: {}", e)))?;

    let folders: Vec<RootFolderResource> = db_folders
        .into_iter()
        .map(|f| {
            let (accessible, free_space) = check_path_accessible(&f.path);
            RootFolderResource {
                id: f.id as i32,
                path: f.path,
                accessible,
                free_space,
                unmapped_folders: f.unmapped_folders
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
            }
        })
        .collect();

    Ok(Json(folders))
}

/// GET /api/v3/rootfolder/:id
pub async fn get_root_folder(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<RootFolderResource>, RootFolderError> {
    let repo = RootFolderRepository::new(state.db.clone());

    let folder = repo.get_by_id(id as i64).await
        .map_err(|e| RootFolderError::Internal(format!("Failed to fetch root folder: {}", e)))?
        .ok_or(RootFolderError::NotFound)?;

    let (accessible, free_space) = check_path_accessible(&folder.path);

    Ok(Json(RootFolderResource {
        id: folder.id as i32,
        path: folder.path,
        accessible,
        free_space,
        unmapped_folders: folder.unmapped_folders
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
    }))
}

/// POST /api/v3/rootfolder
pub async fn create_root_folder(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRootFolderRequest>,
) -> Result<Json<RootFolderResource>, RootFolderError> {
    // Validate path
    let path = body.path.trim();
    if path.is_empty() {
        return Err(RootFolderError::Validation("Path is required".to_string()));
    }

    // Check if path exists
    let (accessible, free_space) = check_path_accessible(path);

    // Insert into database
    let repo = RootFolderRepository::new(state.db.clone());
    let id = repo.insert(path).await
        .map_err(|e| RootFolderError::Internal(format!("Failed to create root folder: {}", e)))?;

    tracing::info!("Created root folder: id={}, path={}", id, path);

    Ok(Json(RootFolderResource {
        id: id as i32,
        path: path.to_string(),
        accessible,
        free_space,
        unmapped_folders: body.unmapped_folders.unwrap_or_default(),
    }))
}

/// DELETE /api/v3/rootfolder/:id
pub async fn delete_root_folder(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, RootFolderError> {
    let repo = RootFolderRepository::new(state.db.clone());

    repo.delete(id as i64).await
        .map_err(|e| RootFolderError::Internal(format!("Failed to delete root folder: {}", e)))?;

    tracing::info!("Deleted root folder: id={}", id);

    Ok(Json(serde_json::json!({})))
}

fn check_path_accessible(path: &str) -> (bool, i64) {
    use std::path::Path;
    let p = Path::new(path);
    if p.exists() && p.is_dir() {
        // Try to get free space
        let free_space = get_free_space(path).unwrap_or(0);
        (true, free_space)
    } else {
        (false, 0)
    }
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
            RootFolderError::NotFound => (StatusCode::NOT_FOUND, "Root folder not found".to_string()),
            RootFolderError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            RootFolderError::Internal(msg) => {
                tracing::error!("Root folder error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
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
