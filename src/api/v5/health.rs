//! Health API endpoints (v5)
//! Implements real health checks for the application

use axum::{extract::State, response::Json, routing::get, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::core::datastore::repositories::{
    DownloadClientRepository, IndexerRepository, RootFolderRepository, SeriesRepository,
};
use crate::web::AppState;

/// Health check result types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthCheckType {
    Ok,
    Notice,
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResource {
    pub source: String,
    #[serde(rename = "type")]
    pub health_type: String,
    pub message: String,
    pub wiki_url: Option<String>,
}

impl HealthResource {
    fn new(source: &str, health_type: HealthCheckType, message: &str) -> Self {
        Self {
            source: source.to_string(),
            health_type: match health_type {
                HealthCheckType::Ok => "ok".to_string(),
                HealthCheckType::Notice => "notice".to_string(),
                HealthCheckType::Warning => "warning".to_string(),
                HealthCheckType::Error => "error".to_string(),
            },
            message: message.to_string(),
            wiki_url: None,
        }
    }

    fn with_wiki(mut self, url: &str) -> Self {
        self.wiki_url = Some(url.to_string());
        self
    }
}

pub async fn get_health(State(state): State<Arc<AppState>>) -> Json<Vec<HealthResource>> {
    let mut health_issues = Vec::new();

    // Check download clients
    check_download_clients(&state, &mut health_issues).await;

    // Check indexers
    check_indexers(&state, &mut health_issues).await;

    // Check root folders
    check_root_folders(&state, &mut health_issues).await;

    // Check for series without root folders
    check_series_root_folders(&state, &mut health_issues).await;

    // Check disk space
    check_disk_space(&state, &mut health_issues).await;

    Json(health_issues)
}

/// Check download client configuration
async fn check_download_clients(state: &AppState, issues: &mut Vec<HealthResource>) {
    let repo = DownloadClientRepository::new(state.db.clone());

    match repo.get_all().await {
        Ok(clients) => {
            let enabled_clients: Vec<_> = clients.iter().filter(|c| c.enable).collect();

            if enabled_clients.is_empty() {
                issues.push(
                    HealthResource::new(
                        "DownloadClientCheck",
                        HealthCheckType::Warning,
                        "No download client is available",
                    )
                    .with_wiki("https://wiki.servarr.com/sonarr/settings#download-clients"),
                );
            }

            // Check for clients without usenet or torrent protocol
            let has_usenet = enabled_clients.iter().any(|c| c.protocol == 1); // 1 = Usenet
            let has_torrent = enabled_clients.iter().any(|c| c.protocol == 2); // 2 = Torrent

            if !has_usenet && !has_torrent && !enabled_clients.is_empty() {
                // This shouldn't normally happen, but check anyway
                tracing::debug!("Download clients exist but have unknown protocol");
            }
        }
        Err(e) => {
            tracing::error!("Failed to check download clients: {}", e);
            issues.push(HealthResource::new(
                "DownloadClientCheck",
                HealthCheckType::Error,
                "Unable to check download clients",
            ));
        }
    }
}

/// Check indexer configuration
async fn check_indexers(state: &AppState, issues: &mut Vec<HealthResource>) {
    let repo = IndexerRepository::new(state.db.clone());

    match repo.get_all().await {
        Ok(indexers) => {
            let enabled_indexers: Vec<_> = indexers
                .iter()
                .filter(|i| i.enable_automatic_search || i.enable_rss)
                .collect();

            if enabled_indexers.is_empty() {
                issues.push(
                    HealthResource::new(
                        "IndexerCheck",
                        HealthCheckType::Warning,
                        "No indexers available with RSS sync or Automatic Search enabled",
                    )
                    .with_wiki("https://wiki.servarr.com/sonarr/settings#indexers"),
                );
            }

            // Check for indexers with RSS disabled
            let rss_enabled: Vec<_> = indexers.iter().filter(|i| i.enable_rss).collect();
            if rss_enabled.is_empty() && !indexers.is_empty() {
                issues.push(
                    HealthResource::new(
                        "IndexerRssCheck",
                        HealthCheckType::Warning,
                        "All indexers have RSS sync disabled",
                    )
                    .with_wiki("https://wiki.servarr.com/sonarr/settings#indexers"),
                );
            }

            // Check for indexers with search disabled
            let search_enabled: Vec<_> = indexers
                .iter()
                .filter(|i| i.enable_automatic_search || i.enable_interactive_search)
                .collect();
            if search_enabled.is_empty() && !indexers.is_empty() {
                issues.push(HealthResource::new(
                    "IndexerSearchCheck",
                    HealthCheckType::Warning,
                    "All indexers have search disabled",
                ));
            }
        }
        Err(e) => {
            tracing::error!("Failed to check indexers: {}", e);
            issues.push(HealthResource::new(
                "IndexerCheck",
                HealthCheckType::Error,
                "Unable to check indexers",
            ));
        }
    }
}

/// Check root folder accessibility
async fn check_root_folders(state: &AppState, issues: &mut Vec<HealthResource>) {
    let repo = RootFolderRepository::new(state.db.clone());

    match repo.get_all().await {
        Ok(folders) => {
            if folders.is_empty() {
                issues.push(
                    HealthResource::new(
                        "RootFolderCheck",
                        HealthCheckType::Warning,
                        "No root folder is defined",
                    )
                    .with_wiki("https://wiki.servarr.com/sonarr/settings#root-folders"),
                );
                return;
            }

            for folder in folders {
                let path = std::path::Path::new(&folder.path);

                if !path.exists() {
                    issues.push(
                        HealthResource::new(
                            "RootFolderCheck",
                            HealthCheckType::Error,
                            &format!("Root folder does not exist: {}", folder.path),
                        )
                        .with_wiki("https://wiki.servarr.com/sonarr/settings#root-folders"),
                    );
                } else if !path.is_dir() {
                    issues.push(HealthResource::new(
                        "RootFolderCheck",
                        HealthCheckType::Error,
                        &format!("Root folder path is not a directory: {}", folder.path),
                    ));
                } else {
                    // Check if path is readable
                    match std::fs::read_dir(path) {
                        Ok(_) => {
                            // Path is accessible
                        }
                        Err(e) => {
                            issues.push(HealthResource::new(
                                "RootFolderCheck",
                                HealthCheckType::Error,
                                &format!(
                                    "Root folder is not accessible: {} ({})",
                                    folder.path, e
                                ),
                            ));
                        }
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to check root folders: {}", e);
            issues.push(HealthResource::new(
                "RootFolderCheck",
                HealthCheckType::Error,
                "Unable to check root folders",
            ));
        }
    }
}

/// Check for series that have missing root folders
async fn check_series_root_folders(state: &AppState, issues: &mut Vec<HealthResource>) {
    let series_repo = SeriesRepository::new(state.db.clone());
    let root_folder_repo = RootFolderRepository::new(state.db.clone());

    let series = match series_repo.get_all().await {
        Ok(s) => s,
        Err(_) => return,
    };

    let root_folders = match root_folder_repo.get_all().await {
        Ok(f) => f,
        Err(_) => return,
    };

    let root_folder_paths: std::collections::HashSet<_> =
        root_folders.iter().map(|f| f.path.as_str()).collect();

    let mut missing_root_series = Vec::new();
    let mut missing_path_series = Vec::new();

    for s in &series {
        // Check if series root folder exists in configured root folders
        if !root_folder_paths.contains(s.root_folder_path.as_str()) {
            // Check if the root folder path might just be a prefix match
            let has_matching_root = root_folder_paths
                .iter()
                .any(|rf| s.path.starts_with(*rf));

            if !has_matching_root {
                missing_root_series.push(s.title.clone());
            }
        }

        // Check if series path exists on disk
        let series_path = std::path::Path::new(&s.path);
        if !series_path.exists() {
            missing_path_series.push(s.title.clone());
        }
    }

    if !missing_root_series.is_empty() {
        let count = missing_root_series.len();
        let sample = if count <= 3 {
            missing_root_series.join(", ")
        } else {
            format!(
                "{}, and {} more",
                missing_root_series[..3].join(", "),
                count - 3
            )
        };

        issues.push(
            HealthResource::new(
                "RemovedSeriesCheck",
                HealthCheckType::Warning,
                &format!(
                    "{} series have root folders that are not in root folder list: {}",
                    count, sample
                ),
            )
            .with_wiki("https://wiki.servarr.com/sonarr/faq#series-removed-because-root-folder-is-missing"),
        );
    }

    if !missing_path_series.is_empty() {
        let count = missing_path_series.len();
        let sample = if count <= 3 {
            missing_path_series.join(", ")
        } else {
            format!(
                "{}, and {} more",
                missing_path_series[..3].join(", "),
                count - 3
            )
        };

        issues.push(HealthResource::new(
            "MissingSeriesPathCheck",
            HealthCheckType::Warning,
            &format!(
                "{} series have paths that do not exist on disk: {}",
                count, sample
            ),
        ));
    }
}

/// Check disk space on root folders
async fn check_disk_space(state: &AppState, issues: &mut Vec<HealthResource>) {
    let repo = RootFolderRepository::new(state.db.clone());

    let folders = match repo.get_all().await {
        Ok(f) => f,
        Err(_) => return,
    };

    // Minimum free space threshold (configurable in real Sonarr, default 100MB)
    let min_free_space: u64 = 100 * 1024 * 1024; // 100 MB

    for folder in folders {
        let path = std::path::Path::new(&folder.path);
        if !path.exists() {
            continue; // Already checked in root folder check
        }

        // Use system call to get disk space
        if let Some(free_space) = get_available_space(path) {
            if free_space < min_free_space {
                let free_mb = free_space / (1024 * 1024);
                issues.push(
                    HealthResource::new(
                        "DiskSpaceCheck",
                        HealthCheckType::Error,
                        &format!(
                            "Disk space is critically low for {}: {} MB free",
                            folder.path, free_mb
                        ),
                    )
                    .with_wiki("https://wiki.servarr.com/sonarr/system#disk-space"),
                );
            } else if free_space < min_free_space * 5 {
                // Warning at 500MB
                let free_mb = free_space / (1024 * 1024);
                issues.push(
                    HealthResource::new(
                        "DiskSpaceCheck",
                        HealthCheckType::Warning,
                        &format!(
                            "Disk space is running low for {}: {} MB free",
                            folder.path, free_mb
                        ),
                    )
                    .with_wiki("https://wiki.servarr.com/sonarr/system#disk-space"),
                );
            }
        }
    }
}

/// Get available disk space for a path (platform-specific)
fn get_available_space(path: &std::path::Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let path_cstr = CString::new(path.as_os_str().as_bytes()).ok()?;

        unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(path_cstr.as_ptr(), &mut stat) == 0 {
                // Available space for non-root users
                Some(stat.f_bavail as u64 * stat.f_frsize as u64)
            } else {
                None
            }
        }
    }

    #[cfg(windows)]
    {
        // Windows implementation would use GetDiskFreeSpaceExW
        None
    }

    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_health))
}
