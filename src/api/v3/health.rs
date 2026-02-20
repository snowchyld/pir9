//! Health API endpoints

use axum::{extract::State, response::Json, routing::get, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::core::datastore::repositories::RootFolderRepository;
use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResource {
    pub source: String,
    #[serde(rename = "type")]
    pub health_type: String,
    pub message: String,
    pub wiki_url: Option<String>,
}

/// GET /api/v3/health
pub async fn get_health(State(state): State<Arc<AppState>>) -> Json<Vec<HealthResource>> {
    let mut issues = Vec::new();

    // Check disk space on root folders
    let repo = RootFolderRepository::new(state.db.clone());
    if let Ok(folders) = repo.get_all().await {
        for folder in &folders {
            if let Some((free, total)) = get_disk_space(&folder.path) {
                if total > 0 {
                    let free_pct = (free as f64 / total as f64) * 100.0;
                    if free_pct < 2.0 {
                        issues.push(HealthResource {
                            source: "DiskSpace".to_string(),
                            health_type: "error".to_string(),
                            message: format!(
                                "Disk space critically low on '{}': {:.1}% free",
                                folder.path, free_pct
                            ),
                            wiki_url: None,
                        });
                    } else if free_pct < 5.0 {
                        issues.push(HealthResource {
                            source: "DiskSpace".to_string(),
                            health_type: "warning".to_string(),
                            message: format!(
                                "Disk space low on '{}': {:.1}% free",
                                folder.path, free_pct
                            ),
                            wiki_url: None,
                        });
                    }
                }
            }
        }
    }

    Json(issues)
}

fn get_disk_space(path: &str) -> Option<(i64, i64)> {
    use std::ffi::CString;
    let c_path = CString::new(path).ok()?;
    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &mut stat) == 0 {
            let block_size = stat.f_frsize as i64;
            Some((
                stat.f_bavail as i64 * block_size,
                stat.f_blocks as i64 * block_size,
            ))
        } else {
            None
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_health))
}
