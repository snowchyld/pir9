//! Disk Space API endpoints

use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::core::datastore::repositories::RootFolderRepository;
use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpaceResource {
    pub path: String,
    pub label: String,
    pub free_space: i64,
    pub total_space: i64,
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

/// GET /api/v3/diskspace
pub async fn get_disk_space(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<DiskSpaceResource>> {
    let mut disk_spaces = Vec::new();
    let mut seen_devs = std::collections::HashSet::new();

    let repo = RootFolderRepository::new(state.db.clone());
    if let Ok(folders) = repo.get_all().await {
        for folder in &folders {
            if let Some(ds) = get_statvfs_info(&folder.path) {
                let dev_key = (ds.total_space, ds.free_space);
                if seen_devs.insert(dev_key) {
                    disk_spaces.push(DiskSpaceResource {
                        path: folder.path.clone(),
                        label: folder.path.clone(),
                        free_space: ds.free_space,
                        total_space: ds.total_space,
                    });
                }
            }
        }
    }

    if let Some(ds) = get_statvfs_info("/") {
        let dev_key = (ds.total_space, ds.free_space);
        if seen_devs.insert(dev_key) {
            disk_spaces.push(DiskSpaceResource {
                path: "/".to_string(),
                label: "/".to_string(),
                free_space: ds.free_space,
                total_space: ds.total_space,
            });
        }
    }

    Json(disk_spaces)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_disk_space))
}
