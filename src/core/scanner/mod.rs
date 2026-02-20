#![allow(dead_code, unused_imports)]
//! File scanning module for pir9
//!
//! This module provides file system scanning capabilities that can be used
//! by both the main server (local scanning) and distributed workers.
//!
//! The scanner is designed to be stateless - it only reads files and returns
//! results. All database operations happen in the caller.

pub mod consumer;
pub mod jobs;
pub mod registry;

use std::path::{Path, PathBuf};
use regex::Regex;
use tracing::{debug, warn};

use crate::core::messaging::ScannedFile;

pub use consumer::{ScanResultConsumer, create_scan_request};
pub use jobs::JobTrackerService;
pub use registry::WorkerRegistryService;

/// Video file extensions supported by pir9
pub const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "wmv", "m4v", "ts", "webm", "mov"];

/// Scan a directory recursively for video files
///
/// Returns a list of all video files found in the directory tree.
pub fn scan_directory_for_videos(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    scan_directory_recursive(dir, &mut files);
    files
}

fn scan_directory_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_directory_recursive(&path, files);
            } else if is_video_file(&path) {
                files.push(path);
            }
        }
    }
}

/// Check if a path is a video file based on extension
pub fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Parse episode information from a filename
///
/// Supports multiple formats:
/// - S01E01, S01E01E02E03 (multi-episode)
/// - 1x01
///
/// Returns a vector of (season, episode) tuples.
pub fn parse_episodes_from_filename(filename: &str) -> Vec<(i32, i32)> {
    let mut episodes = Vec::new();

    // Try S01E01E02E03 format (multi-episode)
    // First, find the season number
    if let Some(season_match) = Regex::new(r"[Ss](\d{1,2})")
        .ok()
        .and_then(|re| re.captures(filename))
    {
        if let Some(season) = season_match.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
            // Find all episode numbers after the season marker
            // Match pattern like S01E01E02E03 or S01E01-E02-E03
            if let Ok(re) = Regex::new(r"[Ss]\d{1,2}([Ee]\d{1,2})+") {
                if let Some(full_match) = re.find(filename) {
                    let episode_part = full_match.as_str();
                    // Extract all episode numbers from the match
                    if let Ok(ep_re) = Regex::new(r"[Ee](\d{1,2})") {
                        for cap in ep_re.captures_iter(episode_part) {
                            if let Some(ep_num) = cap.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                                episodes.push((season, ep_num));
                            }
                        }
                    }
                }
            }
        }
    }

    // If no episodes found, try 1x01 format (single episode only)
    if episodes.is_empty() {
        if let Some(caps) = Regex::new(r"(\d{1,2})x(\d{1,2})")
            .ok()
            .and_then(|re| re.captures(filename))
        {
            if let (Some(season), Some(episode)) = (
                caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()),
                caps.get(2).and_then(|m| m.as_str().parse::<i32>().ok()),
            ) {
                episodes.push((season, episode));
            }
        }
    }

    episodes
}

/// Extract release group from filename
///
/// E.g., "Show.S01E01.720p.HDTV.x264-GROUP" -> "GROUP"
pub fn extract_release_group(filename: &str) -> Option<String> {
    // Common pattern: last part after a dash, before the extension
    let name_without_ext = filename
        .rsplit('.')
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(".");

    // Find the last dash that precedes the release group
    if let Some(dash_pos) = name_without_ext.rfind('-') {
        let group = &name_without_ext[dash_pos + 1..];
        // Filter out common false positives
        if !group.is_empty()
            && !group.chars().all(|c| c.is_numeric())
            && group.len() <= 20
        {
            return Some(group.to_string());
        }
    }
    None
}

/// Scan a series directory and return structured results
///
/// This is the main entry point for scanning a series folder.
/// Returns a vector of ScannedFile with parsed episode information.
pub fn scan_series_directory(series_path: &Path) -> Vec<ScannedFile> {
    let video_files = scan_directory_for_videos(series_path);
    let mut results = Vec::with_capacity(video_files.len());

    for file_path in video_files {
        let filename = match file_path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let parsed_episodes = parse_episodes_from_filename(&filename);
        if parsed_episodes.is_empty() {
            debug!("Could not parse episode info from: {}", filename);
            continue;
        }

        let file_size = std::fs::metadata(&file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let release_group = extract_release_group(&filename);

        // Use first episode's season as the primary season
        let season_number = parsed_episodes.first().map(|(s, _)| *s);
        let episode_numbers: Vec<i32> = parsed_episodes.iter().map(|(_, e)| *e).collect();

        if episode_numbers.len() > 1 {
            debug!(
                "Multi-episode file detected: {} -> S{:02}E{}",
                filename,
                season_number.unwrap_or(0),
                episode_numbers.iter().map(|e| format!("{:02}", e)).collect::<Vec<_>>().join("E")
            );
        }

        results.push(ScannedFile {
            path: file_path,
            size: file_size,
            season_number,
            episode_numbers,
            release_group,
            filename,
        });
    }

    results
}

/// Scan multiple paths and aggregate results
///
/// Used by workers that handle multiple root paths.
pub fn scan_paths(paths: &[PathBuf]) -> Vec<(PathBuf, Vec<ScannedFile>)> {
    let mut results = Vec::new();

    for path in paths {
        if !path.exists() {
            warn!("Scan path does not exist: {:?}", path);
            continue;
        }

        if path.is_file() {
            // Single file - scan it directly
            if is_video_file(path) {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    let parsed_episodes = parse_episodes_from_filename(filename);
                    let file_size = std::fs::metadata(path)
                        .map(|m| m.len() as i64)
                        .unwrap_or(0);

                    let scanned = ScannedFile {
                        path: path.clone(),
                        size: file_size,
                        season_number: parsed_episodes.first().map(|(s, _)| *s),
                        episode_numbers: parsed_episodes.iter().map(|(_, e)| *e).collect(),
                        release_group: extract_release_group(filename),
                        filename: filename.to_string(),
                    };
                    results.push((path.clone(), vec![scanned]));
                }
            }
        } else {
            // Directory - scan recursively
            let files = scan_series_directory(path);
            if !files.is_empty() {
                results.push((path.clone(), files));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_episode() {
        let cases = vec![
            ("Show.S01E01.720p.HDTV.x264-GROUP.mkv", vec![(1, 1)]),
            ("show.s02e05.title.mkv", vec![(2, 5)]),
            ("Show - S10E20 - Title.mkv", vec![(10, 20)]),
            ("1x01 - Pilot.mkv", vec![(1, 1)]),
            ("12x05.mkv", vec![(12, 5)]),
        ];

        for (filename, expected) in cases {
            assert_eq!(
                parse_episodes_from_filename(filename),
                expected,
                "Failed for: {}",
                filename
            );
        }
    }

    #[test]
    fn test_parse_multi_episode() {
        let cases = vec![
            ("Show.S01E01E02.mkv", vec![(1, 1), (1, 2)]),
            ("Show.S01E01E02E03.720p.mkv", vec![(1, 1), (1, 2), (1, 3)]),
            ("Show - S02E10E11E12 - Marathon.mkv", vec![(2, 10), (2, 11), (2, 12)]),
        ];

        for (filename, expected) in cases {
            assert_eq!(
                parse_episodes_from_filename(filename),
                expected,
                "Failed for: {}",
                filename
            );
        }
    }

    #[test]
    fn test_extract_release_group() {
        let cases = vec![
            ("Show.S01E01.720p.HDTV.x264-GROUP.mkv", Some("GROUP")),
            ("Show.S01E01-DIMENSION.mkv", Some("DIMENSION")),
            ("Show.S01E01.mkv", None), // No dash-group pattern
            ("Show.S01E01.720p-123.mkv", None), // Numeric only = false positive
        ];

        for (filename, expected) in cases {
            assert_eq!(
                extract_release_group(filename),
                expected.map(String::from),
                "Failed for: {}",
                filename
            );
        }
    }

    #[test]
    fn test_is_video_file() {
        assert!(is_video_file(Path::new("test.mkv")));
        assert!(is_video_file(Path::new("test.mp4")));
        assert!(is_video_file(Path::new("test.MKV"))); // Case insensitive
        assert!(!is_video_file(Path::new("test.txt")));
        assert!(!is_video_file(Path::new("test.srt")));
        assert!(!is_video_file(Path::new("test"))); // No extension
    }
}
