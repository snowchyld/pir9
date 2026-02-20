#![allow(dead_code, unused_imports)]
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

// ========== Real media probing via unbundle (FFmpeg) ==========
#[cfg(feature = "media-probe")]
impl MediaAnalyzer {
    /// Analyze a media file by probing actual stream data via FFmpeg.
    ///
    /// Runs in a blocking thread since FFmpeg I/O is synchronous.
    /// Returns real resolution, codec, bitrate, audio channels, etc.
    pub async fn analyze(path: &Path) -> Result<MediaInfoModel> {
        let path_owned = path.to_path_buf();
        tokio::task::spawn_blocking(move || Self::probe_file(&path_owned))
            .await
            .context("Media probe task panicked")?
    }

    /// Synchronous probe using unbundle's MediaProbe.
    fn probe_file(path: &Path) -> Result<MediaInfoModel> {
        use unbundle::MediaProbe;

        let metadata = MediaProbe::probe(path)
            .map_err(|e| anyhow::anyhow!("FFmpeg probe failed for {}: {}", path.display(), e))?;

        // Video stream info
        let (resolution, video_codec, video_fps, video_bit_depth) =
            if let Some(ref video) = metadata.video {
                let resolution = format!("{}x{}", video.width, video.height);
                let codec = normalize_video_codec(&video.codec);
                let fps = if video.frames_per_second > 0.0 {
                    Some(video.frames_per_second)
                } else {
                    None
                };
                let bit_depth = video.bits_per_raw_sample.map(|b| b as i32);
                (Some(resolution), Some(codec), fps, bit_depth)
            } else {
                (None, None, None, None)
            };

        // HDR detection from color metadata
        let (video_dynamic_range, video_dynamic_range_type) = metadata
            .video
            .as_ref()
            .map(|v| detect_hdr_from_color_metadata(v))
            .unwrap_or((None, None));

        // Primary audio stream
        let (audio_codec, audio_channels, audio_bitrate) = if let Some(ref audio) = metadata.audio {
            let codec = normalize_audio_codec(&audio.codec);
            let channels = channels_to_layout(audio.channels);
            let bitrate = if audio.bit_rate > 0 {
                Some(audio.bit_rate as i64)
            } else {
                None
            };
            (Some(codec), Some(channels), bitrate)
        } else {
            (None, None, None)
        };

        // Audio stream count
        let audio_stream_count = metadata
            .audio_tracks
            .as_ref()
            .map(|tracks| tracks.len() as i32);

        // Subtitle track count as comma-separated languages (if available)
        let subtitles = metadata.subtitle_tracks.as_ref().map(|tracks| {
            if tracks.is_empty() {
                return String::new();
            }
            format!("{} subtitle track(s)", tracks.len())
        });

        // Duration formatting
        let run_time = {
            let secs = metadata.duration.as_secs();
            if secs > 0 {
                let hours = secs / 3600;
                let minutes = (secs % 3600) / 60;
                let seconds = secs % 60;
                Some(format!("{:02}:{:02}:{:02}", hours, minutes, seconds))
            } else {
                None
            }
        };

        Ok(MediaInfoModel {
            audio_bitrate,
            audio_channels,
            audio_codec,
            audio_languages: None,
            audio_stream_count,
            video_bit_depth,
            video_bitrate: None, // unbundle VideoMetadata doesn't expose video bitrate
            video_codec,
            video_fps,
            video_dynamic_range,
            video_dynamic_range_type,
            resolution,
            run_time,
            scan_type: None,
            subtitles,
        })
    }
}

// ========== Filename-based fallback (no FFmpeg) ==========
#[cfg(not(feature = "media-probe"))]
impl MediaAnalyzer {
    /// Fallback: analyze a media file using filename patterns only.
    ///
    /// Used when the `media-probe` feature is disabled (no FFmpeg available).
    pub async fn analyze(path: &Path) -> Result<MediaInfoModel> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        let resolution = detect_resolution_from_filename(&filename);
        let video_codec = detect_video_codec_from_filename(&filename);
        let audio_codec = detect_audio_codec_from_filename(&filename);
        let (video_dynamic_range, video_dynamic_range_type) = detect_hdr_from_filename(&filename);
        let video_bit_depth = detect_bit_depth_from_filename(&filename);

        Ok(MediaInfoModel {
            audio_bitrate: None,
            audio_channels: detect_audio_channels_from_filename(&filename),
            audio_codec,
            audio_languages: None,
            audio_stream_count: None,
            video_bit_depth,
            video_bitrate: None,
            video_codec,
            video_fps: None,
            video_dynamic_range,
            video_dynamic_range_type,
            resolution,
            run_time: None,
            scan_type: None,
            subtitles: None,
        })
    }
}

// ========== Shared methods (both feature paths) ==========
impl MediaAnalyzer {
    /// Analyze a file and return the result as a JSON string suitable for DB storage.
    /// Returns None on failure (non-fatal — media info is optional).
    pub async fn analyze_to_json(path: &Path) -> Option<String> {
        match Self::analyze(path).await {
            Ok(info) => serde_json::to_string(&info).ok(),
            Err(e) => {
                debug!("Media analysis failed for {}: {}", path.display(), e);
                None
            }
        }
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

// ========== BLAKE3 File Hashing ==========

/// Compute BLAKE3 hash of a file's contents.
///
/// Uses a 1MB buffer for efficient streaming. BLAKE3 achieves 3-5 GB/s
/// on modern hardware so even large video files are fast.
pub async fn compute_file_hash(path: &Path) -> Result<String> {
    let path_owned = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path_owned)
            .with_context(|| format!("Failed to open for hashing: {}", path_owned.display()))?;
        let mut reader = std::io::BufReader::with_capacity(1 << 20, file); // 1MB buffer
        let mut hasher = blake3::Hasher::new();
        std::io::copy(&mut reader, &mut hasher)?;
        Ok(hasher.finalize().to_hex().to_string())
    })
    .await
    .context("File hash task panicked")?
}

// ========== Quality Derivation from Media Info ==========

/// Derive quality tier from actual media stream data.
///
/// Resolution is deterministic from the video stream. Source (bluray/web/hdtv)
/// is inferred from the filename since the stream codec alone can't distinguish
/// a bluray encode from a TV capture using the same encoder.
pub fn derive_quality_from_media(info: &MediaInfoModel, filename: &str) -> serde_json::Value {
    let height = info
        .resolution
        .as_deref()
        .and_then(|r| r.split('x').nth(1))
        .and_then(|h| h.parse::<i32>().ok())
        .unwrap_or(0);

    let filename_lower = filename.to_lowercase();

    // Source must come from filename — stream data is encoding-agnostic
    let is_bluray = filename_lower.contains("bluray")
        || filename_lower.contains("bdrip")
        || filename_lower.contains("remux");
    let is_webdl = filename_lower.contains("web")
        || filename_lower.contains("amzn")
        || filename_lower.contains("nf.");

    let (id, name) = match height {
        h if h >= 2160 => {
            if is_bluray {
                (14, "Bluray-2160p")
            } else if is_webdl {
                (12, "WEBDL-2160p")
            } else {
                (11, "HDTV-2160p")
            }
        }
        h if h >= 1080 => {
            if is_bluray {
                (9, "Bluray-1080p")
            } else if is_webdl {
                (8, "WEBDL-1080p")
            } else {
                (7, "HDTV-1080p")
            }
        }
        h if h >= 720 => {
            if is_bluray {
                (6, "Bluray-720p")
            } else if is_webdl {
                (5, "WEBDL-720p")
            } else {
                (4, "HDTV-720p")
            }
        }
        h if h >= 480 => (2, "DVD"),
        _ => (1, "SDTV"),
    };

    serde_json::json!({
        "quality": {
            "id": id,
            "name": name,
            "source": "mediaInfo",
            "resolution": height
        },
        "revision": {
            "version": 1,
            "real": 0,
            "isRepack": false
        }
    })
}

// ========== Codec Normalization Helpers ==========

/// Normalize FFmpeg video codec names to user-friendly names
fn normalize_video_codec(codec: &str) -> String {
    match codec {
        "h264" | "H264" => "x264".to_string(),
        "hevc" | "h265" | "H265" => "x265".to_string(),
        "av1" | "AV1" => "AV1".to_string(),
        "mpeg2video" | "mpeg2" => "MPEG2".to_string(),
        "mpeg4" => "XviD".to_string(),
        "vp9" | "VP9" => "VP9".to_string(),
        "vp8" | "VP8" => "VP8".to_string(),
        "vc1" | "wmv3" => "VC-1".to_string(),
        other => other.to_uppercase(),
    }
}

/// Normalize FFmpeg audio codec names to user-friendly names
fn normalize_audio_codec(codec: &str) -> String {
    match codec {
        "aac" | "AAC" => "AAC".to_string(),
        "ac3" | "AC3" => "AC3".to_string(),
        "eac3" | "EAC3" => "EAC3".to_string(),
        "dts" | "DTS" => "DTS".to_string(),
        "truehd" => "TrueHD".to_string(),
        "flac" | "FLAC" => "FLAC".to_string(),
        "mp3" | "mp3float" => "MP3".to_string(),
        "vorbis" => "Vorbis".to_string(),
        "opus" => "Opus".to_string(),
        "pcm_s16le" | "pcm_s24le" | "pcm_s32le" => "PCM".to_string(),
        other => other.to_uppercase(),
    }
}

/// Convert raw channel count to standard layout notation (5.1, 7.1, 2.0, etc.)
fn channels_to_layout(channels: u16) -> f64 {
    match channels {
        1 => 1.0,
        2 => 2.0,
        6 => 5.1,
        8 => 7.1,
        n => n as f64,
    }
}

/// Detect HDR from FFmpeg color metadata fields
#[cfg(feature = "media-probe")]
fn detect_hdr_from_color_metadata(
    video: &unbundle::metadata::VideoMetadata,
) -> (Option<String>, Option<String>) {
    let transfer = video.color_transfer.as_deref().unwrap_or("").to_lowercase();
    let primaries = video
        .color_primaries
        .as_deref()
        .unwrap_or("")
        .to_lowercase();

    // SMPTE ST 2084 (PQ) = HDR10 / Dolby Vision
    if transfer.contains("smpte2084") || transfer.contains("st2084") {
        // BT.2020 primaries + PQ transfer = HDR10 (or DV, but can't distinguish without RPU)
        if primaries.contains("bt2020") {
            return (Some("HDR".to_string()), Some("HDR10".to_string()));
        }
        return (Some("HDR".to_string()), Some("HDR10".to_string()));
    }

    // ARIB STD-B67 = HLG
    if transfer.contains("arib-std-b67") || transfer.contains("hlg") {
        return (Some("HDR".to_string()), Some("HLG".to_string()));
    }

    // 10-bit with BT.2020 but standard transfer = likely HDR
    if primaries.contains("bt2020") {
        if let Some(bits) = video.bits_per_raw_sample {
            if bits >= 10 {
                return (Some("HDR".to_string()), Some("HDR10".to_string()));
            }
        }
    }

    (None, None)
}

// ========== Filename-based detection (fallback helpers) ==========

fn detect_resolution_from_filename(filename: &str) -> Option<String> {
    if filename.contains("2160p") || filename.contains("4k") || filename.contains("uhd") {
        Some("3840x2160".to_string())
    } else if filename.contains("1080p") || filename.contains("1080i") {
        Some("1920x1080".to_string())
    } else if filename.contains("720p") {
        Some("1280x720".to_string())
    } else if filename.contains("576p") || filename.contains("576i") {
        Some("720x576".to_string())
    } else if filename.contains("480p") || filename.contains("480i") {
        Some("720x480".to_string())
    } else {
        None
    }
}

fn detect_video_codec_from_filename(filename: &str) -> Option<String> {
    if filename.contains("x265") || filename.contains("h265") || filename.contains("hevc") {
        Some("x265".to_string())
    } else if filename.contains("x264") || filename.contains("h264") || filename.contains("avc") {
        Some("x264".to_string())
    } else if filename.contains("av1") {
        Some("AV1".to_string())
    } else if filename.contains("xvid") {
        Some("XviD".to_string())
    } else if filename.contains("divx") {
        Some("DivX".to_string())
    } else if filename.contains("mpeg2") {
        Some("MPEG2".to_string())
    } else {
        None
    }
}

fn detect_audio_codec_from_filename(filename: &str) -> Option<String> {
    if filename.contains("truehd") || filename.contains("true.hd") {
        Some("TrueHD".to_string())
    } else if filename.contains("atmos") {
        Some("TrueHD Atmos".to_string())
    } else if filename.contains("dts-hd.ma") || filename.contains("dts-hd ma") {
        Some("DTS-HD MA".to_string())
    } else if filename.contains("dts-hd") {
        Some("DTS-HD".to_string())
    } else if filename.contains("dts") {
        Some("DTS".to_string())
    } else if filename.contains("flac") {
        Some("FLAC".to_string())
    } else if filename.contains("eac3") || filename.contains("ddp") || filename.contains("dd+") {
        Some("EAC3".to_string())
    } else if filename.contains("ac3")
        || filename.contains("dd5")
        || filename.contains("dolby.digital")
    {
        Some("AC3".to_string())
    } else if filename.contains("aac") {
        Some("AAC".to_string())
    } else if filename.contains("mp3") {
        Some("MP3".to_string())
    } else {
        None
    }
}

fn detect_audio_channels_from_filename(filename: &str) -> Option<f64> {
    if filename.contains("7.1") {
        Some(7.1)
    } else if filename.contains("5.1") {
        Some(5.1)
    } else if filename.contains("2.0") || filename.contains("stereo") {
        Some(2.0)
    } else if filename.contains("mono") || filename.contains("1.0") {
        Some(1.0)
    } else {
        None
    }
}

fn detect_hdr_from_filename(filename: &str) -> (Option<String>, Option<String>) {
    if filename.contains("dolby.vision")
        || filename.contains("dovi")
        || filename.contains("dv") && filename.contains("hdr")
    {
        (Some("HDR".to_string()), Some("Dolby Vision".to_string()))
    } else if filename.contains("hdr10+") || filename.contains("hdr10plus") {
        (Some("HDR".to_string()), Some("HDR10Plus".to_string()))
    } else if filename.contains("hdr10") || filename.contains("hdr") {
        (Some("HDR".to_string()), Some("HDR10".to_string()))
    } else if filename.contains("hlg") {
        (Some("HDR".to_string()), Some("HLG".to_string()))
    } else {
        (None, None)
    }
}

fn detect_bit_depth_from_filename(filename: &str) -> Option<i32> {
    if filename.contains("10bit") || filename.contains("10-bit") || filename.contains("hi10p") {
        Some(10)
    } else if filename.contains("8bit") || filename.contains("8-bit") {
        Some(8)
    } else {
        None
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
            std::fs::create_dir_all(parent).context(format!(
                "Failed to create parent directory: {}",
                parent.display()
            ))?;
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
fn copy_directory_recursive(
    source: &Path,
    destination: &Path,
    result: &mut MoveResult,
) -> Result<()> {
    std::fs::create_dir_all(destination).context(format!(
        "Failed to create directory: {}",
        destination.display()
    ))?;
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
                    debug!(
                        "Copied file: {} -> {}",
                        entry_path.display(),
                        dest_path.display()
                    );
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
        info!(
            "Series folder does not exist, nothing to delete: {}",
            path.display()
        );
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
