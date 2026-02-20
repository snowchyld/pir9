//! Parser module
//! Parsing release titles to extract episode information

use regex::Regex;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::core::profiles::qualities::{Quality, QualityModel, Revision};
use crate::core::profiles::languages::Language;

// ============================================================================
// Regex patterns for parsing
// ============================================================================

// Standard season/episode patterns: S01E02, S01E02E03, S1E2
static SEASON_EPISODE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)[.\s_-]S(\d{1,2})[\.\s_-]?E(\d{1,3})(?:[\.\s_-]?E(\d{1,3}))?(?:[\.\s_-]?E(\d{1,3}))?").unwrap()
});

// Alternative season/episode: 1x02, 1x02-03
static ALT_SEASON_EPISODE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:^|[.\s_-])(\d{1,2})x(\d{1,3})(?:[\.\s_-]?(\d{1,3}))?").unwrap()
});

// Full season: S01, Season 1
static FULL_SEASON_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:S|Season[\.\s_-]?)(\d{1,2})(?:[.\s_-](?:Complete|Full)|$|[.\s_-](?:720|1080|2160|HDTV|WEB|BluRay))").unwrap()
});

// Daily show format: Show.2024.01.15 or Show.2024-01-15
static DAILY_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)[\.\s_-](\d{4})[\.\s_-](\d{2})[\.\s_-](\d{2})[\.\s_-]").unwrap()
});

// Absolute episode for anime: Show - 123, Show.123.720p
static ABSOLUTE_EPISODE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:^|[\.\s_-])(?:E|Ep|Episode)?[\.\s_-]?(\d{2,4})(?:[vV]\d)?[\.\s_-](?:720|1080|2160|HDTV|WEB|BluRay|x264|x265|HEVC|AAC|\[)").unwrap()
});

// Year in title: Show (2020) or Show.2020
static YEAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[\.\s_\(\[-]*((?:19|20)\d{2})[\.\s_\)\]-]").unwrap()
});

// Release group: -GROUP or [GROUP]
static RELEASE_GROUP_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:-([A-Za-z0-9]+)$|\[([A-Za-z0-9]+)\]$)").unwrap()
});

// Hash pattern: [ABCD1234]
static HASH_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[([A-Fa-f0-9]{8})\]").unwrap()
});

// Quality patterns
static QUALITY_2160P_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:2160p|4K|UHD)").unwrap()
});

static QUALITY_1080P_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)1080[pi]").unwrap()
});

static QUALITY_720P_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)720p").unwrap()
});

static QUALITY_480P_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:480p|SD)").unwrap()
});

static SOURCE_BLURAY_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:BluRay|Blu-Ray|BDREMUX|BD[\.\s_-]?Rip)").unwrap()
});

static SOURCE_WEBDL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:WEB[\.\s_-]?DL|WEBDL|WEB[\.\s_-]?Rip|AMZN|DSNP|HMAX|NF|ATVP)").unwrap()
});

static SOURCE_HDTV_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:HDTV|PDTV|DSR)").unwrap()
});

static SOURCE_DVD_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:DVDRip|DVD[\.\s_-]?R|DVDSCR)").unwrap()
});

static REMUX_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:REMUX|BD[\.\s_-]?REMUX)").unwrap()
});

static PROPER_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)[\.\s_-]PROPER[\.\s_-]").unwrap()
});

static REPACK_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)[\.\s_-]REPACK[\.\s_-]").unwrap()
});

static SPECIAL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)[\.\s_-](?:Special|SPECIAL|Specials|OVA|OAD)[\.\s_-]").unwrap()
});

/// Parsed episode info from release title
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedEpisodeInfo {
    /// Cleaned series title
    pub series_title: String,
    /// Series title info with year
    pub series_title_info: SeriesTitleInfo,
    /// Detected quality
    pub quality: QualityModel,
    /// Season number
    pub season_number: Option<i32>,
    /// Episode numbers (can be multiple)
    pub episode_numbers: Vec<i32>,
    /// Absolute episode numbers (for anime)
    pub absolute_episode_numbers: Vec<i32>,
    /// Air date for daily shows
    pub air_date: Option<chrono::NaiveDate>,
    /// Detected languages
    pub languages: Vec<Language>,
    /// Release group name
    pub release_group: Option<String>,
    /// Release hash
    pub release_hash: Option<String>,
    /// Is this a daily show?
    pub is_daily: bool,
    /// Is this anime-style absolute numbering?
    pub is_absolute_numbering: bool,
    /// Could this be a special episode?
    pub is_possible_special_episode: bool,
    /// Is this explicitly a special?
    pub special: bool,
    /// Is this a full season pack?
    pub full_season: bool,
    /// Is this a partial season?
    pub is_partial_season: bool,
    /// Does this span multiple seasons?
    pub is_multi_season: bool,
    /// Is this a proper release?
    pub is_proper: bool,
    /// Is this a repack?
    pub is_repack: bool,
    /// Raw title before parsing
    pub raw_title: String,
}

/// Series title info with year extraction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeriesTitleInfo {
    /// Full title
    pub title: String,
    /// Title without the year
    pub title_without_year: String,
    /// Year from title (0 if not found)
    pub year: i32,
}

/// Parse a release title into structured information
pub fn parse_title(title: &str) -> Option<ParsedEpisodeInfo> {
    let mut info = ParsedEpisodeInfo {
        raw_title: title.to_string(),
        ..Default::default()
    };

    // Parse quality
    info.quality = parse_quality(title);
    info.is_proper = PROPER_REGEX.is_match(title);
    info.is_repack = REPACK_REGEX.is_match(title);

    if info.is_proper || info.is_repack {
        info.quality.revision = Revision {
            version: 2,
            real: 0,
            is_repack: info.is_repack,
        };
    }

    // Check for special
    info.special = SPECIAL_REGEX.is_match(title);
    info.is_possible_special_episode = info.special;

    // Parse release group and hash
    if let Some(caps) = RELEASE_GROUP_REGEX.captures(title) {
        info.release_group = caps.get(1).or(caps.get(2)).map(|m| m.as_str().to_string());
    }

    if let Some(caps) = HASH_REGEX.captures(title) {
        info.release_hash = Some(caps[1].to_string());
    }

    // Try standard S01E02 pattern first
    if let Some(caps) = SEASON_EPISODE_REGEX.captures(title) {
        info.season_number = caps.get(1).and_then(|m| m.as_str().parse().ok());

        // Collect all episode numbers
        if let Some(ep1) = caps.get(2).and_then(|m| m.as_str().parse().ok()) {
            info.episode_numbers.push(ep1);
        }
        if let Some(ep2) = caps.get(3).and_then(|m| m.as_str().parse::<i32>().ok()) {
            info.episode_numbers.push(ep2);
        }
        if let Some(ep3) = caps.get(4).and_then(|m| m.as_str().parse::<i32>().ok()) {
            info.episode_numbers.push(ep3);
        }

        let match_start = caps.get(0).map(|m| m.start()).unwrap_or(0);
        info.series_title = clean_title(&title[..match_start]);
    }
    // Try alternative 1x02 pattern
    else if let Some(caps) = ALT_SEASON_EPISODE_REGEX.captures(title) {
        info.season_number = caps.get(1).and_then(|m| m.as_str().parse().ok());

        if let Some(ep1) = caps.get(2).and_then(|m| m.as_str().parse().ok()) {
            info.episode_numbers.push(ep1);
        }
        if let Some(ep2) = caps.get(3).and_then(|m| m.as_str().parse::<i32>().ok()) {
            info.episode_numbers.push(ep2);
        }

        let match_start = caps.get(0).map(|m| m.start()).unwrap_or(0);
        info.series_title = clean_title(&title[..match_start]);
    }
    // Try daily show pattern
    else if let Some(caps) = DAILY_REGEX.captures(title) {
        info.is_daily = true;
        let year: i32 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        let month: u32 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        let day: u32 = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);

        if year > 0 && month > 0 && day > 0 {
            info.air_date = chrono::NaiveDate::from_ymd_opt(year, month, day);
        }

        let match_start = caps.get(0).map(|m| m.start()).unwrap_or(0);
        info.series_title = clean_title(&title[..match_start]);
    }
    // Try full season pattern
    else if let Some(caps) = FULL_SEASON_REGEX.captures(title) {
        info.season_number = caps.get(1).and_then(|m| m.as_str().parse().ok());
        info.full_season = true;

        let match_start = caps.get(0).map(|m| m.start()).unwrap_or(0);
        info.series_title = clean_title(&title[..match_start]);
    }
    // Try absolute episode pattern (anime)
    else if let Some(caps) = ABSOLUTE_EPISODE_REGEX.captures(title) {
        info.is_absolute_numbering = true;
        if let Some(ep) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
            info.absolute_episode_numbers.push(ep);
        }

        let match_start = caps.get(0).map(|m| m.start()).unwrap_or(0);
        info.series_title = clean_title(&title[..match_start]);
    }

    // Extract year from series title
    if !info.series_title.is_empty() {
        if let Some(caps) = YEAR_REGEX.captures(&info.series_title) {
            let year: i32 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            info.series_title_info.title = info.series_title.clone();
            info.series_title_info.year = year;

            // Remove year from series title for matching
            if let Some(year_match) = caps.get(0) {
                let clean = info.series_title[..year_match.start()].trim().to_string();
                if !clean.is_empty() {
                    info.series_title_info.title_without_year = clean.clone();
                    info.series_title = clean;
                }
            }
        } else {
            info.series_title_info.title = info.series_title.clone();
            info.series_title_info.title_without_year = info.series_title.clone();
        }
    }

    // Return None if we couldn't extract any useful info
    if info.series_title.is_empty() && info.episode_numbers.is_empty() && info.absolute_episode_numbers.is_empty() {
        return None;
    }

    Some(info)
}

/// Clean a title string (remove dots, underscores, normalize spaces)
fn clean_title(title: &str) -> String {
    let cleaned = title.replace(['.', '_', '-'], " ");

    // Collapse multiple spaces
    let mut result = String::new();
    let mut prev_space = false;
    for c in cleaned.chars() {
        if c == ' ' {
            if !prev_space && !result.is_empty() {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(c);
            prev_space = false;
        }
    }

    result.trim().to_string()
}

/// Parse quality from a release title
pub fn parse_quality(title: &str) -> QualityModel {
    let is_remux = REMUX_REGEX.is_match(title);
    let is_bluray = SOURCE_BLURAY_REGEX.is_match(title);
    let is_webdl = SOURCE_WEBDL_REGEX.is_match(title);
    let is_hdtv = SOURCE_HDTV_REGEX.is_match(title);
    let is_dvd = SOURCE_DVD_REGEX.is_match(title);

    let quality = if QUALITY_2160P_REGEX.is_match(title) {
        if is_remux && is_bluray {
            Quality::Bluray2160pRemux
        } else if is_bluray {
            Quality::Bluray2160p
        } else if is_webdl {
            Quality::WebDl2160p
        } else {
            Quality::Hdtv2160p
        }
    } else if QUALITY_1080P_REGEX.is_match(title) {
        if is_remux && is_bluray {
            Quality::Bluray1080pRemux
        } else if is_bluray {
            Quality::Bluray1080p
        } else if is_webdl {
            Quality::WebDl1080p
        } else {
            Quality::Hdtv1080p
        }
    } else if QUALITY_720P_REGEX.is_match(title) {
        if is_bluray {
            Quality::Bluray720p
        } else if is_webdl {
            Quality::WebDl720p
        } else {
            Quality::Hdtv720p
        }
    } else if QUALITY_480P_REGEX.is_match(title) || is_dvd {
        if is_webdl {
            Quality::WebDl480p
        } else if is_dvd {
            Quality::Dvd
        } else {
            Quality::SDTV
        }
    } else if is_bluray {
        Quality::Bluray1080p
    } else if is_webdl {
        Quality::WebDl1080p
    } else if is_hdtv {
        Quality::Hdtv720p
    } else {
        Quality::Unknown
    };

    QualityModel {
        quality,
        revision: Revision::default(),
    }
}

/// Normalize a series title for matching (lowercase, remove special chars)
pub fn normalize_title(title: &str) -> String {
    let cleaned = clean_title(title).to_lowercase();

    cleaned
        .replace(" the ", " ")
        .replace("the ", "")
        .replace(" a ", " ")
        .replace(" and ", " ")
        .replace(" & ", " ")
        .replace("'", "")
        .replace("\"", "")
        .replace(":", "")
        .replace(",", "")
        .trim()
        .to_string()
}

/// Match a parsed title against a series name
pub fn title_matches_series(info: &ParsedEpisodeInfo, series_title: &str) -> bool {
    if info.series_title.is_empty() {
        return false;
    }

    let normalized_parsed = normalize_title(&info.series_title);
    let normalized_series = normalize_title(series_title);

    // Exact match
    if normalized_parsed == normalized_series {
        return true;
    }

    // Check if one contains the other (for partial matches)
    if normalized_parsed.contains(&normalized_series) || normalized_series.contains(&normalized_parsed) {
        return true;
    }

    false
}

/// Parse series and episode info from file path
pub fn parse_path(path: &std::path::Path) -> Option<ParsedEpisodeInfo> {
    let file_name = path.file_stem()?.to_str()?;
    parse_title(file_name)
}

/// Sanitize a series title for searching
pub fn sanitize_series_title(title: &str) -> String {
    title.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_episode() {
        let parsed = parse_title("The.Mandalorian.S01E02.720p.WEB-DL.x264-GROUP").unwrap();
        assert_eq!(parsed.series_title, "The Mandalorian");
        assert_eq!(parsed.season_number, Some(1));
        assert_eq!(parsed.episode_numbers, vec![2]);
        assert_eq!(parsed.quality.quality, Quality::WebDl720p);
        assert_eq!(parsed.release_group, Some("GROUP".to_string()));
    }

    #[test]
    fn test_parse_multi_episode() {
        let parsed = parse_title("Show.S02E03E04E05.1080p.HDTV").unwrap();
        assert_eq!(parsed.season_number, Some(2));
        assert_eq!(parsed.episode_numbers, vec![3, 4, 5]);
    }

    #[test]
    fn test_parse_daily_show() {
        let parsed = parse_title("The.Daily.Show.2024.01.15.720p.WEB-DL").unwrap();
        assert!(parsed.is_daily);
        assert_eq!(parsed.air_date, chrono::NaiveDate::from_ymd_opt(2024, 1, 15));
        assert_eq!(parsed.series_title, "The Daily Show");
    }

    #[test]
    fn test_parse_full_season() {
        let parsed = parse_title("Breaking.Bad.S01.Complete.BluRay.1080p").unwrap();
        assert!(parsed.full_season);
        assert_eq!(parsed.season_number, Some(1));
    }

    #[test]
    fn test_quality_detection() {
        assert_eq!(parse_quality("Show.S01E01.2160p.BluRay.REMUX").quality, Quality::Bluray2160pRemux);
        assert_eq!(parse_quality("Show.S01E01.1080p.WEB-DL").quality, Quality::WebDl1080p);
        assert_eq!(parse_quality("Show.S01E01.720p.HDTV").quality, Quality::Hdtv720p);
    }

    #[test]
    fn test_proper_repack() {
        let parsed = parse_title("Show.S01E01.PROPER.720p.HDTV").unwrap();
        assert!(parsed.is_proper);
        assert_eq!(parsed.quality.revision.version, 2);

        let parsed = parse_title("Show.S01E01.REPACK.1080p.WEB-DL").unwrap();
        assert!(parsed.is_repack);
        assert!(parsed.quality.revision.is_repack);
    }
}
