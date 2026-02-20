//! Tracked download service
//! Manages the relationship between pir9 downloads and external download clients

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::{debug, error, info, warn};

use crate::core::datastore::models::TrackedDownloadDbModel;
use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeRepository, MovieRepository, SeriesRepository,
    TrackedDownloadRepository,
};
use crate::core::datastore::Database;
use crate::core::download::clients::{create_client_from_model, DownloadOptions, DownloadState};
use crate::core::indexers::ReleaseInfo;
use crate::core::parser::{parse_title, title_matches_series};
use crate::core::profiles::qualities::QualityModel;

use super::{
    Protocol, QueueItem, QueueResult, QueueStatus, StatusMessage, TrackedDownloadState,
    TrackedDownloadStatus,
};

/// Service for managing tracked downloads
pub struct TrackedDownloadService {
    db: Database,
}

/// Result of attempting to download a torrent file: either raw bytes or a
/// magnet URI discovered via redirect (e.g. Prowlarr → indexer → magnet).
enum TorrentDownload {
    File(Vec<u8>),
    Magnet(String),
}

/// Extract the info_hash from a bencoded .torrent file and build a magnet URI.
///
/// Finds the `info` dictionary in the bencoded data, SHA-1 hashes the raw
/// bencoded bytes of that dictionary, and constructs a magnet URI with the
/// info_hash and display name extracted from the `info.name` field.
fn torrent_bytes_to_magnet(data: &[u8], fallback_name: &str) -> Result<String> {
    use sha1::Digest;

    // Find the `info` key in the top-level dictionary.
    // Bencode format: d...4:infod...ee where `4:info` is the key.
    let info_key = b"4:info";
    let info_pos = data
        .windows(info_key.len())
        .position(|w| w == info_key)
        .context("Torrent file missing 'info' dictionary")?;

    let info_start = info_pos + info_key.len();

    // The info value starts at info_start and must be a dictionary ('d').
    if data.get(info_start) != Some(&b'd') {
        anyhow::bail!("Torrent 'info' value is not a dictionary");
    }

    // Walk the bencoded value to find its end.
    let info_end =
        bencode_value_end(data, info_start).context("Failed to parse bencoded info dictionary")?;
    let info_bytes = &data[info_start..info_end];

    // SHA-1 hash the raw bencoded info dictionary
    let hash = sha1::Sha1::digest(info_bytes);
    let hex_hash = hex::encode_upper(hash);

    // Try to extract the `name` field from the info dictionary for the display name.
    let name =
        extract_bencode_string(info_bytes, b"4:name").unwrap_or_else(|| fallback_name.to_string());
    let encoded_name = urlencoding::encode(&name);

    // Try to extract trackers from the top-level `announce` field
    let mut magnet = format!("magnet:?xt=urn:btih:{}&dn={}", hex_hash, encoded_name);
    if let Some(tracker) = extract_bencode_string(data, b"8:announce") {
        magnet.push_str(&format!("&tr={}", urlencoding::encode(&tracker)));
    }

    Ok(magnet)
}

/// Find the end offset of a bencoded value starting at `pos`.
fn bencode_value_end(data: &[u8], pos: usize) -> Option<usize> {
    if pos >= data.len() {
        return None;
    }
    match data[pos] {
        // Integer: i<digits>e
        b'i' => {
            let end = data[pos..].iter().position(|&b| b == b'e')?;
            Some(pos + end + 1)
        }
        // List: l<values>e
        b'l' => {
            let mut cursor = pos + 1;
            while cursor < data.len() && data[cursor] != b'e' {
                cursor = bencode_value_end(data, cursor)?;
            }
            Some(cursor + 1) // skip 'e'
        }
        // Dictionary: d<key><value>...e
        b'd' => {
            let mut cursor = pos + 1;
            while cursor < data.len() && data[cursor] != b'e' {
                // key (always a string)
                cursor = bencode_value_end(data, cursor)?;
                // value
                cursor = bencode_value_end(data, cursor)?;
            }
            Some(cursor + 1) // skip 'e'
        }
        // String: <length>:<data>
        b'0'..=b'9' => {
            let colon = data[pos..].iter().position(|&b| b == b':')?;
            let len_str = std::str::from_utf8(&data[pos..pos + colon]).ok()?;
            let len: usize = len_str.parse().ok()?;
            Some(pos + colon + 1 + len)
        }
        _ => None,
    }
}

/// Extract a UTF-8 string value for a given bencoded key from raw bytes.
fn extract_bencode_string(data: &[u8], key: &[u8]) -> Option<String> {
    let pos = data.windows(key.len()).position(|w| w == key)?;
    let val_start = pos + key.len();
    // The value should be a string: <len>:<data>
    if val_start >= data.len() || !data[val_start].is_ascii_digit() {
        return None;
    }
    let colon = data[val_start..].iter().position(|&b| b == b':')?;
    let len_str = std::str::from_utf8(&data[val_start..val_start + colon]).ok()?;
    let len: usize = len_str.parse().ok()?;
    let str_start = val_start + colon + 1;
    let str_end = str_start + len;
    if str_end > data.len() {
        return None;
    }
    String::from_utf8(data[str_start..str_end].to_vec()).ok()
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
        movie_id: Option<i64>,
    ) -> Result<i64> {
        let repo = TrackedDownloadRepository::new(self.db.clone());

        // Determine protocol
        let protocol = match release.protocol {
            crate::core::indexers::Protocol::Usenet => 1,
            crate::core::indexers::Protocol::Torrent => 2,
            crate::core::indexers::Protocol::Unknown => 0,
        };

        // Serialize quality and languages
        let quality_json =
            serde_json::to_string(&release.quality).unwrap_or_else(|_| "{}".to_string());
        let languages_json =
            serde_json::to_string(&release.languages).unwrap_or_else(|_| "[]".to_string());
        let episode_ids_json =
            serde_json::to_string(&episode_ids).unwrap_or_else(|_| "[]".to_string());

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
            movie_id,
        };

        let id = repo.insert(&tracked).await?;
        info!(
            "Tracked download created: id={}, title={}",
            id, tracked.title
        );

        Ok(id)
    }

    /// Get all queue items with merged download client status.
    /// Returns a `QueueResult` containing both the tracked items and the raw
    /// downloads polled from each client (keyed by client_id), so callers can
    /// reuse them without hitting the download clients a second time.
    pub async fn get_queue(&self) -> Result<QueueResult> {
        let repo = TrackedDownloadRepository::new(self.db.clone());
        let client_repo = DownloadClientRepository::new(self.db.clone());
        let _series_repo = SeriesRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());

        // Get all active tracked downloads
        let tracked = repo.get_all_active().await?;

        // Get all download clients for status lookup
        let clients = client_repo.get_all().await?;

        // Build client status map: (client_id, download_id) -> DownloadStatus
        // Also collect raw downloads per client for the QueueResult
        let mut client_status_map = std::collections::HashMap::new();
        let mut client_name_map = std::collections::HashMap::new();
        let mut polled_clients = std::collections::HashSet::new();
        let mut client_downloads: std::collections::HashMap<
            i64,
            Vec<crate::core::download::clients::DownloadStatus>,
        > = std::collections::HashMap::new();

        for client_model in &clients {
            if !client_model.enable {
                continue;
            }

            client_name_map.insert(client_model.id, client_model.name.clone());

            match create_client_from_model(client_model) {
                Ok(client) => match client.get_downloads().await {
                    Ok(downloads) => {
                        polled_clients.insert(client_model.id);
                        for dl in &downloads {
                            client_status_map
                                .insert((client_model.id, dl.id.clone()), dl.clone());
                        }
                        client_downloads.insert(client_model.id, downloads);
                    }
                    Err(e) => {
                        debug!("Failed to get downloads from {}: {}", client_model.name, e);
                    }
                },
                Err(e) => {
                    debug!("Failed to create client {}: {}", client_model.name, e);
                }
            }
        }

        if tracked.is_empty() {
            return Ok(QueueResult {
                items: vec![],
                client_downloads,
            });
        }

        // Convert tracked downloads to queue items
        let mut queue_items = Vec::new();

        for td in tracked {
            // Parse stored JSON
            let episode_ids: Vec<i64> = serde_json::from_str(&td.episode_ids).unwrap_or_default();
            let quality: QualityModel = serde_json::from_str(&td.quality).unwrap_or_default();
            let status_messages: Vec<StatusMessage> =
                serde_json::from_str(&td.status_messages).unwrap_or_default();

            // Get live status from download client
            let live_status =
                client_status_map.get(&(td.download_client_id, td.download_id.clone()));

            // If the client was successfully polled but the download is gone,
            // clean up the stale tracked_download record
            if live_status.is_none() && polled_clients.contains(&td.download_client_id) {
                info!(
                    "Cleaning up stale tracked download {}: '{}' no longer in download client",
                    td.id, td.title
                );
                let _ = repo.delete(td.id).await;
                continue;
            }

            // Determine queue status and state
            let (queue_status, tracked_state, size_left, timeleft, estimated_completion) =
                if let Some(live) = live_status {
                    let queue_status = match live.status {
                        DownloadState::Queued => QueueStatus::Queued,
                        DownloadState::Paused => QueueStatus::Paused,
                        DownloadState::Downloading => QueueStatus::Downloading,
                        DownloadState::Stalled => QueueStatus::Warning,
                        DownloadState::Seeding => QueueStatus::Completed,
                        DownloadState::Completed => QueueStatus::Completed,
                        DownloadState::Failed => QueueStatus::Failed,
                        DownloadState::Warning => QueueStatus::Warning,
                    };

                    let tracked_state = match live.status {
                        DownloadState::Queued => TrackedDownloadState::Downloading,
                        DownloadState::Downloading => TrackedDownloadState::Downloading,
                        DownloadState::Stalled => TrackedDownloadState::Downloading,
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

                    let estimated = live
                        .eta
                        .map(|secs| Utc::now() + chrono::Duration::seconds(secs));

                    (
                        queue_status,
                        tracked_state,
                        live.size_left,
                        timeleft,
                        estimated,
                    )
                } else {
                    // Download not found in client - might be removed or client unavailable
                    (
                        QueueStatus::DownloadClientUnavailable,
                        TrackedDownloadState::from_i32(td.status),
                        td.size,
                        None,
                        None,
                    )
                };

            // Get first episode info for display
            let (resolved_episode_id, season_number, episode_numbers) = if !episode_ids.is_empty() {
                if let Ok(Some(ep)) = episode_repo.get_by_id(episode_ids[0]).await {
                    let mut ep_nums: Vec<i32> = vec![ep.episode_number];
                    for &ep_id in episode_ids.iter().skip(1) {
                        if let Ok(Some(other_ep)) = episode_repo.get_by_id(ep_id).await {
                            ep_nums.push(other_ep.episode_number);
                        }
                    }
                    (episode_ids[0], ep.season_number, ep_nums)
                } else {
                    (0, 0, vec![])
                }
            } else {
                // Fallback: parse title to extract episode info when episode_ids is empty
                let mut fallback = (0i64, 0i32, vec![]);
                if td.series_id > 0 {
                    if let Some(info) = parse_title(&td.title) {
                        // Standard S01E02 matching
                        if let Some(season) = info.season_number {
                            if !info.episode_numbers.is_empty() {
                                let ep_num = info.episode_numbers[0];
                                if let Ok(Some(ep)) = episode_repo
                                    .get_by_series_season_episode(td.series_id, season, ep_num)
                                    .await
                                {
                                    fallback = (ep.id, ep.season_number, vec![ep.episode_number]);
                                }
                            }
                        }
                        // Anime absolute episode matching
                        if fallback.0 == 0 && !info.absolute_episode_numbers.is_empty() {
                            let abs_num = info.absolute_episode_numbers[0];
                            if let Ok(Some(ep)) = episode_repo
                                .get_by_series_and_absolute(td.series_id, abs_num)
                                .await
                            {
                                fallback = (ep.id, ep.season_number, vec![ep.episode_number]);
                            }
                        }
                    }
                }
                fallback
            };

            // Get episode has file status
            let episode_has_file = if resolved_episode_id > 0 {
                if let Ok(Some(ep)) = episode_repo.get_by_id(resolved_episode_id).await {
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

            let client_name = client_name_map
                .get(&td.download_client_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());

            // Extract peer data from live status
            let (seeds, leechers, seed_count, leech_count) = if let Some(live) = live_status {
                (live.seeds, live.leechers, live.seed_count, live.leech_count)
            } else {
                (None, None, None, None)
            };

            queue_items.push(QueueItem {
                id: td.id,
                series_id: td.series_id,
                episode_id: resolved_episode_id,
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
                movie_id: td.movie_id.unwrap_or(0),
                size: td.size,
                sizeleft: size_left,
                timeleft,
                estimated_completion_time: estimated_completion,
                added: td.added,
                quality,
                seeds,
                leechers,
                seed_count,
                leech_count,
            });
        }

        Ok(QueueResult {
            items: queue_items,
            client_downloads,
        })
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
                    warn!(
                        "Download client {} not found for tracked download {}",
                        td.download_client_id, td.id
                    );
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
                    // Download not found — removed from client, clean up DB record
                    info!(
                        "Download {} ('{}') no longer in client {}, removing tracked record",
                        td.download_id, td.title, client_model.name
                    );
                    let _ = repo.delete(td.id).await;
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
                            None,
                        )
                        .await?;

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
                        error!(
                            "Download failed: {} - {:?}",
                            td.title, live_status.error_message
                        );

                        repo.update_status(
                            td.id,
                            TrackedDownloadState::Failed as i32,
                            "[]",
                            live_status.error_message.as_deref(),
                        )
                        .await?;
                    }
                }
                _ => {
                    // Still downloading or queued - no action needed
                }
            }
        }

        // Auto-clean tracked downloads that have already been imported
        if let Err(e) = self.cleanup_imported_downloads().await {
            warn!("ProcessQueue: auto-clean failed: {}", e);
        }

        Ok(())
    }

    /// Auto-remove tracked downloads whose target media has been fully imported.
    ///
    /// For each completed/importPending tracked download, checks if the target
    /// (movie or episode) already has an imported file with a stored hash. If the
    /// source file at `output_path` still exists, computes its BLAKE3 hash and
    /// compares with the stored destination hash. On match the tracked download
    /// record is deleted from pir9's database — the torrent itself is **never**
    /// removed from the download client (qBittorrent etc.).
    pub async fn cleanup_imported_downloads(&self) -> Result<usize> {
        use crate::core::datastore::repositories::{EpisodeFileRepository, MovieFileRepository};

        let repo = TrackedDownloadRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let episode_file_repo = EpisodeFileRepository::new(self.db.clone());
        let movie_file_repo = MovieFileRepository::new(self.db.clone());

        let active = repo.get_all_active().await?;
        let mut cleaned = 0usize;

        for td in &active {
            // Only process completed/importPending downloads
            let state = TrackedDownloadState::from_i32(td.status);
            if !matches!(
                state,
                TrackedDownloadState::ImportPending
                    | TrackedDownloadState::Imported
                    | TrackedDownloadState::Downloading
            ) {
                continue;
            }

            // Need an output_path to locate the source file
            let output_path = match td.output_path.as_deref() {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };

            // --- Movie path ---
            if let Some(movie_id) = td.movie_id {
                let dest_file = match movie_file_repo.get_by_movie_id(movie_id).await {
                    Ok(Some(f)) => f,
                    _ => continue,
                };
                let dest_hash = match dest_file.file_hash.as_deref() {
                    Some(h) if !h.is_empty() => h.to_string(),
                    _ => continue,
                };

                // Find the largest video file at the source (same logic as scan_movie_folder)
                let source_path = find_largest_video_file(output_path).await;
                let source_file = match source_path {
                    Some(p) => p,
                    None => continue,
                };

                match crate::core::mediafiles::compute_file_hash(&source_file).await {
                    Ok(source_hash) if source_hash == dest_hash => {
                        info!(
                            "Auto-clean: movie download '{}' verified (hash match), removing from tracking",
                            td.title
                        );
                        let _ = repo.delete(td.id).await;
                        cleaned += 1;
                    }
                    Ok(_) => {
                        debug!(
                            "Auto-clean: hash mismatch for movie '{}', keeping tracked",
                            td.title
                        );
                    }
                    Err(e) => {
                        debug!(
                            "Auto-clean: could not hash source for '{}': {}",
                            td.title, e
                        );
                    }
                }
                continue;
            }

            // --- Series / Anime path ---
            if td.series_id > 0 {
                let episode_ids: Vec<i64> =
                    serde_json::from_str(&td.episode_ids).unwrap_or_default();
                if episode_ids.is_empty() {
                    continue;
                }

                // Check all matched episodes have files with hashes
                let mut all_verified = true;
                for &ep_id in &episode_ids {
                    let ep = match episode_repo.get_by_id(ep_id).await {
                        Ok(Some(e)) => e,
                        _ => {
                            all_verified = false;
                            break;
                        }
                    };
                    if !ep.has_file {
                        all_verified = false;
                        break;
                    }
                    // Verify via file hash if episode_file_id is set
                    if let Some(file_id) = ep.episode_file_id {
                        match episode_file_repo.get_by_id(file_id).await {
                            Ok(Some(ef)) if ef.file_hash.as_deref().is_some_and(|h| !h.is_empty()) => {
                                // Destination file has a verified hash — import succeeded
                            }
                            _ => {
                                all_verified = false;
                                break;
                            }
                        }
                    } else {
                        all_verified = false;
                        break;
                    }
                }

                if all_verified {
                    // For single-file downloads, also verify source hash against dest
                    let do_source_hash_check = episode_ids.len() == 1;
                    if do_source_hash_check {
                        let ep = episode_repo.get_by_id(episode_ids[0]).await?.unwrap();
                        let ef = episode_file_repo
                            .get_by_id(ep.episode_file_id.unwrap())
                            .await?
                            .unwrap();
                        let dest_hash = ef.file_hash.as_deref().unwrap();

                        if let Some(source_file) = find_largest_video_file(output_path).await {
                            match crate::core::mediafiles::compute_file_hash(&source_file).await {
                                Ok(source_hash) if source_hash == dest_hash => {
                                    info!(
                                        "Auto-clean: episode download '{}' verified (hash match), removing from tracking",
                                        td.title
                                    );
                                    let _ = repo.delete(td.id).await;
                                    cleaned += 1;
                                }
                                Ok(_) => {
                                    debug!(
                                        "Auto-clean: hash mismatch for '{}', keeping tracked",
                                        td.title
                                    );
                                }
                                Err(e) => {
                                    debug!(
                                        "Auto-clean: could not hash source for '{}': {}",
                                        td.title, e
                                    );
                                }
                            }
                        }
                    } else {
                        // Season pack: all episodes have verified files — clean without
                        // individual source hash check (too many files to match 1:1)
                        info!(
                            "Auto-clean: season pack '{}' — all {} episodes imported, removing from tracking",
                            td.title,
                            episode_ids.len()
                        );
                        let _ = repo.delete(td.id).await;
                        cleaned += 1;
                    }
                }
            }
        }

        if cleaned > 0 {
            info!("Auto-clean: removed {} imported downloads from tracking", cleaned);
        }
        Ok(cleaned)
    }

    /// Remove a download from queue
    pub async fn remove(&self, id: i64, remove_from_client: bool, blocklist: bool) -> Result<()> {
        let repo = TrackedDownloadRepository::new(self.db.clone());
        let client_repo = DownloadClientRepository::new(self.db.clone());

        // Get the tracked download
        let tracked = repo
            .get_by_id(id)
            .await?
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

    /// Grab a release and send to download client.
    /// Pass `movie_id` for movie releases to select the correct download category.
    pub async fn grab_release(
        &self,
        release: &ReleaseInfo,
        episode_ids: Vec<i64>,
        movie_id: Option<i64>,
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
        let client_model = clients
            .iter()
            .filter(|c| c.enable && c.protocol == protocol_num)
            .min_by_key(|c| c.priority)
            .context("No enabled download client for this protocol")?;

        // Create the download client
        let client = create_client_from_model(client_model)?;

        // Read the category from client settings based on content type.
        // Movies use "movieCategory", series use "category". Falls back to "sonarr".
        let settings = serde_json::from_str::<serde_json::Value>(&client_model.settings)
            .unwrap_or(serde_json::json!({}));

        let category_key = if movie_id.is_some() {
            "movieCategory"
        } else {
            "category"
        };

        let grab_category = settings
            .get(category_key)
            .or_else(|| settings.get("category"))
            .and_then(|v| v.as_str())
            .map(|cat| cat.split(',').next().unwrap_or("sonarr").trim().to_string())
            .unwrap_or_else(|| "sonarr".to_string());

        let options = DownloadOptions {
            category: Some(grab_category),
            priority: None,
            download_dir: None,
            tags: vec![],
        };

        // Send to download client
        // Prefer magnet links — they go directly to qBittorrent without needing
        // the download client to reach the indexer/Prowlarr. Construct a magnet
        // from info_hash if no explicit magnet_url is provided. Also check
        // download_url itself — some indexers put the magnet URI there.
        let magnet = release
            .magnet_url
            .as_deref()
            .filter(|u| u.starts_with("magnet:"))
            .map(String::from)
            .or_else(|| {
                release
                    .download_url
                    .as_deref()
                    .filter(|u| u.starts_with("magnet:"))
                    .map(String::from)
            })
            .or_else(|| {
                release.info_hash.as_ref().map(|hash| {
                    let encoded_title = urlencoding::encode(&release.title).into_owned();
                    format!("magnet:?xt=urn:btih:{}&dn={}", hash, encoded_title)
                })
            });

        let download_id = if let Some(ref magnet) = magnet {
            info!("Adding magnet to {}: {}", client_model.name, release.title);
            client.add_from_magnet(magnet, options).await?
        } else if let Some(ref url) = release.download_url {
            // No magnet available — download the torrent file through pir9 and
            // send bytes to the client so qBittorrent doesn't need to reach the indexer.
            if release.protocol == crate::core::indexers::Protocol::Torrent {
                info!(
                    "Downloading torrent file for {}: {}",
                    client_model.name, release.title
                );
                match Self::download_torrent_file(url).await? {
                    TorrentDownload::Magnet(magnet) => {
                        info!(
                            "Adding magnet (from redirect) to {}: {}",
                            client_model.name, release.title
                        );
                        client.add_from_magnet(&magnet, options).await?
                    }
                    TorrentDownload::File(bytes) => {
                        // Convert .torrent → magnet so we always use magnets
                        let magnet = torrent_bytes_to_magnet(&bytes, &release.title)?;
                        info!(
                            "Adding magnet (from .torrent, {} bytes) to {}: {}",
                            bytes.len(),
                            client_model.name,
                            release.title
                        );
                        client.add_from_magnet(&magnet, options).await?
                    }
                }
            } else {
                info!("Adding URL to {}: {}", client_model.name, release.title);
                client.add_from_url(url, options).await?
            }
        } else {
            anyhow::bail!("Release has no download URL or magnet link");
        };

        // Track the download
        let tracked_id = self
            .track_download(
                download_id,
                client_model.id,
                release,
                episode_ids,
                false, // TODO: Determine if upgrade
                movie_id,
            )
            .await?;

        Ok(tracked_id)
    }

    /// Reconcile untracked downloads from all clients.
    ///
    /// Scans every enabled download client, parses torrent/NZB names, matches
    /// them to series and episodes in the database, and creates
    /// `TrackedDownloadDbModel` entries so the downloads appear in the queue
    /// with proper metadata and are excluded from Wanted/Missing.
    ///
    /// Returns the number of newly tracked downloads.
    pub async fn reconcile_downloads(&self) -> Result<usize> {
        let repo = TrackedDownloadRepository::new(self.db.clone());
        let client_repo = DownloadClientRepository::new(self.db.clone());
        let series_repo = SeriesRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());

        let movie_repo = MovieRepository::new(self.db.clone());

        let clients = client_repo.get_all().await?;
        let all_series = series_repo.get_all().await?;
        let all_movies = movie_repo.get_all().await?;

        let mut reconciled = 0usize;

        // Purge tracked downloads with fake qbt-* UUIDs. These were created
        // before the info_hash extraction fix and can never match a real torrent
        // in qBittorrent. Deleting them lets the reconciliation below re-create
        // them with the correct info_hash as download_id.
        let active = repo.get_all_active().await?;
        for td in active.iter().filter(|t| t.download_id.starts_with("qbt-")) {
            info!(
                "Purging stale tracked download with fake ID: {} ({})",
                td.download_id, td.title
            );
            repo.delete(td.id).await?;
        }

        for client_model in clients.iter().filter(|c| c.enable) {
            let client = match create_client_from_model(client_model) {
                Ok(c) => c,
                Err(e) => {
                    debug!(
                        "Reconcile: failed to create client {}: {}",
                        client_model.name, e
                    );
                    continue;
                }
            };

            let downloads = match client.get_downloads().await {
                Ok(d) => d,
                Err(e) => {
                    debug!(
                        "Reconcile: failed to get downloads from {}: {}",
                        client_model.name, e
                    );
                    continue;
                }
            };

            for dl in downloads {
                // Skip if already tracked
                if repo
                    .get_by_download_id(client_model.id, &dl.id)
                    .await?
                    .is_some()
                {
                    continue;
                }

                // Parse the download name
                let parsed = match parse_title(&dl.name) {
                    Some(info) => info,
                    None => {
                        debug!("Reconcile: could not parse title: {}", dl.name);
                        continue;
                    }
                };

                // Match against series
                let matched_series = all_series.iter().find(|s| {
                    title_matches_series(&parsed, &s.title)
                        || title_matches_series(&parsed, &s.clean_title)
                });

                // Serialize quality/languages (shared by both series and movie paths)
                let quality_json =
                    serde_json::to_string(&parsed.quality).unwrap_or_else(|_| "{}".to_string());
                let languages_json =
                    serde_json::to_string(&parsed.languages).unwrap_or_else(|_| "[]".to_string());

                if let Some(series) = matched_series {
                    // --- Series match path ---
                    let mut episode_ids = Vec::new();
                    if let Some(season) = parsed.season_number {
                        for &ep_num in &parsed.episode_numbers {
                            if let Ok(Some(ep)) = episode_repo
                                .get_by_series_season_episode(series.id, season, ep_num)
                                .await
                            {
                                episode_ids.push(ep.id);
                            }
                        }
                    }

                    if episode_ids.is_empty() {
                        debug!(
                            "Reconcile: matched series '{}' but no episodes for '{}'",
                            series.title, dl.name
                        );
                        continue;
                    }

                    let episode_ids_json =
                        serde_json::to_string(&episode_ids).unwrap_or_else(|_| "[]".to_string());

                    let tracked = TrackedDownloadDbModel {
                        id: 0,
                        download_id: dl.id.clone(),
                        download_client_id: client_model.id,
                        series_id: series.id,
                        episode_ids: episode_ids_json,
                        title: dl.name.clone(),
                        indexer: None,
                        size: dl.size,
                        protocol: client_model.protocol,
                        quality: quality_json,
                        languages: languages_json,
                        status: TrackedDownloadState::Downloading as i32,
                        status_messages: "[]".to_string(),
                        error_message: None,
                        output_path: dl.output_path.clone(),
                        is_upgrade: false,
                        added: Utc::now(),
                        movie_id: None,
                    };

                    match repo.insert(&tracked).await {
                        Ok(id) => {
                            info!(
                                "Reconciled download: id={}, '{}' → {} S{:02}E{} (episodes: {:?})",
                                id,
                                dl.name,
                                series.title,
                                parsed.season_number.unwrap_or(0),
                                parsed
                                    .episode_numbers
                                    .iter()
                                    .map(|n| format!("{:02}", n))
                                    .collect::<Vec<_>>()
                                    .join("E"),
                                episode_ids,
                            );
                            reconciled += 1;
                        }
                        Err(e) => {
                            warn!(
                                "Reconcile: failed to insert tracked download for '{}': {}",
                                dl.name, e
                            );
                        }
                    }
                } else {
                    // --- Movie match fallback ---
                    use crate::core::parser::normalize_title;
                    let name_normalized = normalize_title(&dl.name);

                    let matched_movie = all_movies.iter().find(|m| {
                        let clean = normalize_title(&m.clean_title);
                        clean.len() >= 4 && name_normalized.contains(clean.as_str())
                    });

                    match matched_movie {
                        Some(movie) => {
                            let tracked = TrackedDownloadDbModel {
                                id: 0,
                                download_id: dl.id.clone(),
                                download_client_id: client_model.id,
                                series_id: 0,
                                episode_ids: "[]".to_string(),
                                title: dl.name.clone(),
                                indexer: None,
                                size: dl.size,
                                protocol: client_model.protocol,
                                quality: quality_json,
                                languages: languages_json,
                                status: TrackedDownloadState::Downloading as i32,
                                status_messages: "[]".to_string(),
                                error_message: None,
                                output_path: dl.output_path.clone(),
                                is_upgrade: false,
                                added: Utc::now(),
                                movie_id: Some(movie.id),
                            };

                            match repo.insert(&tracked).await {
                                Ok(id) => {
                                    info!(
                                        "Reconciled movie download: id={}, '{}' → {}",
                                        id, dl.name, movie.title,
                                    );
                                    reconciled += 1;
                                }
                                Err(e) => {
                                    warn!(
                                        "Reconcile: failed to insert tracked download for '{}': {}",
                                        dl.name, e
                                    );
                                }
                            }
                        }
                        None => {
                            debug!(
                                "Reconcile: no series or movie match for '{}'",
                                dl.name
                            );
                        }
                    }
                }
            }
        }

        info!("Reconcile downloads complete: {} newly tracked", reconciled);

        // Auto-clean tracked downloads that have already been imported
        if let Err(e) = self.cleanup_imported_downloads().await {
            warn!("Reconcile: auto-clean failed: {}", e);
        }

        Ok(reconciled)
    }

    /// Download a torrent file from a URL, manually following redirects with logging.
    /// If a redirect leads to a `magnet:` URI, returns `TorrentDownload::Magnet`
    /// instead of trying to HTTP GET the magnet.
    async fn download_torrent_file(url: &str) -> Result<TorrentDownload> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .danger_accept_invalid_certs(true)
            .build()?;

        let mut current_url = url.to_string();
        for i in 0..10 {
            debug!(
                "Torrent download hop {}: {}",
                i,
                &current_url[..current_url.len().min(120)]
            );
            let resp = http.get(&current_url).send().await.context(format!(
                "Failed to download torrent from: {}",
                &current_url[..current_url.len().min(100)]
            ))?;

            let status = resp.status();
            if status.is_redirection() {
                if let Some(location) = resp.headers().get("location") {
                    let loc = location.to_str().unwrap_or("(invalid)");
                    debug!(
                        "Redirect {} → Location: {}",
                        status,
                        &loc[..loc.len().min(200)]
                    );

                    // Magnet redirect — return it directly, don't try to HTTP GET it
                    if loc.starts_with("magnet:") {
                        info!("Redirect resolved to magnet URI");
                        return Ok(TorrentDownload::Magnet(loc.to_string()));
                    }

                    current_url = if loc.starts_with("http") {
                        loc.to_string()
                    } else if loc.starts_with('/') {
                        if let Ok(base) = reqwest::Url::parse(&current_url) {
                            format!(
                                "{}://{}{}",
                                base.scheme(),
                                base.host_str().unwrap_or(""),
                                loc
                            )
                        } else {
                            loc.to_string()
                        }
                    } else {
                        loc.to_string()
                    };
                    continue;
                }
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!(
                    "Redirect {} with no Location header from: \"{}\". Body: \"{}\"",
                    status,
                    &current_url[..current_url.len().min(100)],
                    &body[..body.len().min(200)]
                );
            }

            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!(
                    "Failed to download torrent (HTTP {}). Source: \"{}\". Reason: \"{}\"",
                    status,
                    &current_url[..current_url.len().min(100)],
                    &body[..body.len().min(200)]
                );
            }

            let bytes = resp.bytes().await?.to_vec();
            if bytes.is_empty() {
                anyhow::bail!(
                    "Downloaded empty torrent file from: {}",
                    &current_url[..current_url.len().min(100)]
                );
            }
            return Ok(TorrentDownload::File(bytes));
        }
        anyhow::bail!(
            "Too many redirects downloading torrent from: {}",
            &url[..url.len().min(100)]
        )
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

/// Find the largest video file at a path (file or directory, max depth 2).
/// Returns `None` if the path doesn't exist or contains no video files.
/// Uses `spawn_blocking` since this is filesystem I/O (may be NFS-mounted).
async fn find_largest_video_file(path: &str) -> Option<std::path::PathBuf> {
    use crate::core::scanner::is_video_file;
    let path = std::path::PathBuf::from(path);

    tokio::task::spawn_blocking(move || {
        if !path.exists() {
            return None;
        }

        // Single file: return it if it's a video
        if path.is_file() {
            return if is_video_file(&path) {
                Some(path)
            } else {
                None
            };
        }

        // Directory: walk up to depth 2, return largest video file
        let mut best: Option<(std::path::PathBuf, u64)> = None;

        fn walk(
            dir: &std::path::Path,
            best: &mut Option<(std::path::PathBuf, u64)>,
            depth: usize,
        ) {
            use crate::core::scanner::is_video_file;
            if depth > 2 {
                return;
            }
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => return,
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, best, depth + 1);
                } else if is_video_file(&p) {
                    let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    if best.as_ref().is_none_or(|(_, s)| size > *s) {
                        *best = Some((p, size));
                    }
                }
            }
        }

        walk(&path, &mut best, 0);
        best.map(|(p, _)| p)
    })
    .await
    .unwrap_or(None)
}
