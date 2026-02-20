//! Download import service
//! Processes completed downloads and imports them into the library

use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::datastore::models::{
    EpisodeDbModel, EpisodeFileDbModel, HistoryDbModel, SeriesDbModel,
};
use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeFileRepository, EpisodeRepository,
    HistoryRepository, SeriesRepository,
};
use crate::core::datastore::Database;
use crate::core::download::clients::{create_client_from_model, DownloadState, DownloadStatus};
use crate::core::parser::{parse_title, title_matches_series, ParsedEpisodeInfo};

/// Result of an import operation
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Whether the import was successful
    pub success: bool,
    /// The episode file ID if created
    pub episode_file_id: Option<i64>,
    /// Episodes that were linked to the file
    pub episode_ids: Vec<i64>,
    /// Error message if import failed
    pub error_message: Option<String>,
    /// Path where the file was imported
    pub import_path: Option<PathBuf>,
}

/// Pending import item from a completed download
#[derive(Debug, Clone)]
pub struct PendingImport {
    /// Download ID from the client
    pub download_id: String,
    /// Download client ID
    pub download_client_id: i64,
    /// Download client name
    pub download_client_name: String,
    /// Download title (release name)
    pub title: String,
    /// Output path where the download is located
    pub output_path: PathBuf,
    /// Parsed episode info from the title
    pub parsed_info: Option<ParsedEpisodeInfo>,
    /// Matched series
    pub series: Option<SeriesDbModel>,
    /// Matched episodes
    pub episodes: Vec<EpisodeDbModel>,
}

/// Import service for processing completed downloads
pub struct ImportService {
    db: Database,
}

impl ImportService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Check all download clients for completed downloads ready to import
    pub async fn check_for_completed_downloads(&self) -> Result<Vec<PendingImport>> {
        let client_repo = DownloadClientRepository::new(self.db.clone());
        let clients = client_repo.get_all().await?;

        let mut pending_imports = Vec::new();

        for client_model in clients {
            if !client_model.enable {
                continue;
            }

            match create_client_from_model(&client_model) {
                Ok(client) => {
                    match client.get_downloads().await {
                        Ok(downloads) => {
                            for download in downloads {
                                if download.status == DownloadState::Completed {
                                    if let Some(output_path) = &download.output_path {
                                        let pending = self.create_pending_import(
                                            &download,
                                            client_model.id,
                                            &client_model.name,
                                            output_path,
                                        ).await;

                                        if let Some(p) = pending {
                                            pending_imports.push(p);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to get downloads from {}: {}",
                                client_model.name,
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create client for {}: {}",
                        client_model.name,
                        e
                    );
                }
            }
        }

        Ok(pending_imports)
    }

    /// Create a pending import from a completed download
    async fn create_pending_import(
        &self,
        download: &DownloadStatus,
        client_id: i64,
        client_name: &str,
        output_path: &str,
    ) -> Option<PendingImport> {
        let parsed = parse_title(&download.name);

        let mut pending = PendingImport {
            download_id: download.id.clone(),
            download_client_id: client_id,
            download_client_name: client_name.to_string(),
            title: download.name.clone(),
            output_path: PathBuf::from(output_path),
            parsed_info: parsed.clone(),
            series: None,
            episodes: Vec::new(),
        };

        // Try to match to a series
        if let Some(ref info) = parsed {
            if let Ok(series) = self.match_series(info).await {
                if let Some(ref s) = series {
                    // Try to match episodes
                    if let Ok(episodes) = self.match_episodes(s, info).await {
                        pending.episodes = episodes;
                    }
                }
                pending.series = series;
            }
        }

        Some(pending)
    }

    /// Try to match parsed info to a series in the database
    pub async fn match_series(&self, info: &ParsedEpisodeInfo) -> Result<Option<SeriesDbModel>> {
        let series_repo = SeriesRepository::new(self.db.clone());
        let all_series = series_repo.get_all().await?;

        for series in all_series {
            if title_matches_series(info, &series.title) {
                return Ok(Some(series));
            }

            // Also try clean title
            if title_matches_series(info, &series.clean_title) {
                return Ok(Some(series));
            }
        }

        Ok(None)
    }

    /// Match episodes for a series based on parsed info
    pub async fn match_episodes(
        &self,
        series: &SeriesDbModel,
        info: &ParsedEpisodeInfo,
    ) -> Result<Vec<EpisodeDbModel>> {
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let mut matched_episodes = Vec::new();

        // Handle full season
        if info.full_season {
            if let Some(season) = info.season_number {
                let episodes = episode_repo
                    .get_by_series_and_season(series.id, season)
                    .await?;
                return Ok(episodes);
            }
        }

        // Handle standard season/episode
        if let Some(season) = info.season_number {
            for ep_num in &info.episode_numbers {
                if let Ok(Some(episode)) = episode_repo
                    .get_by_series_season_episode(series.id, season, *ep_num)
                    .await
                {
                    matched_episodes.push(episode);
                }
            }
        }

        // Handle absolute episode numbers (anime)
        if !info.absolute_episode_numbers.is_empty() {
            let all_episodes = episode_repo.get_by_series_id(series.id).await?;
            for abs_num in &info.absolute_episode_numbers {
                for ep in &all_episodes {
                    if ep.absolute_episode_number == Some(*abs_num) {
                        matched_episodes.push(ep.clone());
                        break;
                    }
                }
            }
        }

        // Handle daily episodes
        if info.is_daily {
            if let Some(air_date) = info.air_date {
                let all_episodes = episode_repo.get_by_series_id(series.id).await?;
                for ep in all_episodes {
                    if ep.air_date == Some(air_date) {
                        matched_episodes.push(ep);
                        break;
                    }
                }
            }
        }

        Ok(matched_episodes)
    }

    /// Import a completed download into the library
    pub async fn import(&self, pending: &PendingImport) -> Result<ImportResult> {
        // Validate we have necessary info
        let series = match &pending.series {
            Some(s) => s,
            None => {
                return Ok(ImportResult {
                    success: false,
                    episode_file_id: None,
                    episode_ids: vec![],
                    error_message: Some("No matching series found".to_string()),
                    import_path: None,
                });
            }
        };

        if pending.episodes.is_empty() {
            return Ok(ImportResult {
                success: false,
                episode_file_id: None,
                episode_ids: vec![],
                error_message: Some("No matching episodes found".to_string()),
                import_path: None,
            });
        }

        let parsed_info = match &pending.parsed_info {
            Some(i) => i,
            None => {
                return Ok(ImportResult {
                    success: false,
                    episode_file_id: None,
                    episode_ids: vec![],
                    error_message: Some("Could not parse release title".to_string()),
                    import_path: None,
                });
            }
        };

        // Find video files in the download path
        let video_files = self.find_video_files(&pending.output_path)?;

        if video_files.is_empty() {
            return Ok(ImportResult {
                success: false,
                episode_file_id: None,
                episode_ids: vec![],
                error_message: Some("No video files found".to_string()),
                import_path: None,
            });
        }

        // For now, take the largest video file
        let source_file = video_files
            .iter()
            .max_by_key(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))
            .unwrap();

        // Build destination path
        let season_number = pending.episodes.first().map(|e| e.season_number).unwrap_or(1);
        let dest_path = self.build_destination_path(series, season_number, source_file)?;

        // Create destination directory
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create destination directory")?;
        }

        // Move/copy the file
        let file_size = std::fs::metadata(source_file)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        // Try move first, fall back to copy
        if std::fs::rename(source_file, &dest_path).is_err() {
            std::fs::copy(source_file, &dest_path).context("Failed to copy file")?;
            // If copy succeeded, remove original
            let _ = std::fs::remove_file(source_file);
        }

        // Create episode file record
        let relative_path = dest_path
            .strip_prefix(&series.path)
            .unwrap_or(&dest_path)
            .to_string_lossy()
            .to_string();

        let episode_file = EpisodeFileDbModel {
            id: 0, // Will be set by insert
            series_id: series.id,
            season_number,
            relative_path: relative_path.clone(),
            path: dest_path.to_string_lossy().to_string(),
            size: file_size,
            date_added: Utc::now(),
            scene_name: Some(pending.title.clone()),
            release_group: parsed_info.release_group.clone(),
            quality: serde_json::to_string(&parsed_info.quality).unwrap_or_default(),
            languages: serde_json::to_string(&parsed_info.languages).unwrap_or_default(),
            media_info: None,
            original_file_path: Some(source_file.to_string_lossy().to_string()),
        };

        let file_repo = EpisodeFileRepository::new(self.db.clone());
        let file_id = file_repo.insert(&episode_file).await?;

        // Update episodes to link to the file
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let mut episode_ids = Vec::new();

        for episode in &pending.episodes {
            let mut updated_episode = episode.clone();
            updated_episode.episode_file_id = Some(file_id);
            updated_episode.has_file = true;
            episode_repo.update(&updated_episode).await?;
            episode_ids.push(episode.id);
        }

        // Record history
        self.record_history(
            series.id,
            &episode_ids,
            &pending.title,
            &parsed_info.quality,
            &pending.download_id,
        )
        .await?;

        // Log the import
        crate::core::logging::log_info(
            "DownloadImported",
            &format!(
                "Imported '{}' -> '{}'",
                pending.title,
                dest_path.display()
            ),
        )
        .await;

        Ok(ImportResult {
            success: true,
            episode_file_id: Some(file_id),
            episode_ids,
            error_message: None,
            import_path: Some(dest_path),
        })
    }

    /// Find video files in a path
    fn find_video_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let video_extensions = [
            "mkv", "mp4", "avi", "wmv", "mov", "m4v", "ts", "webm", "flv",
        ];

        let mut video_files = Vec::new();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if video_extensions.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                    video_files.push(path.to_path_buf());
                }
            }
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .min_depth(1)
                .max_depth(3)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if entry_path.is_file() {
                    if let Some(ext) = entry_path.extension() {
                        if video_extensions.contains(&ext.to_string_lossy().to_lowercase().as_str())
                        {
                            video_files.push(entry_path.to_path_buf());
                        }
                    }
                }
            }
        }

        Ok(video_files)
    }

    /// Build the destination path for an imported file
    fn build_destination_path(
        &self,
        series: &SeriesDbModel,
        season_number: i32,
        source_file: &Path,
    ) -> Result<PathBuf> {
        let ext = source_file
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "mkv".to_string());

        let source_name = source_file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "video".to_string());

        let mut dest = PathBuf::from(&series.path);

        // Add season folder if enabled
        if series.season_folder {
            if season_number == 0 {
                dest.push("Specials");
            } else {
                dest.push(format!("Season {:02}", season_number));
            }
        }

        // Use the original filename for now
        dest.push(format!("{}.{}", source_name, ext));

        Ok(dest)
    }

    /// Record import in history
    async fn record_history(
        &self,
        series_id: i64,
        episode_ids: &[i64],
        source_title: &str,
        quality: &crate::core::profiles::qualities::QualityModel,
        download_id: &str,
    ) -> Result<()> {
        let history_repo = HistoryRepository::new(self.db.clone());

        for episode_id in episode_ids {
            let history = HistoryDbModel {
                id: 0, // Will be set by insert
                series_id,
                episode_id: *episode_id,
                source_title: source_title.to_string(),
                quality: serde_json::to_string(quality).unwrap_or_default(),
                languages: "[]".to_string(),
                custom_formats: "[]".to_string(),
                custom_format_score: 0,
                quality_cutoff_not_met: false,
                date: Utc::now(),
                download_id: Some(download_id.to_string()),
                event_type: 3, // DownloadImported
                data: "{}".to_string(),
            };

            history_repo.insert(&history).await?;
        }

        Ok(())
    }

    /// Clean up a download from the download client after successful import
    pub async fn cleanup_download(&self, pending: &PendingImport, delete_files: bool) -> Result<()> {
        let client_repo = DownloadClientRepository::new(self.db.clone());

        if let Some(client_model) = client_repo.get_by_id(pending.download_client_id).await? {
            let client = create_client_from_model(&client_model)?;
            client.remove(&pending.download_id, delete_files).await?;

            tracing::info!(
                "Removed completed download '{}' from {}",
                pending.title,
                client_model.name
            );
        }

        Ok(())
    }

    /// Process all completed downloads (check, import, cleanup)
    pub async fn process_completed_downloads(&self, remove_from_client: bool) -> Result<Vec<ImportResult>> {
        let pending = self.check_for_completed_downloads().await?;
        let mut results = Vec::new();

        for item in pending {
            let result = self.import(&item).await?;

            if result.success && remove_from_client {
                // Clean up from download client
                if let Err(e) = self.cleanup_download(&item, false).await {
                    tracing::warn!(
                        "Failed to cleanup download '{}': {}",
                        item.title,
                        e
                    );
                }
            }

            results.push(result);
        }

        Ok(results)
    }
}
