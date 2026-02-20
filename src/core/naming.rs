//! Episode naming template engine
//!
//! Parses format strings like `{Series Title} - S{season:00}E{episode:00}`
//! and produces filenames from series/episode/quality context.

use crate::core::configuration::MediaConfig;
use crate::core::datastore::models::{EpisodeDbModel, SeriesDbModel};
use crate::core::profiles::qualities::QualityModel;

/// All the context needed to build a filename
pub struct EpisodeNamingContext<'a> {
    pub series: &'a SeriesDbModel,
    pub episodes: &'a [EpisodeDbModel],
    pub quality: &'a QualityModel,
    pub release_group: Option<&'a str>,
}

/// Build episode filename (without extension) using the naming config.
///
/// Selects the format string based on `series.series_type`:
/// - 0 (Standard) → `episode_naming_pattern`
/// - 1 (Daily)    → `daily_episode_format`
/// - 2 (Anime)    → `anime_episode_format`
pub fn build_episode_filename(config: &MediaConfig, ctx: &EpisodeNamingContext) -> String {
    let format = match ctx.series.series_type {
        1 => &config.daily_episode_format,
        2 => &config.anime_episode_format,
        _ => &config.episode_naming_pattern,
    };

    let raw = render_template(format, ctx, config.multi_episode_style);
    let cleaned = replace_colons(&raw, &config.colon_replacement_format);
    sanitize_filename(&cleaned)
}

/// Build season folder name from config template.
///
/// Uses `specials_folder_format` for season 0, `season_folder_format` otherwise.
pub fn build_season_folder(config: &MediaConfig, season_number: i32) -> String {
    if season_number == 0 {
        return config.specials_folder_format.clone();
    }

    let mut result = String::new();
    let template = &config.season_folder_format;
    let chars: Vec<char> = template.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '{' {
            if let Some(end) = chars[i..].iter().position(|&c| c == '}') {
                let token_str = &template[i + 1..i + end];
                let resolved = resolve_season_token(token_str, season_number);
                result.push_str(&resolved);
                i += end + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    sanitize_filename(&result)
}

/// Render a format template string into an episode name.
fn render_template(format: &str, ctx: &EpisodeNamingContext, multi_episode_style: i32) -> String {
    let mut result = String::new();
    let chars: Vec<char> = format.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '{' {
            if let Some(end) = chars[i..].iter().position(|&c| c == '}') {
                let token_str = &format[i + 1..i + end];
                let resolved = resolve_token(token_str, ctx, multi_episode_style);

                // If token resolved to empty and it was inside brackets like [{token}],
                // remove the surrounding brackets too
                if resolved.is_empty() {
                    // Check if previous char was '[' and next char is ']'
                    let prev_bracket = result.ends_with('[');
                    let next_bracket = i + end + 1 < len && chars[i + end + 1] == ']';
                    if prev_bracket && next_bracket {
                        result.pop(); // remove '['
                        i += end + 2; // skip past ']'
                                      // Also trim trailing space before the removed bracket
                        if result.ends_with(' ') {
                            result.pop();
                        }
                        continue;
                    }
                }

                result.push_str(&resolved);
                i += end + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Resolve a single token like "Series Title" or "season:00".
fn resolve_token(token: &str, ctx: &EpisodeNamingContext, multi_episode_style: i32) -> String {
    // Split on ':' for padding spec
    let (name, pad_spec) = match token.find(':') {
        Some(pos) => (&token[..pos], Some(&token[pos + 1..])),
        None => (token, None),
    };

    match name {
        "Series Title" => ctx.series.title.clone(),
        "Series CleanTitle" => ctx.series.clean_title.clone(),
        "Series TitleYear" => {
            if ctx.series.year > 0 {
                let year_suffix = format!("({})", ctx.series.year);
                if ctx.series.title.ends_with(&year_suffix) {
                    // Title already contains the year (e.g. "Saving Grace (2007)")
                    ctx.series.title.clone()
                } else {
                    format!("{} ({})", ctx.series.title, ctx.series.year)
                }
            } else {
                ctx.series.title.clone()
            }
        }

        "season" => {
            let season = ctx.episodes.first().map(|e| e.season_number).unwrap_or(0);
            pad_number(season, pad_spec)
        }

        "episode" => {
            if ctx.episodes.is_empty() {
                return String::new();
            }
            format_episode_numbers(ctx.episodes, pad_spec, multi_episode_style)
        }

        "Episode Title" => {
            let ep = ctx.episodes.first();
            match ep {
                Some(e) if !e.title.is_empty() => e.title.clone(),
                Some(e) => format!("Episode {}", e.episode_number),
                None => "Episode 0".to_string(),
            }
        }

        "Quality Full" => {
            let name = ctx.quality.quality.display_name();
            let rev = &ctx.quality.revision;
            if rev.is_repack {
                format!("{} Repack", name)
            } else if rev.version > 1 {
                format!("{} Proper", name)
            } else {
                name.to_string()
            }
        }

        "Quality Title" => ctx.quality.quality.display_name().to_string(),

        "Air-Date" => {
            let ep = ctx.episodes.first();
            match ep.and_then(|e| e.air_date) {
                Some(date) => date.format("%Y-%m-%d").to_string(),
                None => "Unknown".to_string(),
            }
        }

        "absolute" => {
            let abs = ctx
                .episodes
                .first()
                .and_then(|e| e.absolute_episode_number)
                .unwrap_or(0);
            pad_number(abs, pad_spec)
        }

        "Release Group" => ctx.release_group.unwrap_or("").to_string(),

        // Unknown token — pass through as-is
        _ => format!("{{{}}}", token),
    }
}

/// Resolve a season-folder token (only season-related tokens).
fn resolve_season_token(token: &str, season_number: i32) -> String {
    let (name, pad_spec) = match token.find(':') {
        Some(pos) => (&token[..pos], Some(&token[pos + 1..])),
        None => (token, None),
    };

    match name {
        "season" => pad_number(season_number, pad_spec),
        _ => format!("{{{}}}", token),
    }
}

/// Format episode numbers with multi-episode handling.
///
/// Styles (Sonarr-compatible integer values):
/// - 0 (Extend):         `01-02-03`          — bare numbers after dash
/// - 1 (Duplicate):      `01.S01E02.S01E03`  — full SxxExx repeated with dot separator
/// - 2 (Repeat):         `01E02E03`           — E-prefixed numbers, no separator
/// - 3 (Scene):          `01-E02-E03`         — E-prefixed numbers with dash separator
/// - 4 (Range):          `01-03`              — first and last, bare numbers
/// - 5 (Prefixed Range): `01-E03`             — first bare, last with E prefix
///
/// Note: The leading `E` comes from the format template (`S{season:00}E{episode:00}`),
/// so the returned string starts with the first episode number, not `E`.
fn format_episode_numbers(
    episodes: &[EpisodeDbModel],
    pad_spec: Option<&str>,
    multi_episode_style: i32,
) -> String {
    if episodes.is_empty() {
        return String::new();
    }

    let first = pad_number(episodes[0].episode_number, pad_spec);

    if episodes.len() == 1 {
        return first;
    }

    let last_ep = episodes.last().expect("checked non-empty");

    match multi_episode_style {
        1 => {
            // Duplicate: S01E01.S01E02.S01E03
            // Returns "01.S01E02.S01E03" — the leading "S01E" comes from the template
            let season = pad_number(episodes[0].season_number, Some("00"));
            let mut result = first;
            for ep in &episodes[1..] {
                result.push_str(&format!(
                    ".S{}E{}",
                    season,
                    pad_number(ep.episode_number, pad_spec)
                ));
            }
            result
        }
        2 => {
            // Repeat: S01E01E02E03
            // Returns "01E02E03" — episode numbers joined with E prefix, no separator
            let mut result = first;
            for ep in &episodes[1..] {
                result.push_str(&format!("E{}", pad_number(ep.episode_number, pad_spec)));
            }
            result
        }
        3 => {
            // Scene: S01E01-E02-E03
            // Returns "01-E02-E03" — E-prefixed numbers with dash separator
            let mut result = first;
            for ep in &episodes[1..] {
                result.push_str(&format!("-E{}", pad_number(ep.episode_number, pad_spec)));
            }
            result
        }
        4 => {
            // Range: S01E01-03
            // Returns "01-03" — bare numbers, first and last only
            let last = pad_number(last_ep.episode_number, pad_spec);
            format!("{}-{}", first, last)
        }
        5 => {
            // Prefixed Range: S01E01-E03
            // Returns "01-E03" — first bare, last with E prefix
            let last = pad_number(last_ep.episode_number, pad_spec);
            format!("{}-E{}", first, last)
        }
        _ => {
            // Extend (default, style 0): S01E01-02-03
            // Returns "01-02-03" — bare numbers with dash separator
            let mut result = first;
            for ep in &episodes[1..] {
                result.push_str(&format!("-{}", pad_number(ep.episode_number, pad_spec)));
            }
            result
        }
    }
}

/// Pad a number based on the pad spec (e.g., "00" = 2 digits, "000" = 3 digits).
fn pad_number(n: i32, pad_spec: Option<&str>) -> String {
    match pad_spec {
        Some(spec) => {
            let width = spec.len();
            format!("{:0>width$}", n, width = width)
        }
        None => n.to_string(),
    }
}

/// Replace colons based on the configured replacement format.
fn replace_colons(s: &str, format: &str) -> String {
    match format {
        "delete" => s.replace(':', ""),
        "dash" => s.replace(':', " -"),
        "space" | "spaceDash" => s.replace(':', " "),
        _ => s.replace(':', " -"), // default to dash
    }
}

/// Remove illegal filename characters and trim trailing dots/spaces.
fn sanitize_filename(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '/' | '\\' | '<' | '>' | '"' | '|' | '?' | '*' | '\0' => {}
            _ => result.push(ch),
        }
    }
    // Trim trailing dots and spaces (Windows compat)
    result
        .trim_end_matches(|c: char| c == '.' || c == ' ')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::datastore::models::{EpisodeDbModel, SeriesDbModel};
    use crate::core::profiles::qualities::{Quality, QualityModel, Revision};
    use chrono::{NaiveDate, Utc};

    fn test_series() -> SeriesDbModel {
        SeriesDbModel {
            id: 1,
            tvdb_id: 12345,
            tv_rage_id: 0,
            tv_maze_id: 0,
            imdb_id: Some("tt1234567".to_string()),
            tmdb_id: 0,
            title: "The Flash".to_string(),
            clean_title: "theflash".to_string(),
            sort_title: "flash".to_string(),
            status: 0,
            overview: None,
            monitored: true,
            monitor_new_items: 0,
            quality_profile_id: 1,
            language_profile_id: None,
            season_folder: true,
            series_type: 0,
            title_slug: "the-flash".to_string(),
            path: "/tv/The Flash".to_string(),
            root_folder_path: "/tv".to_string(),
            year: 2014,
            first_aired: None,
            last_aired: None,
            runtime: 42,
            network: Some("The CW".to_string()),
            certification: None,
            use_scene_numbering: false,
            episode_ordering: "aired".to_string(),
            added: Utc::now(),
            last_info_sync: None,
            imdb_rating: None,
            imdb_votes: None,
        }
    }

    fn test_episode(season: i32, episode: i32) -> EpisodeDbModel {
        EpisodeDbModel {
            id: 100 + episode as i64,
            series_id: 1,
            tvdb_id: 0,
            episode_file_id: None,
            season_number: season,
            episode_number: episode,
            absolute_episode_number: Some(episode + 10),
            scene_absolute_episode_number: None,
            scene_episode_number: None,
            scene_season_number: None,
            title: "Fastest Man Alive".to_string(),
            overview: None,
            air_date: Some(NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid date")),
            air_date_utc: None,
            runtime: 42,
            has_file: false,
            monitored: true,
            unverified_scene_numbering: false,
            added: Utc::now(),
            last_search_time: None,
            imdb_id: None,
            imdb_rating: None,
            imdb_votes: None,
        }
    }

    fn test_quality() -> QualityModel {
        QualityModel {
            quality: Quality::WebDl1080p,
            revision: Revision::default(),
        }
    }

    fn test_config() -> MediaConfig {
        MediaConfig {
            default_root_folder: std::path::PathBuf::from("/tv"),
            rename_episodes: true,
            replace_illegal_chars: true,
            colon_replacement_format: "dash".to_string(),
            episode_naming_pattern:
                "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Full}]"
                    .to_string(),
            daily_episode_format:
                "{Series Title} - {Air-Date} - {Episode Title} [{Quality Full}]".to_string(),
            anime_episode_format:
                "{Series Title} - S{season:00}E{episode:00} - {absolute:000} - {Episode Title} [{Quality Full}]"
                    .to_string(),
            season_folder_format: "Season {season:00}".to_string(),
            specials_folder_format: "Specials".to_string(),
            multi_episode_style: 0,
            create_empty_series_folders: false,
            delete_empty_folders: true,
            skip_free_space_check: false,
            minimum_free_space_mb: 100,
        }
    }

    #[test]
    fn test_standard_format() {
        let config = test_config();
        let series = test_series();
        let episodes = vec![test_episode(1, 5)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: Some("EVOLVE"),
        };

        let result = build_episode_filename(&config, &ctx);
        assert_eq!(
            result,
            "The Flash - S01E05 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_daily_format() {
        let config = test_config();
        let mut series = test_series();
        series.series_type = 1; // Daily
        let episodes = vec![test_episode(1, 5)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert_eq!(
            result,
            "The Flash - 2024-01-15 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_anime_format() {
        let config = test_config();
        let mut series = test_series();
        series.series_type = 2; // Anime
        let episodes = vec![test_episode(1, 5)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: Some("SubsPlease"),
        };

        let result = build_episode_filename(&config, &ctx);
        assert_eq!(
            result,
            "The Flash - S01E05 - 015 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_multi_episode_extend() {
        let config = test_config(); // multi_episode_style = 0
        let series = test_series();
        let episodes = vec![test_episode(1, 1), test_episode(1, 2), test_episode(1, 3)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        // Extend: bare numbers after dash (S01E01-02-03)
        assert_eq!(
            result,
            "The Flash - S01E01-02-03 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_multi_episode_duplicate() {
        let mut config = test_config();
        config.multi_episode_style = 1;
        let series = test_series();
        let episodes = vec![test_episode(1, 1), test_episode(1, 2), test_episode(1, 3)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        // Duplicate: full SxxExx repeated with dot separator
        assert_eq!(
            result,
            "The Flash - S01E01.S01E02.S01E03 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_multi_episode_repeat() {
        let mut config = test_config();
        config.multi_episode_style = 2;
        let series = test_series();
        let episodes = vec![test_episode(1, 1), test_episode(1, 2), test_episode(1, 3)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        // Repeat: E-prefixed numbers, no separator (S01E01E02E03)
        assert_eq!(
            result,
            "The Flash - S01E01E02E03 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_multi_episode_scene() {
        let mut config = test_config();
        config.multi_episode_style = 3;
        let series = test_series();
        let episodes = vec![test_episode(1, 1), test_episode(1, 2), test_episode(1, 3)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        // Scene: E-prefixed numbers with dash separator (S01E01-E02-E03)
        assert_eq!(
            result,
            "The Flash - S01E01-E02-E03 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_multi_episode_range() {
        let mut config = test_config();
        config.multi_episode_style = 4;
        let series = test_series();
        let episodes = vec![test_episode(1, 1), test_episode(1, 2), test_episode(1, 3)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        // Range: bare numbers, first and last (S01E01-03)
        assert_eq!(
            result,
            "The Flash - S01E01-03 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_multi_episode_prefixed_range() {
        let mut config = test_config();
        config.multi_episode_style = 5;
        let series = test_series();
        let episodes = vec![test_episode(1, 1), test_episode(1, 2), test_episode(1, 3)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        // Prefixed Range: first bare, last with E prefix (S01E01-E03)
        assert_eq!(
            result,
            "The Flash - S01E01-E03 - Fastest Man Alive [WEBDL-1080p]"
        );
    }

    #[test]
    fn test_colon_replacement_dash() {
        let config = test_config();
        let mut series = test_series();
        series.title = "DC: The Flash".to_string();
        let episodes = vec![test_episode(1, 5)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert!(result.starts_with("DC - The Flash"));
        assert!(!result.contains(':'));
    }

    #[test]
    fn test_colon_replacement_delete() {
        let mut config = test_config();
        config.colon_replacement_format = "delete".to_string();
        let mut series = test_series();
        series.title = "DC: The Flash".to_string();
        let episodes = vec![test_episode(1, 5)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert!(result.starts_with("DC The Flash"));
    }

    #[test]
    fn test_illegal_characters_stripped() {
        let result = sanitize_filename("Test: File <Name> | \"Bad\" ?.mkv");
        assert_eq!(result, "Test: File Name  Bad .mkv");
    }

    #[test]
    fn test_trailing_dots_trimmed() {
        let result = sanitize_filename("Test File...");
        assert_eq!(result, "Test File");
    }

    #[test]
    fn test_missing_episode_title() {
        let config = test_config();
        let series = test_series();
        let mut ep = test_episode(1, 5);
        ep.title = String::new();
        let episodes = vec![ep];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert!(result.contains("Episode 5"));
    }

    #[test]
    fn test_empty_release_group_removes_brackets() {
        let mut config = test_config();
        config.episode_naming_pattern =
            "{Series Title} - S{season:00}E{episode:00} [{Release Group}]".to_string();
        let series = test_series();
        let episodes = vec![test_episode(1, 5)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert_eq!(result, "The Flash - S01E05");
        assert!(!result.contains('['));
        assert!(!result.contains(']'));
    }

    #[test]
    fn test_quality_proper() {
        let config = test_config();
        let series = test_series();
        let episodes = vec![test_episode(1, 5)];
        let quality = QualityModel {
            quality: Quality::WebDl1080p,
            revision: Revision {
                version: 2,
                real: 0,
                is_repack: false,
            },
        };
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert!(result.contains("WEBDL-1080p Proper"));
    }

    #[test]
    fn test_quality_repack() {
        let config = test_config();
        let series = test_series();
        let episodes = vec![test_episode(1, 5)];
        let quality = QualityModel {
            quality: Quality::Bluray1080p,
            revision: Revision {
                version: 2,
                real: 0,
                is_repack: true,
            },
        };
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert!(result.contains("Bluray-1080p Repack"));
    }

    #[test]
    fn test_season_folder() {
        let config = test_config();
        assert_eq!(build_season_folder(&config, 1), "Season 01");
        assert_eq!(build_season_folder(&config, 12), "Season 12");
        assert_eq!(build_season_folder(&config, 0), "Specials");
    }

    #[test]
    fn test_title_year() {
        let mut config = test_config();
        config.episode_naming_pattern =
            "{Series TitleYear} - S{season:00}E{episode:00}".to_string();
        let series = test_series();
        let episodes = vec![test_episode(1, 5)];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert_eq!(result, "The Flash (2014) - S01E05");
    }

    #[test]
    fn test_no_air_date_fallback() {
        let config = test_config();
        let mut series = test_series();
        series.series_type = 1; // Daily
        let mut ep = test_episode(1, 5);
        ep.air_date = None;
        let episodes = vec![ep];
        let quality = test_quality();
        let ctx = EpisodeNamingContext {
            series: &series,
            episodes: &episodes,
            quality: &quality,
            release_group: None,
        };

        let result = build_episode_filename(&config, &ctx);
        assert!(result.contains("Unknown"));
    }
}
