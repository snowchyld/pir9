#![allow(dead_code, unused_imports)]
//! RSS feed parser for podcast feeds
//! Uses the `rss` crate to parse podcast RSS/XML feeds

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};

/// Parsed feed metadata from an RSS feed
#[derive(Debug, Clone)]
pub struct FeedMetadata {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub link: Option<String>,
    pub image_url: Option<String>,
    pub categories: Vec<String>,
    pub episodes: Vec<FeedEpisode>,
}

/// A single episode parsed from an RSS feed item
#[derive(Debug, Clone)]
pub struct FeedEpisode {
    pub title: String,
    pub description: Option<String>,
    pub guid: Option<String>,
    pub pub_date: Option<DateTime<Utc>>,
    pub duration_ms: Option<i32>,
    pub download_url: Option<String>,
    pub file_size: Option<i64>,
    pub episode_number: Option<i32>,
    pub season_number: Option<i32>,
}

/// Fetch and parse an RSS feed from a URL
pub async fn fetch_feed(url: &str) -> Result<FeedMetadata> {
    info!("Fetching podcast feed: {}", url);

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", format!("pir9/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .await
        .context("Failed to fetch RSS feed")?;

    if !response.status().is_success() {
        anyhow::bail!("RSS feed returned status: {}", response.status());
    }

    let body = response
        .bytes()
        .await
        .context("Failed to read RSS feed body")?;

    parse_feed(&body)
}

/// Parse RSS feed XML bytes into structured metadata
pub fn parse_feed(data: &[u8]) -> Result<FeedMetadata> {
    let channel = rss::Channel::read_from(data).context("Failed to parse RSS feed XML")?;

    let image_url = channel
        .image()
        .map(|img| img.url().to_string())
        .or_else(|| {
            // iTunes image extension
            channel
                .itunes_ext()
                .and_then(|ext| ext.image().map(|s| s.to_string()))
        });

    let author = channel
        .itunes_ext()
        .and_then(|ext| ext.author().map(|s| s.to_string()));

    let categories: Vec<String> = channel
        .itunes_ext()
        .map(|ext| {
            ext.categories()
                .iter()
                .map(|c| c.text().to_string())
                .collect()
        })
        .unwrap_or_default();

    let episodes: Vec<FeedEpisode> = channel
        .items()
        .iter()
        .enumerate()
        .map(|(_idx, item)| {
            let guid = item.guid().map(|g| g.value().to_string());

            let pub_date = item.pub_date().and_then(|s| {
                DateTime::parse_from_rfc2822(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

            // Parse enclosure for download URL and file size
            let (download_url, file_size) = item
                .enclosure()
                .map(|enc| {
                    let url = enc.url().to_string();
                    let size = enc.length().parse::<i64>().ok();
                    (Some(url), size)
                })
                .unwrap_or((None, None));

            // Parse duration from iTunes extension (can be HH:MM:SS, MM:SS, or seconds)
            let duration_ms = item
                .itunes_ext()
                .and_then(|ext| ext.duration().map(|s| parse_duration_ms(s)));

            // Episode/season from iTunes extension
            let episode_number = item
                .itunes_ext()
                .and_then(|ext| ext.episode().and_then(|s| s.parse::<i32>().ok()));

            let season_number = item
                .itunes_ext()
                .and_then(|ext| ext.season().and_then(|s| s.parse::<i32>().ok()));

            let description = item
                .description()
                .map(|s| s.to_string())
                .or_else(|| {
                    item.itunes_ext()
                        .and_then(|ext| ext.summary().map(|s| s.to_string()))
                });

            FeedEpisode {
                title: item.title().unwrap_or("Untitled").to_string(),
                description,
                guid,
                pub_date,
                duration_ms,
                download_url,
                file_size,
                episode_number,
                season_number,
            }
        })
        .collect();

    info!(
        "Parsed feed '{}' with {} episodes",
        channel.title(),
        episodes.len()
    );

    let description = {
        let desc = channel.description();
        if desc.is_empty() {
            None
        } else {
            Some(desc.to_string())
        }
    };

    let link = {
        let l = channel.link();
        if l.is_empty() { None } else { Some(l.to_string()) }
    };

    Ok(FeedMetadata {
        title: channel.title().to_string(),
        description,
        author,
        link,
        image_url,
        categories,
        episodes,
    })
}

/// Parse an iTunes duration string into milliseconds.
/// Accepts: "3600" (seconds), "1:00:00" (H:M:S), "60:00" (M:S)
fn parse_duration_ms(s: &str) -> i32 {
    let parts: Vec<&str> = s.split(':').collect();
    let total_seconds = match parts.len() {
        1 => parts[0].parse::<i64>().unwrap_or(0),
        2 => {
            let mins = parts[0].parse::<i64>().unwrap_or(0);
            let secs = parts[1].parse::<i64>().unwrap_or(0);
            mins * 60 + secs
        }
        3 => {
            let hours = parts[0].parse::<i64>().unwrap_or(0);
            let mins = parts[1].parse::<i64>().unwrap_or(0);
            let secs = parts[2].parse::<i64>().unwrap_or(0);
            hours * 3600 + mins * 60 + secs
        }
        _ => 0,
    };

    (total_seconds * 1000) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_ms_seconds() {
        assert_eq!(parse_duration_ms("3600"), 3_600_000);
    }

    #[test]
    fn test_parse_duration_ms_mm_ss() {
        assert_eq!(parse_duration_ms("60:00"), 3_600_000);
    }

    #[test]
    fn test_parse_duration_ms_hh_mm_ss() {
        assert_eq!(parse_duration_ms("1:30:00"), 5_400_000);
    }

    #[test]
    fn test_parse_duration_ms_invalid() {
        assert_eq!(parse_duration_ms("invalid"), 0);
    }
}
