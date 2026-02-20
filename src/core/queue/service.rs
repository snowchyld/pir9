//! Tracked download service
//! Manages the relationship between pir9 downloads and external download clients

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::{debug, info, warn, error};

use crate::core::datastore::Database;
use crate::core::datastore::models::TrackedDownloadDbModel;
use crate::core::datastore::repositories::{
    TrackedDownloadRepository, DownloadClientRepository, SeriesRepository, EpisodeRepository,
};
use crate::core::download::clients::{create_client_from_model, DownloadOptions, DownloadState};
use crate::core::indexers::ReleaseInfo;
use crate::core::profiles::qualities::QualityModel;

use super::{TrackedDownloadState, TrackedDownloadStatus, Protocol, QueueItem, QueueStatus, StatusMessage};

/// Service for managing tracked downloads
pub struct TrackedDownloadService {
    db: Database,
}

impl TrackedDownloadService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Track a new download after sending to client
    /// Returns the tracked download ID
    pub async fn track_download(
        &self,
        download_id: String,
        download_client_id: i64,
        release: &ReleaseInfo,
        episode_ids: Vec<i64>,
        is_upgrade: bool,
    ) -> Result<i64> {
        let repo = TrackedDownloadRepository::new(self.db.clone());

        // Determine protocol
        let protocol = match release.protocol {
            crate::core::indexers::Protocol::Usenet => 1,
            crate::core::indexers::Protocol::Torrent => 2,
            crate::core::indexers::Protocol::Unknown => 0,
        };

        // Serialize quality and languages
        let quality_json = serde_json::to_string(&release.quality)
            .unwrap_or_else(|_| "{}".to_string());
        let languages_json = serde_json::to_string(&release.languages)
            .unwrap_or_else(|_| "[]".to_string());
        let episode_ids_json = serde_json::to_string(&episode_ids)
            .unwrap_or_else(|_| "[]".to_string());

        let tracked = TrackedDownloadDbModel {
            id: 0, // Will be set by database
            download_id,
            download_client_id,
            series_id: release.series_id.unwrap_or(0),
            episode_ids: episode_ids_json,
            title: release.title.clone(),
            indexer: Some(release.indexer.clone()),
            size: release.size,
            protocol,
            quality: quality_json,
            languages: languages_json,
            status: TrackedDownloadState::Downloading as i32,
            status_messages: "[]".to_string(),
            error_message: None,
            output_path: None,
            is_upgrade,
            added: Utc::now(),
        };

        let id = repo.insert(&tracked).await?;
        info!("Tracked download created: id={}, title={}", id, tracked.title);

        Ok(id)
    }

    /// Get all queue items with merged download client status
    pub async fn get_queue(&self) -> Result<Vec<QueueItem>> {
        let repo = TrackedDownloadRepository::new(self.db.clone());
        let client_repo = DownloadClientRepository::new(self.db.clone());
        let _series_repo = SeriesRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());

        // Get all active tracked downloads
        let tracked = repo.get_all_active().await?;
        if tracked.is_empty() {
            return Ok(vec![]);
        }

        // Get all download clients for status lookup
        let clients = client_repo.get_all().await?;

        // Build client status map: (client_id, download_id) -> DownloadStatus
        let mut client_status_map = std::collections::HashMap::new();
        let mut client_name_map = std::collections::HashMap::new();

        for client_model in &clients {
            if !client_model.enable {
                continue;
            }

            client_name_map.insert(client_model.id, client_model.name.clone());

            match create_client_from_model(client_model) {
                Ok(client) => {
                    match client.get_downloads().await {
                        Ok(downloads) => {
                            for dl in downloads {
                                client_status_map.insert(
                                    (client_model.id, dl.id.clone()),
                                    dl
                                );
                            }
                        }
                        Err(e) => {
                            debug!("Failed to get downloads from {}: {}", client_model.name, e);
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to create client {}: {}", client_model.name, e);
                }
            }
        }

        // Convert tracked downloads to queue items
        let mut queue_items = Vec::new();

        for td in tracked {
            // Parse stored JSON
            let episode_ids: Vec<i64> = serde_json::from_str(&td.episode_ids).unwrap_or_default();
            let quality: QualityModel = serde_json::from_str(&td.quality).unwrap_or_default();
            let status_messages: Vec<StatusMessage> = serde_json::from_str(&td.status_messages).unwrap_or_default();

            // Get live status from download client
            let live_status = client_status_map.get(&(td.download_client_id, td.download_id.clone()));

            // Determine queue status and state
            let (queue_status, tracked_state, size_left, timeleft, estimated_completion) =
                if let Some(live) = live_status {
                    let queue_status = match live.status {
                        DownloadState::Queued => QueueStatus::Queued,
                        DownloadState::Paused => QueueStatus::Paused,
                        DownloadState::Downloading => QueueStatus::Downloading,
                        DownloadState::Seeding => QueueStatus::Completed,
                        DownloadState::Completed => QueueStatus::Completed,
                        DownloadState::Failed => QueueStatus::Failed,
                        DownloadState::Warning => QueueStatus::Warning,
                    };

                    let tracked_state = match live.status {
                        DownloadState::Queued => TrackedDownloadState::Downloading,
                        DownloadState::Downloading => TrackedDownloadState::Downloading,
                        DownloadState::Paused => TrackedDownloadState::Downloading,
                        DownloadState::Seeding => TrackedDownloadState::ImportPending,
                        DownloadState::Completed => TrackedDownloadState::ImportPending,
                        DownloadState::Failed => TrackedDownloadState::Failed,
                        DownloadState::Warning => TrackedDownloadState::ImportBlocked,
                    };

                    let timeleft = live.eta.map(|secs| {
                        let hours = secs / 3600;
                        let minutes = (secs % 3600) / 60;
                        let seconds = secs % 60;
                        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
                    });

                    let estimated = live.eta.map(|secs| {
                        Utc::now() + chrono::Duration::seconds(secs)
                    });

                    (queue_status, tracked_state, live.size_left, timeleft, estimated)
                } else {
                    // Download not found in client - might be removed or client unavailable
                    (QueueStatus::DownloadClientUnavailable,
                     TrackedDownloadState::from_i32(td.status),
                     td.size,
                     None,
                     None)
                };

            // Get first episode info for display
            let (season_number, episode_numbers) = if !episode_ids.is_empty() {
                if let Ok(Some(ep)) = episode_repo.get_by_id(episode_ids[0]).await {
                    let mut ep_nums: Vec<i32> = vec![ep.episode_number];
                    for &ep_id in episode_ids.iter().skip(1) {
                        if let Ok(Some(other_ep)) = episode_repo.get_by_id(ep_id).await {
                            ep_nums.push(other_ep.episode_number);
                        }
                    }
                    (ep.season_number, ep_nums)
                } else {
                    (0, vec![])
                }
            } else {
                (0, vec![])
            };

            // Get episode has file status
            let episode_has_file = if !episode_ids.is_empty() {
                if let Ok(Some(ep)) = episode_repo.get_by_id(episode_ids[0]).await {
                    ep.has_file
                } else {
                    false
                }
            } else {
                false
            };

            let protocol = match td.protocol {
                1 => Protocol::Usenet,
                2 => Protocol::Torrent,
                _ => Protocol::Unknown,
            };

            let client_name = client_name_map.get(&td.download_client_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());

            queue_items.push(QueueItem {
                id: td.id,
                series_id: td.series_id,
                episode_id: episode_ids.first().copied().unwrap_or(0),
                season_number,
                episode_numbers,
                title: td.title,
                status: queue_status,
                tracked_download_status: TrackedDownloadStatus::Ok,
                tracked_download_state: tracked_state,
                status_messages,
                error_message: td.error_message,
                download_id: Some(td.download_id),
                protocol,
                download_client: client_name,
                indexer: td.indexer.unwrap_or_default(),
                output_path: td.output_path,
                episode_has_file,
                size: td.size,
                sizeleft: size_left,
                timeleft,
                estimated_completion_time: estimated_completion,
                added: td.added,
                quality,
            });
        }

        Ok(queue_items)
    }

    /// Process the download queue - update statuses and trigger imports
    pub async fn process_queue(&self) -> Result<()> {
        let repo = TrackedDownloadRepository::new(self.db.clone());
        let client_repo = DownloadClientRepository::new(self.db.clone());

        // Get all active tracked downloads
        let tracked = repo.get_all_active().await?;
        if tracked.is_empty() {
            return Ok(());
        }

        debug!("Processing {} tracked downloads", tracked.len());

        // Get download clients
        let clients = client_repo.get_all().await?;

        for td in tracked {
            // Find the appropriate client
            let client_model = match clients.iter().find(|c| c.id == td.download_client_id) {
                Some(c) => c,
                None => {
                    warn!("Download client {} not found for tracked download {}",
                          td.download_client_id, td.id);
                    continue;
                }
            };

            if !client_model.enable {
                continue;
            }

            // Get client and check download status
            let client = match create_client_from_model(client_model) {
                Ok(c) => c,
                Err(e) => {
                    debug!("Failed to create client {}: {}", client_model.name, e);
                    continue;
                }
            };

            let live_status = match client.get_download(&td.download_id).await {
                Ok(Some(status)) => status,
                Ok(None) => {
                    // Download not found - might have been removed externally
                    warn!("Download {} not found in client {}", td.download_id, client_model.name);
                    // Mark as failed
                    repo.update_status(
                        td.id,
                        TrackedDownloadState::Failed as i32,
                        "[]",
                        Some("Download not found in client")
                    ).await?;
                    continue;
                }
                Err(e) => {
                    debug!("Failed to get download status: {}", e);
                    continue;
                }
            };

            // Update output path if available
            if let Some(ref output_path) = live_status.output_path {
                if td.output_path.as_deref() != Some(output_path) {
                    repo.update_output_path(td.id, output_path).await?;
                }
            }

            // Check for state transitions
            let current_state = TrackedDownloadState::from_i32(td.status);

            match live_status.status {
                DownloadState::Completed | DownloadState::Seeding => {
                    if current_state == TrackedDownloadState::Downloading {
                        info!("Download completed: {} ({})", td.title, td.download_id);

                        // Update to ImportPending
                        repo.update_status(
                            td.id,
                            TrackedDownloadState::ImportPending as i32,
                            "[]",
                            None
                        ).await?;

                        // TODO: Trigger import process
                        // This would:
                        // 1. Parse files in output_path
                        // 2. Match to episodes
                        // 3. Rename and move to series folder
                        // 4. Update episode records
                        // 5. Mark as Imported or Failed
                    }
                }
                DownloadState::Failed => {
                    if current_state != TrackedDownloadState::Failed {
                        error!("Download failed: {} - {:?}", td.title, live_status.error_message);

                        repo.update_status(
                            td.id,
                            TrackedDownloadState::Failed as i32,
                            "[]",
                            live_status.error_message.as_deref()
                        ).await?;
                    }
                }
                _ => {
                    // Still downloading or queued - no action needed
                }
            }
        }

        Ok(())
    }

    /// Remove a download from queue
    pub async fn remove(
        &self,
        id: i64,
        remove_from_client: bool,
        blocklist: bool,
    ) -> Result<()> {
        let repo = TrackedDownloadRepository::new(self.db.clone());
        let client_repo = DownloadClientRepository::new(self.db.clone());

        // Get the tracked download
        let tracked = repo.get_by_id(id).await?
            .context("Tracked download not found")?;

        // Remove from download client if requested
        if remove_from_client {
            let client_model = client_repo.get_by_id(tracked.download_client_id).await?;
            if let Some(model) = client_model {
                if let Ok(client) = create_client_from_model(&model) {
                    if let Err(e) = client.remove(&tracked.download_id, true).await {
                        warn!("Failed to remove download from client: {}", e);
                    }
                }
            }
        }

        // Add to blocklist if requested
        if blocklist {
            // TODO: Add to blocklist table
            info!("Would add to blocklist: {}", tracked.title);
        }

        // Delete from tracked downloads
        repo.delete(id).await?;
        info!("Removed tracked download: {} ({})", tracked.title, id);

        Ok(())
    }

    /// Grab a release and send to download client
    pub async fn grab_release(
        &self,
        release: &ReleaseInfo,
        episode_ids: Vec<i64>,
    ) -> Result<i64> {
        let client_repo = DownloadClientRepository::new(self.db.clone());

        // Get enabled download clients for this protocol
        let clients = client_repo.get_all().await?;

        let protocol_num = match release.protocol {
            crate::core::indexers::Protocol::Usenet => 1,
            crate::core::indexers::Protocol::Torrent => 2,
            crate::core::indexers::Protocol::Unknown => 0,
        };

        // Find the best client for this protocol (highest priority = lowest number)
        let client_model = clients.iter()
            .filter(|c| c.enable && c.protocol == protocol_num)
            .min_by_key(|c| c.priority)
            .context("No enabled download client for this protocol")?;

        // Create the download client
        let client = create_client_from_model(client_model)?;

        // Prepare download options
        let options = DownloadOptions {
            category: Some("tv-sonarr".to_string()),
            priority: None,
            download_dir: None,
            tags: vec![],
        };

        // Send to download client
        let download_id = if let Some(ref magnet) = release.magnet_url {
            info!("Adding magnet to {}: {}", client_model.name, release.title);
            client.add_from_magnet(magnet, options).await?
        } else if let Some(ref url) = release.download_url {
            info!("Adding URL to {}: {}", client_model.name, release.title);
            client.add_from_url(url, options).await?
        } else {
            anyhow::bail!("Release has no download URL or magnet link");
        };

        // Track the download
        let tracked_id = self.track_download(
            download_id,
            client_model.id,
            release,
            episode_ids,
            false, // TODO: Determine if upgrade
        ).await?;

        Ok(tracked_id)
    }
}

impl TrackedDownloadState {
    /// Convert from i32 database value
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => TrackedDownloadState::Downloading,
            1 => TrackedDownloadState::ImportBlocked,
            2 => TrackedDownloadState::ImportPending,
            3 => TrackedDownloadState::Importing,
            4 => TrackedDownloadState::Imported,
            5 => TrackedDownloadState::FailedPending,
            6 => TrackedDownloadState::Failed,
            7 => TrackedDownloadState::Ignored,
            _ => TrackedDownloadState::Downloading,
        }
    }
}
