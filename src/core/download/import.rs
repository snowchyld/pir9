#![allow(dead_code, unused_imports)]
//! Download import service
//! Processes completed downloads and imports them into the library
//!
//! Handles both single-file downloads and multi-file season/multi-season packs.
//! For season packs, each video file is individually matched to its episode(s)
//! via filename parsing, creating separate episode_file records per file.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::configuration::MediaConfig;
use crate::core::datastore::models::{
    EpisodeDbModel, EpisodeFileDbModel, HistoryDbModel, SeriesDbModel,
};
use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeFileRepository, EpisodeRepository, HistoryRepository,
    SeriesRepository, TrackedDownloadRepository,
};
use crate::core::datastore::Database;
use crate::core::download::clients::{create_client_from_model, DownloadState, DownloadStatus};
use crate::core::mediafiles::{compute_file_hash, derive_quality_from_media, MediaAnalyzer};
use crate::core::naming::{self, EpisodeNamingContext};
use crate::core::parser::{best_series_match, parse_title, ParsedEpisodeInfo};
use crate::core::scanner::{match_special_by_title, parse_episodes_from_filename};

/// Compute the destination path for an imported file (pure computation, no I/O).
///
/// This standalone function extracts the path logic from `ImportService` so the
/// `ScanResultConsumer` can plan file moves without needing filesystem access.
pub fn compute_destination_path(
    media_config: &MediaConfig,
    series: &SeriesDbModel,
    season_number: i32,
    source_filename: &str,
    episodes: &[EpisodeDbModel],
    parsed_info: &ParsedEpisodeInfo,
) -> PathBuf {
    let ext = std::path::Path::new(source_filename)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "mkv".to_string());

    let mut dest = PathBuf::from(&series.path);

    // Add season folder if enabled
    if series.season_folder {
        dest.push(naming::build_season_folder(media_config, season_number));
    }

    // Build filename: use naming template if enabled, otherwise keep original
    let filename = if media_config.rename_episodes && !episodes.is_empty() {
        let ctx = EpisodeNamingContext {
            series,
            episodes,
            quality: &parsed_info.quality,
            release_group: parsed_info.release_group.as_deref(),
        };
        let named = naming::build_episode_filename(media_config, &ctx);
        format!("{}.{}", named, ext)
    } else {
        let source_stem = std::path::Path::new(source_filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "video".to_string());
        format!("{}.{}", source_stem, ext)
    };

    dest.push(filename);
    dest
}

/// Match parsed info to a series in the database (standalone, no ImportService needed).
pub async fn match_series_standalone(
    db: &Database,
    info: &ParsedEpisodeInfo,
) -> anyhow::Result<Option<SeriesDbModel>> {
    let series_repo = SeriesRepository::new(db.clone());
    let all_series = series_repo.get_all().await?;
    Ok(
        best_series_match(info, &all_series)
            .map(|idx| all_series.into_iter().nth(idx).unwrap()),
    )
}

/// Match episodes for a series based on parsed info (standalone, no ImportService needed).
pub async fn match_episodes_standalone(
    db: &Database,
    series: &SeriesDbModel,
    info: &ParsedEpisodeInfo,
) -> anyhow::Result<Vec<EpisodeDbModel>> {
    let episode_repo = EpisodeRepository::new(db.clone());
    let mut matched_episodes = Vec::new();

    // Handle multi-season full packs
    if info.full_season && info.is_multi_season {
        return episode_repo.get_by_series_id(series.id).await;
    }

    // Handle full season
    if info.full_season {
        if let Some(season) = info.season_number {
            return episode_repo
                .get_by_series_and_season(series.id, season)
                .await;
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

/// Result of an import operation
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Whether the import was successful
    pub success: bool,
    /// Episode file IDs created during import
    pub episode_file_ids: Vec<i64>,
    /// Episodes that were linked to files
    pub episode_ids: Vec<i64>,
    /// Error message if import failed
    pub error_message: Option<String>,
    /// Paths where files were imported
    pub import_paths: Vec<PathBuf>,
    /// Number of files successfully imported
    pub files_imported: usize,
    /// Number of files skipped (unmatched extras, samples)
    pub files_skipped: usize,
}

impl ImportResult {
    fn failure(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            episode_file_ids: vec![],
            episode_ids: vec![],
            error_message: Some(msg.into()),
            import_paths: vec![],
            files_imported: 0,
            files_skipped: 0,
        }
    }
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
    /// Manual episode overrides from import preview UI: source_file → [(season, episode)]
    pub overrides: std::collections::HashMap<String, Vec<(i32, i32)>>,
    /// Source file paths to force-reimport even if identical (same size as existing)
    pub force_reimport: std::collections::HashSet<String>,
    /// Source file paths to skip during import (user chose "Do not import")
    pub skip_files: std::collections::HashSet<String>,
}

/// Result of importing a single file
struct SingleFileResult {
    file_id: i64,
    episode_ids: Vec<i64>,
    dest_path: PathBuf,
}

/// Import service for processing completed downloads
pub struct ImportService {
    db: Database,
    media_config: MediaConfig,
}

impl ImportService {
    pub fn new(db: Database, media_config: MediaConfig) -> Self {
        Self { db, media_config }
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
                Ok(client) => match client.get_downloads().await {
                    Ok(downloads) => {
                        for download in downloads {
                            if download.status == DownloadState::Completed {
                                if let Some(output_path) = &download.output_path {
                                    let pending = self
                                        .create_pending_import(
                                            &download,
                                            client_model.id,
                                            &client_model.name,
                                            output_path,
                                        )
                                        .await;

                                    if let Some(p) = pending {
                                        pending_imports.push(p);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get downloads from {}: {}", client_model.name, e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to create client for {}: {}", client_model.name, e);
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
            overrides: std::collections::HashMap::new(),
            force_reimport: std::collections::HashSet::new(),
            skip_files: std::collections::HashSet::new(),
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

    /// Try to match parsed info to a series in the database (year-aware).
    ///
    /// Uses scoring to pick the best candidate, considering both title and year
    /// when the release title contains a year (e.g., "The Flash 2014 S01E01").
    pub async fn match_series(&self, info: &ParsedEpisodeInfo) -> Result<Option<SeriesDbModel>> {
        let series_repo = SeriesRepository::new(self.db.clone());
        let all_series = series_repo.get_all().await?;

        Ok(
            best_series_match(info, &all_series)
                .map(|idx| all_series.into_iter().nth(idx).unwrap()),
        )
    }

    /// Match episodes for a series based on parsed info
    pub async fn match_episodes(
        &self,
        series: &SeriesDbModel,
        info: &ParsedEpisodeInfo,
    ) -> Result<Vec<EpisodeDbModel>> {
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let mut matched_episodes = Vec::new();

        // Handle multi-season full packs — return all series episodes so the
        // per-file parser can match each file to its correct season+episode
        if info.full_season && info.is_multi_season {
            return episode_repo.get_by_series_id(series.id).await;
        }

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

    /// Import a completed download into the library.
    ///
    /// Detects multi-file downloads (season packs) and imports each file
    /// individually, matching it to the correct episode(s) via filename parsing.
    pub async fn import(&self, pending: &PendingImport) -> Result<ImportResult> {
        // Validate we have necessary info
        let series = match &pending.series {
            Some(s) => s,
            None => return Ok(ImportResult::failure("No matching series found")),
        };

        if pending.episodes.is_empty() {
            return Ok(ImportResult::failure("No matching episodes found"));
        }

        let parsed_info = match &pending.parsed_info {
            Some(i) => i,
            None => return Ok(ImportResult::failure("Could not parse release title")),
        };

        // Find video files in the download path
        let video_files = self.find_video_files(&pending.output_path)?;

        if video_files.is_empty() {
            return Ok(ImportResult::failure("No video files found"));
        }

        // Single file → link all matched episodes to it (original behavior)
        // Multi-file → per-file episode matching (season pack import)
        if video_files.len() <= 1 {
            self.import_single_download(
                &video_files[0],
                series,
                &pending.episodes,
                parsed_info,
                &pending.download_id,
                &pending.title,
            )
            .await
        } else {
            self.import_season_pack(
                &video_files,
                series,
                &pending.episodes,
                parsed_info,
                &pending.download_id,
                &pending.title,
                &pending.overrides,
                &pending.output_path,
                &pending.force_reimport,
                &pending.skip_files,
            )
            .await
        }
    }

    /// Import a single-file download, linking all matched episodes to one file.
    async fn import_single_download(
        &self,
        source_file: &Path,
        series: &SeriesDbModel,
        episodes: &[EpisodeDbModel],
        parsed_info: &ParsedEpisodeInfo,
        download_id: &str,
        download_title: &str,
    ) -> Result<ImportResult> {
        let season_number = episodes.first().map(|e| e.season_number).unwrap_or(1);

        let result = self
            .import_single_file(
                source_file,
                series,
                episodes,
                season_number,
                parsed_info,
                download_id,
                download_title,
            )
            .await?;

        crate::core::logging::log_info(
            "DownloadImported",
            &format!(
                "Imported '{}' -> '{}' ({} episodes)",
                download_title,
                result.dest_path.display(),
                result.episode_ids.len()
            ),
        )
        .await;

        Ok(ImportResult {
            success: true,
            episode_file_ids: vec![result.file_id],
            episode_ids: result.episode_ids,
            error_message: None,
            import_paths: vec![result.dest_path],
            files_imported: 1,
            files_skipped: 0,
        })
    }

    /// Import a multi-file season pack by matching each file to its episode(s).
    ///
    /// For each video file, parses the filename to extract season/episode numbers,
    /// matches against candidate episodes, then imports each file individually.
    /// Unmatched files (samples, extras) are skipped.
    /// If two files claim the same episode, the larger file wins.
    async fn import_season_pack(
        &self,
        video_files: &[PathBuf],
        series: &SeriesDbModel,
        all_episodes: &[EpisodeDbModel],
        parsed_info: &ParsedEpisodeInfo,
        download_id: &str,
        download_title: &str,
        file_overrides: &HashMap<String, Vec<(i32, i32)>>,
        download_path: &Path,
        force_reimport: &std::collections::HashSet<String>,
        skip_files: &std::collections::HashSet<String>,
    ) -> Result<ImportResult> {
        // Build episode lookup: (season, episode_number) -> EpisodeDbModel
        let mut episode_map: HashMap<(i32, i32), &EpisodeDbModel> = HashMap::new();
        for ep in all_episodes {
            episode_map.insert((ep.season_number, ep.episode_number), ep);
        }

        // Scene numbering fallback: when episode ordering is non-default (e.g. DVD),
        // season_number/episode_number hold the DVD values but filenames use aired numbers
        // which are backed up in scene_season_number/scene_episode_number.
        let mut scene_map: HashMap<(i32, i32), &EpisodeDbModel> = HashMap::new();
        for ep in all_episodes {
            if let (Some(ss), Some(se)) = (ep.scene_season_number, ep.scene_episode_number) {
                scene_map.insert((ss, se), ep);
            }
        }

        // Load existing episode file sizes for same-size skip detection.
        // If a source file is the same size as the existing file, it's almost certainly
        // identical — skip importing it to avoid wasteful disk I/O.
        let episode_file_repo = EpisodeFileRepository::new(self.db.clone());
        let existing_file_sizes: HashMap<i64, i64> = episode_file_repo
            .get_by_series_id(series.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|f| (f.id, f.size))
            .collect();

        // Phase 1: Match each file to episodes, resolving duplicates by file size
        // Key: episode DB id -> (file path, file size, all matched episodes for that file)
        let mut file_assignments: HashMap<PathBuf, (Vec<&EpisodeDbModel>, u64)> = HashMap::new();
        // Track which episode is claimed by which file (for duplicate resolution)
        let mut episode_claims: HashMap<i64, (PathBuf, u64)> = HashMap::new();

        let mut files_skipped = 0;

        for video_file in video_files {
            let filename = match video_file.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => {
                    files_skipped += 1;
                    continue;
                }
            };

            // User explicitly chose "Do not import" for this file
            if skip_files.contains(filename)
                || skip_files.iter().any(|p| p.ends_with(filename))
            {
                tracing::info!(
                    "Season pack import: skipping '{}' — user chose Do not import",
                    filename,
                );
                files_skipped += 1;
                continue;
            }

            let parsed_eps = parse_episodes_from_filename(filename);

            // Fallback chain: title-based specials → manual overrides → skip
            let parsed_eps = if parsed_eps.is_empty() {
                let specials: Vec<(i32, &str)> = all_episodes
                    .iter()
                    .filter(|e| e.season_number == 0)
                    .map(|e| (e.episode_number, e.title.as_str()))
                    .collect();
                let special_match = if !specials.is_empty() {
                    match_special_by_title(filename, &series.title, &specials)
                } else {
                    None
                };
                if let Some(pair) = special_match {
                    tracing::info!(
                        "Season pack import: matched '{}' to special S00E{:02} by title",
                        filename,
                        pair.1
                    );
                    vec![pair]
                } else {
                    // Check manual overrides from import preview UI
                    let relative = video_file
                        .strip_prefix(download_path)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| filename.to_string());
                    if let Some(pairs) = file_overrides
                        .get(&relative)
                        .or_else(|| file_overrides.get(filename))
                    {
                        tracing::info!(
                            "Season pack import: manual override '{}' → {:?}",
                            filename, pairs
                        );
                        pairs.clone()
                    } else {
                        tracing::debug!(
                            "Season pack import: skipping unmatched file '{}'",
                            filename
                        );
                        files_skipped += 1;
                        continue;
                    }
                }
            } else {
                parsed_eps
            };

            let file_size = std::fs::metadata(video_file).map(|m| m.len()).unwrap_or(0);

            // Find matching episodes from our candidate set.
            // Try primary map first (current season/episode numbers), then fall back
            // to scene_map (original aired numbers backed up during ordering remap).
            let mut matched: Vec<&EpisodeDbModel> = Vec::new();
            for (season, ep_num) in &parsed_eps {
                if let Some(ep) = episode_map
                    .get(&(*season, *ep_num))
                    .or_else(|| scene_map.get(&(*season, *ep_num)))
                {
                    matched.push(ep);
                }
            }

            if matched.is_empty() {
                tracing::debug!(
                    "Season pack import: no episode match for '{}' (parsed {:?})",
                    filename,
                    parsed_eps
                );
                files_skipped += 1;
                continue;
            }

            // Same-size skip: if ALL matched episodes already have files of the same size,
            // the source file is identical — skip to avoid wasteful overwrite.
            // force_reimport bypasses this check (for damaged destination files).
            // Check both basename and relative path since frontend sends sourceFile (relative path).
            let is_force_reimport = force_reimport.contains(filename)
                || force_reimport.iter().any(|p| p.ends_with(filename));
            if !is_force_reimport
                && matched.iter().all(|ep| {
                    ep.episode_file_id
                        .and_then(|fid| existing_file_sizes.get(&fid))
                        .map(|&existing_size| existing_size == file_size as i64)
                        .unwrap_or(false)
                })
            {
                tracing::info!(
                    "Season pack import: skipping '{}' — same size as existing file(s)",
                    filename,
                );
                files_skipped += 1;
                continue;
            }

            // Duplicate resolution: if another file already claimed an episode,
            // the larger file wins
            let mut dominated = false;
            for ep in &matched {
                if let Some((existing_path, existing_size)) = episode_claims.get(&ep.id) {
                    if *existing_size >= file_size {
                        // Existing file is larger or equal — this file loses
                        tracing::debug!(
                            "Season pack import: '{}' loses S{:02}E{:02} to '{}' (larger)",
                            filename,
                            ep.season_number,
                            ep.episode_number,
                            existing_path.display()
                        );
                        dominated = true;
                        break;
                    }
                    // This file is larger — evict the existing claim
                    tracing::debug!(
                        "Season pack import: '{}' takes S{:02}E{:02} from '{}' (larger)",
                        filename,
                        ep.season_number,
                        ep.episode_number,
                        existing_path.display()
                    );
                    // Remove episode from previous file's assignment
                    if let Some((eps, _)) = file_assignments.get_mut(existing_path) {
                        eps.retain(|e| e.id != ep.id);
                    }
                }
            }

            if dominated {
                files_skipped += 1;
                continue;
            }

            // Register claims
            for ep in &matched {
                episode_claims.insert(ep.id, (video_file.clone(), file_size));
            }

            file_assignments.insert(video_file.clone(), (matched, file_size));
        }

        // Remove file assignments that lost all their episodes to larger files
        file_assignments.retain(|path, (eps, _)| {
            if eps.is_empty() {
                tracing::debug!(
                    "Season pack import: dropping '{}' (all episodes reassigned)",
                    path.display()
                );
                false
            } else {
                true
            }
        });

        if file_assignments.is_empty() {
            return Ok(ImportResult::failure(
                "No video files could be matched to episodes",
            ));
        }

        // Phase 2: Import each matched file
        let mut result = ImportResult {
            success: true,
            episode_file_ids: Vec::new(),
            episode_ids: Vec::new(),
            error_message: None,
            import_paths: Vec::new(),
            files_imported: 0,
            files_skipped,
        };

        // Sort by path for deterministic import order
        let mut sorted_files: Vec<_> = file_assignments.into_iter().collect();
        sorted_files.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (video_file, (matched_episodes, _)) in &sorted_files {
            // Use first matched episode's season for destination path
            let season_number = matched_episodes
                .first()
                .map(|e| e.season_number)
                .unwrap_or(1);

            let episodes_owned: Vec<EpisodeDbModel> =
                matched_episodes.iter().map(|e| (*e).clone()).collect();

            match self
                .import_single_file(
                    video_file,
                    series,
                    &episodes_owned,
                    season_number,
                    parsed_info,
                    download_id,
                    download_title,
                )
                .await
            {
                Ok(single) => {
                    result.episode_file_ids.push(single.file_id);
                    result.episode_ids.extend(single.episode_ids);
                    result.import_paths.push(single.dest_path);
                    result.files_imported += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        "Season pack import: failed to import '{}': {}",
                        video_file.display(),
                        e
                    );
                    result.files_skipped += 1;
                }
            }
        }

        if result.files_imported == 0 {
            result.success = false;
            result.error_message = Some("All file imports failed".to_string());
        }

        crate::core::logging::log_info(
            "DownloadImported",
            &format!(
                "Season pack '{}': imported {} files, skipped {}, {} episodes",
                download_title,
                result.files_imported,
                result.files_skipped,
                result.episode_ids.len()
            ),
        )
        .await;

        Ok(result)
    }

    /// Import a single video file: move to library, probe media, hash, insert DB record,
    /// link episodes, and record history.
    async fn import_single_file(
        &self,
        source_file: &Path,
        series: &SeriesDbModel,
        episodes: &[EpisodeDbModel],
        season_number: i32,
        parsed_info: &ParsedEpisodeInfo,
        download_id: &str,
        download_title: &str,
    ) -> Result<SingleFileResult> {
        let dest_path =
            self.build_destination_path(series, season_number, source_file, episodes, parsed_info)?;

        // Create destination directory
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create destination directory")?;
        }

        // Copy the file to library (source is never deleted — download client
        // manages cleanup via its own retention/seeding rules)
        let file_size = std::fs::metadata(source_file)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        std::fs::copy(source_file, &dest_path).context("Failed to copy file")?;

        // Create episode file record
        let relative_path = dest_path
            .strip_prefix(&series.path)
            .unwrap_or(&dest_path)
            .to_string_lossy()
            .to_string();

        // Real media analysis via FFmpeg probe
        let media_info_result = MediaAnalyzer::analyze(&dest_path).await;
        let media_info = media_info_result
            .as_ref()
            .ok()
            .and_then(|info| serde_json::to_string(info).ok());

        // Derive quality from actual resolution when available, fallback to parsed
        let quality_str = match &media_info_result {
            Ok(info) => {
                let quality = derive_quality_from_media(info, &dest_path.to_string_lossy());
                serde_json::to_string(&quality).unwrap_or_default()
            }
            Err(_) => serde_json::to_string(&parsed_info.quality).unwrap_or_default(),
        };

        // BLAKE3 file hash
        let file_hash = compute_file_hash(&dest_path).await.ok();

        let episode_file = EpisodeFileDbModel {
            id: 0, // Will be set by insert
            series_id: series.id,
            season_number,
            relative_path: relative_path.clone(),
            path: dest_path.to_string_lossy().to_string(),
            size: file_size,
            date_added: Utc::now(),
            scene_name: Some(download_title.to_string()),
            release_group: parsed_info.release_group.clone(),
            quality: quality_str,
            languages: serde_json::to_string(&parsed_info.languages).unwrap_or_default(),
            media_info,
            original_file_path: Some(source_file.to_string_lossy().to_string()),
            file_hash,
        };

        let file_repo = EpisodeFileRepository::new(self.db.clone());
        let file_id = file_repo.insert(&episode_file).await?;

        // Update episodes to link to the file
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let mut episode_ids = Vec::new();

        for episode in episodes {
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
            download_title,
            &parsed_info.quality,
            download_id,
        )
        .await?;

        Ok(SingleFileResult {
            file_id,
            episode_ids,
            dest_path,
        })
    }

    /// Find video files in a path
    fn find_video_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let video_extensions = crate::core::scanner::VIDEO_EXTENSIONS;

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

    /// Build the destination path for an imported file.
    ///
    /// When `rename_episodes` is enabled, uses the naming template engine to
    /// generate a properly formatted filename. Otherwise keeps the original name.
    fn build_destination_path(
        &self,
        series: &SeriesDbModel,
        season_number: i32,
        source_file: &Path,
        episodes: &[EpisodeDbModel],
        parsed_info: &ParsedEpisodeInfo,
    ) -> Result<PathBuf> {
        let ext = source_file
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "mkv".to_string());

        let mut dest = PathBuf::from(&series.path);

        // Add season folder if enabled
        if series.season_folder {
            dest.push(naming::build_season_folder(
                &self.media_config,
                season_number,
            ));
        }

        // Build filename: use naming template if enabled, otherwise keep original
        let filename = if self.media_config.rename_episodes && !episodes.is_empty() {
            let ctx = EpisodeNamingContext {
                series,
                episodes,
                quality: &parsed_info.quality,
                release_group: parsed_info.release_group.as_deref(),
            };
            let named = naming::build_episode_filename(&self.media_config, &ctx);
            format!("{}.{}", named, ext)
        } else {
            let source_name = source_file
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "video".to_string());
            format!("{}.{}", source_name, ext)
        };

        dest.push(filename);
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
                id: 0,
                series_id: Some(series_id),
                episode_id: Some(*episode_id),
                movie_id: None,
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

    /// Process all completed downloads (check, import, cleanup)
    pub async fn process_completed_downloads(
        &self,
        mark_imported: bool,
    ) -> Result<Vec<ImportResult>> {
        let pending = self.check_for_completed_downloads().await?;
        let mut results = Vec::new();

        for item in pending {
            let result = self.import(&item).await?;

            if result.success && mark_imported {
                // Mark tracked download as Imported so it disappears from queue.
                // Never remove from the download client — user controls seeding.
                self.mark_tracked_imported(&item).await;
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Mark the tracked download as Imported (state 4) after successful import
    pub async fn mark_tracked_imported(&self, pending: &PendingImport) {
        use crate::core::queue::TrackedDownloadState;

        let repo = TrackedDownloadRepository::new(self.db.clone());
        if let Ok(Some(td)) = repo
            .get_by_download_id(pending.download_client_id, &pending.download_id)
            .await
        {
            if let Err(e) = repo
                .update_status(td.id, TrackedDownloadState::Imported as i32, "[]", None)
                .await
            {
                tracing::warn!(
                    "Failed to mark tracked download as imported for '{}': {}",
                    pending.title,
                    e
                );
            }
        }
    }
}
