//! Media files module
//! Episode file management, media info, and file operations

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Episode file entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeFile {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub episode_numbers: Vec<i32>,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub date_added: chrono::DateTime<chrono::Utc>,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub quality: crate::core::profiles::qualities::QualityModel,
    pub languages: Vec<crate::core::profiles::languages::Language>,
    pub media_info: Option<MediaInfoModel>,
    pub original_file_path: Option<String>,
}

/// Media info from file analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfoModel {
    pub audio_bitrate: Option<i64>,
    pub audio_channels: Option<f64>,
    pub audio_codec: Option<String>,
    pub audio_languages: Option<String>,
    pub audio_stream_count: Option<i32>,
    pub video_bit_depth: Option<i32>,
    pub video_bitrate: Option<i64>,
    pub video_codec: Option<String>,
    pub video_fps: Option<f64>,
    pub video_dynamic_range: Option<String>,
    pub video_dynamic_range_type: Option<String>,
    pub resolution: Option<String>,
    pub run_time: Option<String>,
    pub scan_type: Option<String>,
    pub subtitles: Option<String>,
}

/// Media file analyzer
pub struct MediaAnalyzer;

impl MediaAnalyzer {
    /// Analyze a media file and extract media info
    pub async fn analyze(path: &std::path::Path) -> anyhow::Result<MediaInfoModel> {
        // This would use a media info library like mediainfo
        Ok(MediaInfoModel {
            audio_bitrate: None,
            audio_channels: None,
            audio_codec: None,
            audio_languages: None,
            audio_stream_count: None,
            video_bit_depth: None,
            video_bitrate: None,
            video_codec: None,
            video_fps: None,
            video_dynamic_range: None,
            video_dynamic_range_type: None,
            resolution: None,
            run_time: None,
            scan_type: None,
            subtitles: None,
        })
    }
    
    /// Get video resolution from media info
    pub fn get_resolution(media_info: &MediaInfoModel) -> Option<(i32, i32)> {
        media_info.resolution.as_ref().and_then(|res| {
            let parts: Vec<&str> = res.split('x').collect();
            if parts.len() == 2 {
                let width = parts[0].parse().ok()?;
                let height = parts[1].parse().ok()?;
                Some((width, height))
            } else {
                None
            }
        })
    }
}

// ========== File Operations ==========

/// Result of a file move operation
#[derive(Debug)]
pub struct MoveResult {
    /// Files successfully moved
    pub files_moved: usize,
    /// Directories created
    pub dirs_created: usize,
    /// Errors encountered (path, error message)
    pub errors: Vec<(PathBuf, String)>,
}

/// Move a series directory to a new location
///
/// This moves all contents from `source` to `destination`, preserving
/// the internal structure (season folders, etc.).
pub fn move_series_folder(source: &Path, destination: &Path) -> Result<MoveResult> {
    let mut result = MoveResult {
        files_moved: 0,
        dirs_created: 0,
        errors: Vec::new(),
    };

    // Validate source exists
    if !source.exists() {
        anyhow::bail!("Source directory does not exist: {}", source.display());
    }

    if !source.is_dir() {
        anyhow::bail!("Source is not a directory: {}", source.display());
    }

    // Check if destination already exists
    if destination.exists() {
        // If it's the same path (case-insensitive rename), we handle it
        if source.canonicalize().ok() == destination.canonicalize().ok() {
            info!("Source and destination are the same path, skipping move");
            return Ok(result);
        }
        anyhow::bail!(
            "Destination already exists: {}. Cannot overwrite.",
            destination.display()
        );
    }

    // Create destination parent directory if needed
    if let Some(parent) = destination.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .context(format!("Failed to create parent directory: {}", parent.display()))?;
            result.dirs_created += 1;
        }
    }

    info!(
        "Moving series folder from {} to {}",
        source.display(),
        destination.display()
    );

    // Try atomic rename first (works on same filesystem)
    if std::fs::rename(source, destination).is_ok() {
        info!("Moved series folder via rename (atomic)");
        // Count what we moved
        result.files_moved = count_files_recursive(destination);
        return Ok(result);
    }

    debug!("Atomic rename failed, falling back to copy+delete");

    // Fall back to recursive copy then delete
    copy_directory_recursive(source, destination, &mut result)?;

    // Only delete source if copy was successful and we moved files
    if result.files_moved > 0 && result.errors.is_empty() {
        if let Err(e) = std::fs::remove_dir_all(source) {
            warn!("Failed to remove source directory after copy: {}", e);
            result.errors.push((source.to_path_buf(), e.to_string()));
        }
    }

    Ok(result)
}

/// Copy a directory recursively
fn copy_directory_recursive(source: &Path, destination: &Path, result: &mut MoveResult) -> Result<()> {
    std::fs::create_dir_all(destination)
        .context(format!("Failed to create directory: {}", destination.display()))?;
    result.dirs_created += 1;

    for entry in std::fs::read_dir(source)
        .context(format!("Failed to read directory: {}", source.display()))?
    {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_path = destination.join(entry.file_name());

        if entry_path.is_dir() {
            copy_directory_recursive(&entry_path, &dest_path, result)?;
        } else {
            match std::fs::copy(&entry_path, &dest_path) {
                Ok(_) => {
                    debug!("Copied file: {} -> {}", entry_path.display(), dest_path.display());
                    result.files_moved += 1;
                }
                Err(e) => {
                    warn!("Failed to copy file {}: {}", entry_path.display(), e);
                    result.errors.push((entry_path, e.to_string()));
                }
            }
        }
    }

    Ok(())
}

/// Count files in a directory recursively
fn count_files_recursive(path: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                count += count_files_recursive(&entry_path);
            } else {
                count += 1;
            }
        }
    }
    count
}

/// Delete a series folder and all its contents
pub fn delete_series_folder(path: &Path) -> Result<usize> {
    if !path.exists() {
        info!("Series folder does not exist, nothing to delete: {}", path.display());
        return Ok(0);
    }

    if !path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", path.display());
    }

    let file_count = count_files_recursive(path);

    info!(
        "Deleting series folder: {} ({} files)",
        path.display(),
        file_count
    );

    std::fs::remove_dir_all(path)
        .context(format!("Failed to delete directory: {}", path.display()))?;

    Ok(file_count)
}

/// Update episode file paths after a series move
///
/// Updates the `path` and `relative_path` fields in episode files
/// to reflect the new series location.
pub fn update_episode_file_paths(
    old_series_path: &Path,
    new_series_path: &Path,
    episode_files: &mut [crate::core::datastore::models::EpisodeFileDbModel],
) {
    for file in episode_files.iter_mut() {
        // Update absolute path
        if let Ok(stripped) = Path::new(&file.path).strip_prefix(old_series_path) {
            file.path = new_series_path.join(stripped).to_string_lossy().to_string();
        }
        // relative_path should stay the same as it's relative to series root
    }
}
