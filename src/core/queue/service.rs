//! Tracked download service
//! Manages the relationship between pir9 downloads and external download clients

use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::{debug, error, info, warn};

use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeRepository, MovieRepository, SeriesRepository,
};
use crate::core::datastore::Database;
use crate::core::download::clients::{create_client_from_model, DownloadOptions, DownloadState};
use crate::core::indexers::ReleaseInfo;
use crate::core::profiles::qualities::QualityModel;

use super::stores::TrackedDownloads;
use super::tracked::*;
use super::{
    Protocol, QueueItem, QueueResult, QueueStatus, StatusMessage, TrackedDownloadState,
    TrackedDownloadStatus,
};

/// Service for managing tracked downloads
pub struct TrackedDownloadService {
    db: Database,
    tracked: Arc<TrackedDownloads>,
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

    let info_key = b"4:info";
    let info_pos = data
        .windows(info_key.len())
        .position(|w| w == info_key)
        .context("Torrent file missing 'info' dictionary")?;

    let info_start = info_pos + info_key.len();

    if data.get(info_start) != Some(&b'd') {
        anyhow::bail!("Torrent 'info' value is not a dictionary");
    }

    let info_end =
        bencode_value_end(data, info_start).context("Failed to parse bencoded info dictionary")?;
    let info_bytes = &data[info_start..info_end];

    let hash = sha1::Sha1::digest(info_bytes);
    let hex_hash = hex::encode_upper(hash);

    let name =
        extract_bencode_string(info_bytes, b"4:name").unwrap_or_else(|| fallback_name.to_string());
    let encoded_name = urlencoding::encode(&name);

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
        b'i' => {
            let end = data[pos..].iter().position(|&b| b == b'e')?;
            Some(pos + end + 1)
        }
        b'l' => {
            let mut cursor = pos + 1;
            while cursor < data.len() && data[cursor] != b'e' {
                cursor = bencode_value_end(data, cursor)?;
            }
            Some(cursor + 1)
        }
        b'd' => {
            let mut cursor = pos + 1;
            while cursor < data.len() && data[cursor] != b'e' {
                cursor = bencode_value_end(data, cursor)?;
                cursor = bencode_value_end(data, cursor)?;
            }
            Some(cursor + 1)
        }
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
    pub fn new(db: Database, tracked: Arc<TrackedDownloads>) -> Self {
        Self { db, tracked }
    }

    /// Track a new download after sending to client.
    /// Determines the content type and inserts into the appropriate store.
    /// Returns the tracked download ID.
    pub async fn track_download(
        &self,
        download_id: String,
        download_client_id: i64,
        release: &ReleaseInfo,
        episode_ids: Vec<i64>,
        is_upgrade: bool,
        movie_id: Option<i64>,
        content_type: &str,
    ) -> Result<i64> {
        let quality_json =
            serde_json::to_string(&release.quality).unwrap_or_else(|_| "{}".to_string());
        let indexer = Some(release.indexer.clone());
        let now = Utc::now();

        macro_rules! make_td {
            ($content:expr) => {
                TrackedDownload {
                    id: 0,
                    download_id: download_id.clone(),
                    client_id: download_client_id,
                    content: $content,
                    title: release.title.clone(),
                    quality: quality_json.clone(),
                    indexer: indexer.clone(),
                    added: now,
                    is_upgrade,
                }
            };
        }

        let id = match content_type {
            "movie" => {
                let movie_id = movie_id.context("movie_id required for movie content type")?;
                self.tracked.movies.insert(make_td!(MovieRef { movie_id })).await?
            }
            "music" => {
                let artist_id = release.series_id.unwrap_or(0);
                self.tracked.music.insert(make_td!(MusicRef { artist_id })).await?
            }
            "audiobook" => {
                let audiobook_id = release.series_id.unwrap_or(0);
                self.tracked.audiobooks.insert(make_td!(AudiobookRef { audiobook_id })).await?
            }
            "podcast" => {
                let podcast_id = release.series_id.unwrap_or(0);
                self.tracked.podcasts.insert(make_td!(PodcastRef { podcast_id })).await?
            }
            _ => {
                let series_id = release.series_id.unwrap_or(0);
                self.tracked.series.insert(make_td!(SeriesRef { series_id, episode_ids })).await?
            }
        };

        info!(
            "Tracked download created: id={}, title={}, content_type={}",
            id, release.title, content_type
        );

        Ok(id)
    }

    /// Get all queue items with merged download client status.
    /// Returns a `QueueResult` containing both the tracked items and the raw
    /// downloads polled from each client (keyed by client_id), so callers can
    /// reuse them without hitting the download clients a second time.
    pub async fn get_queue(&self) -> Result<QueueResult> {
        let client_repo = DownloadClientRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());

        // Get all tracked downloads from in-memory stores (type-erased)
        let tracked = self.tracked.get_all_any().await;

        // Get all download clients for status lookup
        let clients = client_repo.get_all().await?;

        // Build client status map: (client_id, download_id) -> DownloadStatus
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
            let quality: QualityModel =
                serde_json::from_str(&td.quality).unwrap_or_default();

            // Get live status from download client
            let live_status =
                client_status_map.get(&(td.client_id, td.download_id.clone()));

            // If the client was successfully polled but the download is gone,
            // auto-remove the tracked record.
            if live_status.is_none() && polled_clients.contains(&td.client_id) {
                info!(
                    "Auto-removing tracked download {} '{}' — no longer in download client (client_id={})",
                    td.id, td.title, td.client_id
                );
                self.tracked.remove_by_id(td.id).await;
                continue;
            }

            // Determine queue status and state from live client data
            let (queue_status, tracked_state, size, size_left, timeleft, estimated_completion, output_path, error_message) =
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
                        DownloadState::Queued
                        | DownloadState::Downloading
                        | DownloadState::Stalled
                        | DownloadState::Paused => TrackedDownloadState::Downloading,
                        DownloadState::Seeding
                        | DownloadState::Completed => TrackedDownloadState::ImportPending,
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
                        live.size,
                        live.size_left,
                        timeleft,
                        estimated,
                        live.output_path.clone(),
                        live.error_message.clone(),
                    )
                } else {
                    // Download client unavailable — show with zero progress
                    (
                        QueueStatus::DownloadClientUnavailable,
                        TrackedDownloadState::Downloading,
                        0i64,
                        0i64,
                        None,
                        None,
                        None,
                        None,
                    )
                };

            // Resolve episode info for series downloads
            let (resolved_episode_id, season_number, episode_numbers) =
                if !td.episode_ids.is_empty() {
                    if let Ok(Some(ep)) = episode_repo.get_by_id(td.episode_ids[0]).await {
                        let mut ep_nums: Vec<i32> = vec![ep.episode_number];
                        for &ep_id in td.episode_ids.iter().skip(1) {
                            if let Ok(Some(other_ep)) = episode_repo.get_by_id(ep_id).await {
                                ep_nums.push(other_ep.episode_number);
                            }
                        }
                        (td.episode_ids[0], ep.season_number, ep_nums)
                    } else {
                        (0, 0, vec![])
                    }
                } else if td.series_id > 0 {
                    // Fallback: parse title to extract episode info
                    use crate::core::parser::parse_title;
                    let mut fallback = (0i64, 0i32, vec![]);
                    if let Some(info) = parse_title(&td.title) {
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
                    fallback
                } else {
                    (0, 0, vec![])
                };

            // Check if ALL tracked episodes have files
            let episode_has_file = if !td.episode_ids.is_empty() {
                let mut all_have_files = true;
                for &ep_id in &td.episode_ids {
                    match episode_repo.get_by_id(ep_id).await {
                        Ok(Some(ep)) if ep.has_file => {}
                        _ => {
                            all_have_files = false;
                            break;
                        }
                    }
                }
                all_have_files
            } else {
                false
            };

            // Determine protocol from client model
            let protocol = clients
                .iter()
                .find(|c| c.id == td.client_id)
                .map(|c| match c.protocol {
                    1 => Protocol::Usenet,
                    2 => Protocol::Torrent,
                    _ => Protocol::Unknown,
                })
                .unwrap_or(Protocol::Unknown);

            let client_name = client_name_map
                .get(&td.client_id)
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
                status_messages: vec![],
                error_message,
                download_id: Some(td.download_id),
                protocol,
                download_client: client_name,
                indexer: td.indexer.unwrap_or_default(),
                output_path,
                episode_has_file,
                movie_id: td.movie_id,
                artist_id: if td.artist_id > 0 {
                    Some(td.artist_id)
                } else {
                    None
                },
                audiobook_id: if td.audiobook_id > 0 {
                    Some(td.audiobook_id)
                } else {
                    None
                },
                size,
                sizeleft: size_left,
                timeleft,
                estimated_completion_time: estimated_completion,
                added: td.added,
                quality,
                seeds,
                leechers,
                seed_count,
                leech_count,
                content_type: td.content_type.to_string(),
            });
        }

        Ok(QueueResult {
            items: queue_items,
            client_downloads,
        })
    }

    /// Process the download queue — poll clients, detect completions,
    /// auto-clean imported downloads.
    ///
    /// Unlike the old DB-backed version, this does NOT persist status or
    /// output_path — those are derived from live client polling. The only
    /// mutation is deleting records that are confirmed imported.
    pub async fn process_queue(&self) -> Result<()> {
        let client_repo = DownloadClientRepository::new(self.db.clone());

        let tracked = self.tracked.get_all_any().await;
        if tracked.is_empty() {
            return Ok(());
        }

        debug!("Processing {} tracked downloads", tracked.len());

        let clients = client_repo.get_all().await?;

        for td in &tracked {
            let client_model = match clients.iter().find(|c| c.id == td.client_id) {
                Some(c) => c,
                None => {
                    warn!(
                        "Download client {} not found for tracked download {}",
                        td.client_id, td.id
                    );
                    continue;
                }
            };

            if !client_model.enable {
                continue;
            }

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
                    debug!(
                        "Download {} ('{}') not found in client {}, skipping",
                        td.download_id, td.title, client_model.name
                    );
                    continue;
                }
                Err(e) => {
                    debug!("Failed to get download status: {}", e);
                    continue;
                }
            };

            // Check for completion — if target media already imported, remove tracking
            match live_status.status {
                DownloadState::Completed | DownloadState::Seeding => {
                    let already_imported = self.check_already_imported(td).await;
                    if already_imported {
                        info!(
                            "Download '{}' already imported, removing from tracking",
                            td.title
                        );
                        self.tracked.remove_by_id(td.id).await;
                    }
                }
                DownloadState::Failed => {
                    error!(
                        "Download failed: {} - {:?}",
                        td.title, live_status.error_message
                    );
                }
                _ => {}
            }
        }

        // Auto-clean tracked downloads that have already been imported
        if let Err(e) = self.cleanup_imported_downloads().await {
            warn!("ProcessQueue: auto-clean failed: {}", e);
        }

        Ok(())
    }

    /// Check if the target movie/episode already has imported files.
    async fn check_already_imported(&self, td: &AnyTrackedDownload) -> bool {
        use crate::core::datastore::repositories::{
            EpisodeRepository as EpRepo, MovieFileRepository,
        };

        // Movie check
        if td.movie_id > 0 {
            let movie_file_repo = MovieFileRepository::new(self.db.clone());
            if let Ok(Some(_)) = movie_file_repo.get_by_movie_id(td.movie_id).await {
                return true;
            }
        }

        // Episode check
        if td.series_id > 0 && !td.episode_ids.is_empty() {
            let ep_repo = EpRepo::new(self.db.clone());
            for &ep_id in &td.episode_ids {
                match ep_repo.get_by_id(ep_id).await {
                    Ok(Some(ep)) if ep.has_file => {}
                    _ => return false,
                }
            }
            return true;
        }

        false
    }

    /// Auto-remove tracked downloads whose target media has been fully imported.
    pub async fn cleanup_imported_downloads(&self) -> Result<usize> {
        use crate::core::datastore::repositories::{EpisodeFileRepository, MovieFileRepository};

        let episode_repo = EpisodeRepository::new(self.db.clone());
        let episode_file_repo = EpisodeFileRepository::new(self.db.clone());
        let movie_file_repo = MovieFileRepository::new(self.db.clone());

        // Get all tracked downloads across all stores
        let all_tracked = self.tracked.get_all_any().await;
        let mut cleaned = 0usize;

        // Build download client status map for output_path lookups
        let client_repo = DownloadClientRepository::new(self.db.clone());
        let clients = client_repo.get_all().await?;
        let mut output_paths: std::collections::HashMap<(i64, String), String> =
            std::collections::HashMap::new();

        for client_model in clients.iter().filter(|c| c.enable) {
            if let Ok(client) = create_client_from_model(client_model) {
                if let Ok(downloads) = client.get_downloads().await {
                    for dl in downloads {
                        if let Some(ref path) = dl.output_path {
                            output_paths
                                .insert((client_model.id, dl.id.clone()), path.clone());
                        }
                    }
                }
            }
        }

        for td in &all_tracked {
            // Get output_path from live client data
            let output_path = match output_paths.get(&(td.client_id, td.download_id.clone())) {
                Some(p) if !p.is_empty() => p.as_str(),
                _ => continue,
            };

            // --- Movie path ---
            if td.movie_id > 0 {
                let dest_file = match movie_file_repo.get_by_movie_id(td.movie_id).await {
                    Ok(Some(f)) => f,
                    _ => continue,
                };

                let dest_hash = dest_file
                    .file_hash
                    .as_deref()
                    .filter(|h| !h.is_empty())
                    .map(|h| h.to_string());

                if let Some(ref dest_hash) = dest_hash {
                    if let Some(source_file) = find_largest_video_file(output_path).await {
                        match crate::core::mediafiles::compute_file_hash(&source_file).await {
                            Ok(source_hash) if source_hash == *dest_hash => {
                                info!(
                                    "Auto-clean: movie download '{}' verified (hash match), removing from tracking",
                                    td.title
                                );
                                self.tracked.remove_by_id(td.id).await;
                                cleaned += 1;
                                continue;
                            }
                            Ok(_) => {
                                debug!(
                                    "Auto-clean: hash mismatch for movie '{}', keeping tracked",
                                    td.title
                                );
                                continue;
                            }
                            Err(e) => {
                                debug!(
                                    "Auto-clean: could not hash source for '{}': {}, falling back",
                                    td.title, e
                                );
                            }
                        }
                    }
                    info!(
                        "Auto-clean: movie download '{}' — file imported (source gone or unreadable), removing from tracking",
                        td.title
                    );
                    self.tracked.remove_by_id(td.id).await;
                    cleaned += 1;
                } else {
                    info!(
                        "Auto-clean: movie download '{}' — file exists in library (no hash to verify), removing from tracking",
                        td.title
                    );
                    self.tracked.remove_by_id(td.id).await;
                    cleaned += 1;
                }
                continue;
            }

            // --- Series / Anime path ---
            if td.series_id > 0 && !td.episode_ids.is_empty() {
                let mut all_have_files = true;
                let mut all_have_hashes = true;
                let mut single_ep_hash: Option<(i64, String)> = None;

                for &ep_id in &td.episode_ids {
                    let ep = match episode_repo.get_by_id(ep_id).await {
                        Ok(Some(e)) => e,
                        _ => {
                            all_have_files = false;
                            break;
                        }
                    };
                    if !ep.has_file {
                        all_have_files = false;
                        break;
                    }
                    if let Some(file_id) = ep.episode_file_id {
                        match episode_file_repo.get_by_id(file_id).await {
                            Ok(Some(ef))
                                if ef
                                    .file_hash
                                    .as_deref()
                                    .is_some_and(|h| !h.is_empty()) =>
                            {
                                if td.episode_ids.len() == 1 {
                                    single_ep_hash =
                                        Some((file_id, ef.file_hash.unwrap().clone()));
                                }
                            }
                            _ => {
                                all_have_hashes = false;
                            }
                        }
                    } else {
                        all_have_hashes = false;
                    }
                }

                if all_have_files && all_have_hashes {
                    if let Some((_, ref dest_hash)) = single_ep_hash {
                        if let Some(source_file) = find_largest_video_file(output_path).await {
                            match crate::core::mediafiles::compute_file_hash(&source_file).await {
                                Ok(source_hash) if source_hash == *dest_hash => {
                                    info!(
                                        "Auto-clean: episode download '{}' verified (hash match), removing from tracking",
                                        td.title
                                    );
                                    self.tracked.remove_by_id(td.id).await;
                                    cleaned += 1;
                                    continue;
                                }
                                Ok(_) => {
                                    debug!(
                                        "Auto-clean: hash mismatch for '{}', keeping tracked",
                                        td.title
                                    );
                                    continue;
                                }
                                Err(e) => {
                                    debug!(
                                        "Auto-clean: could not hash source for '{}': {}, falling back",
                                        td.title, e
                                    );
                                }
                            }
                        }
                    } else {
                        info!(
                            "Auto-clean: season pack '{}' — all {} episodes imported (hash-verified), removing from tracking",
                            td.title,
                            td.episode_ids.len()
                        );
                        self.tracked.remove_by_id(td.id).await;
                        cleaned += 1;
                        continue;
                    }
                }

                if all_have_files {
                    info!(
                        "Auto-clean: download '{}' — all {} episodes have files (no hash to verify), removing from tracking",
                        td.title,
                        td.episode_ids.len()
                    );
                    self.tracked.remove_by_id(td.id).await;
                    cleaned += 1;
                }
            }
        }

        if cleaned > 0 {
            info!(
                "Auto-clean: removed {} imported downloads from tracking",
                cleaned
            );
        }
        Ok(cleaned)
    }

    /// Remove a download from queue
    pub async fn remove(&self, id: i64, remove_from_client: bool, blocklist: bool) -> Result<()> {
        let client_repo = DownloadClientRepository::new(self.db.clone());

        // Find the tracked download across all stores
        let td = self
            .tracked
            .find_by_id(id)
            .await
            .context("Tracked download not found")?;

        // Remove from download client if requested
        if remove_from_client {
            let client_model = client_repo.get_by_id(td.client_id).await?;
            if let Some(model) = client_model {
                if let Ok(client) = create_client_from_model(&model) {
                    if let Err(e) = client.remove(&td.download_id, true).await {
                        warn!("Failed to remove download from client: {}", e);
                    }
                }
            }
        }

        if blocklist {
            // TODO: Add to blocklist table
            info!("Would add to blocklist: {}", td.title);
        }

        if remove_from_client {
            // Full removal: delete tracking record
            self.tracked.remove_by_id(id).await;
            info!("Removed tracked download: {} ({})", td.title, id);
        } else {
            // Soft removal: add to suppressed list so it doesn't reappear
            // as an untracked download
            let suppressed = TrackedDownload {
                id: 0,
                download_id: td.download_id.clone(),
                client_id: td.client_id,
                content: SuppressedRef,
                title: td.title.clone(),
                quality: String::new(),
                indexer: None,
                added: Utc::now(),
                is_upgrade: false,
            };
            let _ = self.tracked.suppressed.insert(suppressed).await;
            self.tracked.remove_by_id(id).await;
            info!(
                "Suppressed tracked download: {} ({})",
                td.title, id
            );
        }

        Ok(())
    }

    /// Grab a release and send to download client.
    pub async fn grab_release(
        &self,
        release: &ReleaseInfo,
        episode_ids: Vec<i64>,
        movie_id: Option<i64>,
        content_type: &str,
    ) -> Result<i64> {
        let client_repo = DownloadClientRepository::new(self.db.clone());

        let clients = client_repo.get_all().await?;

        let protocol_num = match release.protocol {
            crate::core::indexers::Protocol::Usenet => 1,
            crate::core::indexers::Protocol::Torrent => 2,
            crate::core::indexers::Protocol::Unknown => 0,
        };

        let client_model = clients
            .iter()
            .filter(|c| c.enable && c.protocol == protocol_num)
            .min_by_key(|c| c.priority)
            .context("No enabled download client for this protocol")?;

        let client = create_client_from_model(client_model)?;

        let settings = serde_json::from_str::<serde_json::Value>(&client_model.settings)
            .unwrap_or(serde_json::json!({}));

        let category_key = if movie_id.is_some() {
            "movieCategory"
        } else if release.indexer == "music" {
            "musicCategory"
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

        // Prefer magnet links
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
                false,
                movie_id,
                content_type,
            )
            .await?;

        Ok(tracked_id)
    }

    /// Reconcile untracked downloads from all clients.
    ///
    /// Scans every enabled download client, parses torrent/NZB names, matches
    /// them to series and episodes in the database, and creates tracked download
    /// entries so the downloads appear in the queue with proper metadata.
    pub async fn reconcile_downloads(&self) -> Result<usize> {
        let client_repo = DownloadClientRepository::new(self.db.clone());
        let series_repo = SeriesRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let movie_repo = MovieRepository::new(self.db.clone());

        let clients = client_repo.get_all().await?;
        let all_series = series_repo.get_all().await?;
        let all_movies = movie_repo.get_all().await?;

        let mut reconciled = 0usize;

        // Purge tracked downloads with fake qbt-* UUIDs
        let series_items = self.tracked.series.get_all().await;
        for td in series_items
            .iter()
            .filter(|t| t.download_id.starts_with("qbt-"))
        {
            info!(
                "Purging stale tracked download with fake ID: {} ({})",
                td.download_id, td.title
            );
            self.tracked.series.remove(td.id).await;
        }

        // Collect all known download IDs for dedup
        let known_ids = self.tracked.all_download_ids().await;

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
                // Skip if already tracked (in any store, including suppressed)
                if known_ids.contains(&(client_model.id, dl.id.clone())) {
                    continue;
                }

                // Parse the download name
                use crate::core::parser::{parse_title, title_matches_series};
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

                let quality_json =
                    serde_json::to_string(&parsed.quality).unwrap_or_else(|_| "{}".to_string());

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

                    let td = TrackedDownload {
                        id: 0,
                        download_id: dl.id.clone(),
                        client_id: client_model.id,
                        content: SeriesRef {
                            series_id: series.id,
                            episode_ids: episode_ids.clone(),
                        },
                        title: dl.name.clone(),
                        quality: quality_json,
                        indexer: None,
                        added: Utc::now(),
                        is_upgrade: false,
                    };

                    match self.tracked.series.insert(td).await {
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

                    let matched_movie = all_movies
                        .iter()
                        .filter(|m| {
                            let clean = normalize_title(&m.clean_title);
                            clean.len() >= 4 && name_normalized.contains(clean.as_str())
                        })
                        .max_by_key(|m| normalize_title(&m.clean_title).len());

                    if let Some(movie) = matched_movie {
                        let td = TrackedDownload {
                            id: 0,
                            download_id: dl.id.clone(),
                            client_id: client_model.id,
                            content: MovieRef {
                                movie_id: movie.id,
                            },
                            title: dl.name.clone(),
                            quality: quality_json,
                            indexer: None,
                            added: Utc::now(),
                            is_upgrade: false,
                        };

                        match self.tracked.movies.insert(td).await {
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
                    } else {
                        debug!(
                            "Reconcile: no series or movie match for '{}'",
                            dl.name
                        );
                    }
                }
            }
        }

        info!("Reconcile downloads complete: {} newly tracked", reconciled);

        if let Err(e) = self.cleanup_imported_downloads().await {
            warn!("Reconcile: auto-clean failed: {}", e);
        }

        Ok(reconciled)
    }

    /// Download a torrent file from a URL, manually following redirects with logging.
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
async fn find_largest_video_file(path: &str) -> Option<std::path::PathBuf> {
    use crate::core::scanner::is_video_file;
    let path = std::path::PathBuf::from(path);

    tokio::task::spawn_blocking(move || {
        if !path.exists() {
            return None;
        }

        if path.is_file() {
            return if is_video_file(&path) {
                Some(path)
            } else {
                None
            };
        }

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
