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

use regex::Regex;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::core::messaging::ScannedFile;
use crate::core::parser::normalize_title;

pub use consumer::{
    create_movie_scan_request, create_music_scan_request, create_podcast_scan_request,
    create_scan_request, DownloadImportInfo, ScanResultConsumer,
};
pub use jobs::JobTrackerService;
pub use registry::WorkerRegistryService;

/// Video file extensions supported by pir9
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "wmv", "m4v", "ts", "webm", "mov", "flv", "mpg", "mpeg", "vob", "ogm",
    "divx", "m2ts", "mts",
];

/// Scan a directory recursively for video files
///
/// Returns a list of all video files found in the directory tree.
pub fn scan_directory_for_videos(path: &Path) -> Vec<PathBuf> {
    // Handle single-file paths (e.g., qBittorrent content_path for single-file torrents)
    if path.is_file() {
        return if is_video_file(path) {
            vec![path.to_path_buf()]
        } else {
            vec![]
        };
    }

    let mut files = Vec::new();
    scan_directory_recursive(path, &mut files);
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
/// - Part 1, Pt 2
/// - Ep01, Episode 3
/// - Bare number: 01, 01 - Title (last resort fallback)
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
        if let Some(season) = season_match
            .get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())
        {
            // Find all episode numbers after the season marker
            // Match pattern like S01E01E02E03 or S01E01-E02-E03
            if let Ok(re) = Regex::new(r"[Ss]\d{1,2}([Ee]\d{1,2})+") {
                if let Some(full_match) = re.find(filename) {
                    let episode_part = full_match.as_str();
                    // Extract all episode numbers from the match
                    if let Ok(ep_re) = Regex::new(r"[Ee](\d{1,2})") {
                        for cap in ep_re.captures_iter(episode_part) {
                            if let Some(ep_num) =
                                cap.get(1).and_then(|m| m.as_str().parse::<i32>().ok())
                            {
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

    // Try "part N" / "part.N" / "pt N" — common for miniseries, maps to S01E0N
    if episodes.is_empty() {
        if let Some(caps) = Regex::new(r"(?i)[\.\s_-](?:part|pt)[\.\s_-]?(\d{1,2})")
            .ok()
            .and_then(|re| re.captures(filename))
        {
            if let Some(ep) = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                episodes.push((1, ep));
            }
        }
    }

    // Try bare "Ep01" / "EP02" / "Episode 3" — assume Season 1
    if episodes.is_empty() {
        if let Some(caps) =
            Regex::new(r"(?i)(?:^|[\.\s_-])(?:Ep|Episode)[\.\s_-]?(\d{1,3})(?:[\.\s_-]|$)")
                .ok()
                .and_then(|re| re.captures(filename))
        {
            if let Some(ep) = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                episodes.push((1, ep));
            }
        }
    }

    // Try S01M01 / S07M02 format (specials/movies) → maps to season 0
    if episodes.is_empty() {
        if let Some(caps) = Regex::new(r"[Ss]\d{1,2}[Mm](\d{1,2})")
            .ok()
            .and_then(|re| re.captures(filename))
        {
            if let Some(ep) = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                episodes.push((0, ep));
            }
        }
    }

    // Last resort: bare number at start of filename — "01.mkv", "01 - Title.mkv",
    // "01.Title.mkv". Common for files already in a series/season folder.
    // Season defaults to 1; caller should override from folder context.
    if episodes.is_empty() {
        if let Some(caps) = Regex::new(r"^(\d{1,3})(?:[\.\s_-]|$)")
            .ok()
            .and_then(|re| re.captures(filename))
        {
            if let Some(ep) = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                // Only accept if the number is a plausible episode (1-999)
                if ep >= 1 {
                    episodes.push((1, ep));
                }
            }
        }
    }

    episodes
}

/// Extract season number from a folder name like "Season 1", "Season 01", "S01", "S1".
///
/// Returns `None` if the folder name doesn't contain a recognizable season pattern.
fn season_from_folder(folder_name: &str) -> Option<i32> {
    Regex::new(r"(?i)^(?:Season[\.\s_-]?|S)(\d{1,2})$")
        .ok()
        .and_then(|re| re.captures(folder_name))
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
}

/// Check if a filename was parsed using the bare number fallback
/// (i.e., none of the explicit patterns like S01E01, 1x01, Part, Ep matched).
fn is_bare_number_match(filename: &str) -> bool {
    // If any explicit pattern matches, this is NOT a bare number match
    let has_sxxexx = Regex::new(r"[Ss](\d{1,2})([Ee]\d{1,2})+")
        .ok()
        .is_some_and(|re| re.is_match(filename));
    let has_alt = Regex::new(r"(\d{1,2})x(\d{1,2})")
        .ok()
        .is_some_and(|re| re.is_match(filename));
    let has_part = Regex::new(r"(?i)[\.\s_-](?:part|pt)[\.\s_-]?(\d{1,2})")
        .ok()
        .is_some_and(|re| re.is_match(filename));
    let has_ep = Regex::new(r"(?i)(?:^|[\.\s_-])(?:Ep|Episode)[\.\s_-]?(\d{1,3})(?:[\.\s_-]|$)")
        .ok()
        .is_some_and(|re| re.is_match(filename));
    let has_special = Regex::new(r"[Ss]\d{1,2}[Mm](\d{1,2})")
        .ok()
        .is_some_and(|re| re.is_match(filename));

    if has_sxxexx || has_alt || has_part || has_ep || has_special {
        return false;
    }

    // Check if bare number pattern matches
    Regex::new(r"^(\d{1,3})(?:[\.\s_-]|$)")
        .ok()
        .is_some_and(|re| re.is_match(filename))
}

/// Quality/codec/source tokens to strip from filenames when extracting title words.
const QUALITY_TOKENS: &[&str] = &[
    "1080p", "720p", "480p", "2160p", "4k", "hdtv", "web", "webdl", "webrip", "bluray", "bdrip",
    "dvdrip", "dvd", "xvid", "x264", "x265", "h264", "h265", "hevc", "aac", "ac3", "dd51", "dts",
    "flac", "mp3", "amzn", "dsnp", "nf", "hulu", "atvp", "hmax", "proper", "repack", "rerip",
    "internal", "mkv", "mp4", "avi",
];

/// Match a filename against season 0 (special) episode titles.
///
/// Extracts the "title portion" of the filename (after removing series name,
/// quality tags, and release group) and compares it against each special's title
/// using word overlap. Returns the best match if score is above threshold.
pub fn match_special_by_title(
    filename: &str,
    series_title: &str,
    specials: &[(i32, &str)], // (episode_number, title)
) -> Option<(i32, i32)> {
    // Normalize filename: dots/dashes/underscores → spaces, lowercase, strip articles
    let normalized_filename = normalize_title(filename);
    let normalized_series = normalize_title(series_title);

    // Remove the series title from the filename text
    let remainder = normalized_filename
        .replace(&normalized_series, "")
        .trim()
        .to_string();

    // Extract content words, removing quality/codec tokens and short noise
    let filename_words: Vec<&str> = remainder
        .split_whitespace()
        .filter(|w| w.len() >= 2)
        .filter(|w| !QUALITY_TOKENS.contains(&w.to_lowercase().as_str()))
        .filter(|w| w.parse::<i32>().is_err()) // remove bare numbers (season/episode digits)
        .collect();

    if filename_words.is_empty() {
        return None;
    }

    let mut best_match: Option<(i32, f64)> = None;

    for &(ep_num, ep_title) in specials {
        let normalized_ep_title = normalize_title(ep_title);
        let title_words: Vec<&str> = normalized_ep_title.split_whitespace().collect();

        // Skip single-word titles (too ambiguous — e.g., "Pilot")
        if title_words.len() < 2 {
            continue;
        }

        // Count how many of the special's title words appear in the filename
        let matched = title_words
            .iter()
            .filter(|tw| filename_words.iter().any(|fw| fw == *tw))
            .count();

        let ratio = matched as f64 / title_words.len() as f64;

        // Require >= 60% overlap
        if ratio >= 0.6 {
            if let Some((_, best_ratio)) = best_match {
                if ratio > best_ratio {
                    best_match = Some((ep_num, ratio));
                }
                // On tie, keep first (lowest episode number — already iterated in order)
            } else {
                best_match = Some((ep_num, ratio));
            }
        }
    }

    best_match.map(|(ep_num, _)| (0, ep_num))
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
        if !group.is_empty() && !group.chars().all(|c| c.is_numeric()) && group.len() <= 20 {
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

        let mut parsed_episodes = parse_episodes_from_filename(&filename);
        if parsed_episodes.is_empty() {
            debug!("Could not parse episode info from: {}", filename);
            continue;
        }

        // If we matched via bare number (season defaults to 1), try to infer
        // the real season from the parent folder name (e.g., "Season 2", "S02").
        // Only override season for bare number matches — explicit patterns like
        // S01E01, 1x01, Ep01 already encode the correct season.
        if is_bare_number_match(&filename) {
            if let Some(parent) = file_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
            {
                if let Some(folder_season) = season_from_folder(parent) {
                    for ep in &mut parsed_episodes {
                        ep.0 = folder_season;
                    }
                }
            }
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
                episode_numbers
                    .iter()
                    .map(|e| format!("{:02}", e))
                    .collect::<Vec<_>>()
                    .join("E")
            );
        }

        results.push(ScannedFile {
            path: file_path,
            size: file_size,
            season_number,
            episode_numbers,
            release_group,
            filename,
            media_info: None,
            quality: None,
            file_hash: None,
        });
    }

    results
}

/// Scan a movie folder for the largest video file (max depth 2).
///
/// Returns at most one ScannedFile — the largest video file found.
/// This mirrors `scan_movie_folder()` in `api/v5/movies.rs` but returns
/// a transport-friendly `ScannedFile` instead of a `MovieFileDbModel`.
pub fn scan_movie_directory(dir: &Path) -> Option<ScannedFile> {
    if !dir.exists() {
        return None;
    }

    // Handle single-file case: path points directly to a video file
    if dir.is_file() {
        if !is_video_file(dir) {
            return None;
        }
        let size = std::fs::metadata(dir).map(|m| m.len() as i64).unwrap_or(0);
        let filename = dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let release_group = extract_release_group(&filename);
        return Some(ScannedFile {
            path: dir.to_path_buf(),
            size,
            season_number: None,
            episode_numbers: vec![],
            release_group,
            filename,
            media_info: None,
            quality: None,
            file_hash: None,
        });
    }

    let mut best: Option<(PathBuf, i64)> = None;

    fn walk_movie_dir(dir: &Path, best: &mut Option<(PathBuf, i64)>, depth: usize) {
        if depth > 2 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_movie_dir(&path, best, depth + 1);
                } else if is_video_file(&path) {
                    let size = std::fs::metadata(&path).map(|m| m.len() as i64).unwrap_or(0);
                    if best.as_ref().is_none_or(|(_, s)| size > *s) {
                        *best = Some((path, size));
                    }
                }
            }
        }
    }

    walk_movie_dir(dir, &mut best, 0);

    best.map(|(file_path, size)| {
        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let release_group = extract_release_group(&filename);
        ScannedFile {
            path: file_path,
            size,
            season_number: None,
            episode_numbers: vec![],
            release_group,
            filename,
            media_info: None,
            quality: None,
            file_hash: None,
        }
    })
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
                    let mut parsed_episodes = parse_episodes_from_filename(filename);
                    let file_size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);

                    // Apply folder-based season override for bare number matches
                    if is_bare_number_match(filename) {
                        if let Some(parent) = path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                        {
                            if let Some(folder_season) = season_from_folder(parent) {
                                for ep in &mut parsed_episodes {
                                    ep.0 = folder_season;
                                }
                            }
                        }
                    }

                    let scanned = ScannedFile {
                        path: path.clone(),
                        size: file_size,
                        season_number: parsed_episodes.first().map(|(s, _)| *s),
                        episode_numbers: parsed_episodes.iter().map(|(_, e)| *e).collect(),
                        release_group: extract_release_group(filename),
                        filename: filename.to_string(),
                        media_info: None,
                        quality: None,
                        file_hash: None,
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
            (
                "Show - S02E10E11E12 - Marathon.mkv",
                vec![(2, 10), (2, 11), (2, 12)],
            ),
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
            ("Show.S01E01.mkv", None),          // No dash-group pattern
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
    fn test_parse_part_format() {
        let cases = vec![
            ("The.Diamond.Hunters.2001.part.1.mkv", vec![(1, 1)]),
            ("The.Diamond.Hunters.2001.part.2.mkv", vec![(1, 2)]),
            ("Miniseries.Part.3.720p.mkv", vec![(1, 3)]),
            ("Show.pt.1.mkv", vec![(1, 1)]),
            ("Show - Part 2.mkv", vec![(1, 2)]),
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
    fn test_parse_ep_format() {
        let cases = vec![
            ("Ep01.mp4", vec![(1, 1)]),
            ("EP02.mkv", vec![(1, 2)]),
            ("Episode 3.mp4", vec![(1, 3)]),
            ("Show.Episode.05.mkv", vec![(1, 5)]),
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
    fn test_parse_special_movie_format() {
        let cases = vec![
            (
                "MacGyver.S07M01.Lost.Treasure.of.Atlantis.DVDRip.XviD.mkv",
                vec![(0, 1)],
            ),
            (
                "MacGyver.S07M02.Trail.to.Doomsday.DVDRip.XviD.mkv",
                vec![(0, 2)],
            ),
            ("Show.S02M03.Special.Episode.mkv", vec![(0, 3)]),
            ("show.s01m01.holiday.special.mkv", vec![(0, 1)]),
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
    fn test_match_special_by_title_basic() {
        let specials = vec![(1, "Lost Treasure of Atlantis"), (2, "Trail to Doomsday")];

        // Full title match
        let result = match_special_by_title(
            "MacGyver.Lost.Treasure.of.Atlantis.DVDRip.XviD.mkv",
            "MacGyver",
            &specials,
        );
        assert_eq!(result, Some((0, 1)));

        // Second special
        let result = match_special_by_title(
            "MacGyver.Trail.to.Doomsday.DVDRip.XviD.mkv",
            "MacGyver",
            &specials,
        );
        assert_eq!(result, Some((0, 2)));
    }

    #[test]
    fn test_match_special_by_title_no_match() {
        let specials = vec![(1, "Lost Treasure of Atlantis"), (2, "Trail to Doomsday")];

        // Completely unrelated filename
        let result = match_special_by_title("MacGyver.S03E05.720p.HDTV.mkv", "MacGyver", &specials);
        assert_eq!(result, None);
    }

    #[test]
    fn test_match_special_by_title_single_word_skipped() {
        let specials = vec![
            (1, "Pilot"), // Single word — should be skipped
        ];

        let result = match_special_by_title("Show.Pilot.720p.mkv", "Show", &specials);
        assert_eq!(result, None);
    }

    #[test]
    fn test_match_special_by_title_best_match_wins() {
        let specials = vec![
            (1, "Lost Treasure of Atlantis"),
            (2, "Lost Treasure of the Deep"),
        ];

        // "Atlantis" distinguishes special 1 from special 2
        let result = match_special_by_title(
            "MacGyver.Lost.Treasure.of.Atlantis.DVDRip.mkv",
            "MacGyver",
            &specials,
        );
        assert_eq!(result, Some((0, 1)));
    }

    #[test]
    fn test_parse_bare_number() {
        let cases = vec![
            ("01.mkv", vec![(1, 1)]),
            ("01 - Episode Title.mkv", vec![(1, 1)]),
            ("01 Episode Title.mkv", vec![(1, 1)]),
            ("01.Episode.Title.mkv", vec![(1, 1)]),
            ("10_Title.mkv", vec![(1, 10)]),
            ("100.mkv", vec![(1, 100)]),
            ("1.mkv", vec![(1, 1)]),
            ("05 - The One Where.mkv", vec![(1, 5)]),
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
    fn test_bare_number_does_not_override_explicit() {
        // S01E01 should NOT be treated as bare number
        assert!(!is_bare_number_match("S01E01.mkv"));
        // 1x01 should NOT be treated as bare number
        assert!(!is_bare_number_match("1x01.mkv"));
        // Ep01 should NOT be treated as bare number
        assert!(!is_bare_number_match("Ep01.mkv"));
        // Part 1 should NOT be treated as bare number
        assert!(!is_bare_number_match("Show.Part.1.mkv"));
        // Bare number IS a bare number match
        assert!(is_bare_number_match("01.mkv"));
        assert!(is_bare_number_match("01 - Title.mkv"));
        assert!(is_bare_number_match("05.Episode.Title.mkv"));
    }

    #[test]
    fn test_season_from_folder() {
        assert_eq!(season_from_folder("Season 1"), Some(1));
        assert_eq!(season_from_folder("Season 02"), Some(2));
        assert_eq!(season_from_folder("Season.03"), Some(3));
        assert_eq!(season_from_folder("S01"), Some(1));
        assert_eq!(season_from_folder("S1"), Some(1));
        assert_eq!(season_from_folder("s04"), Some(4));
        assert_eq!(season_from_folder("season 10"), Some(10));
        // Not a season folder
        assert_eq!(season_from_folder("Extras"), None);
        assert_eq!(season_from_folder("Specials"), None);
        assert_eq!(season_from_folder("The Flash"), None);
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
