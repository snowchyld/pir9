#![allow(dead_code)]
//! Indexer client implementations
//! Newznab, Torznab, and Prowlarr protocol support

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use quick_xml::de::from_str;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use super::{Protocol, ReleaseInfo};
use crate::core::profiles::languages::Language;
use crate::core::profiles::qualities::{Quality, QualityModel, Revision};

/// Indexer client trait
#[async_trait::async_trait]
pub trait IndexerClient: Send + Sync {
    /// Get indexer name
    fn name(&self) -> &str;

    /// Get protocol type
    fn protocol(&self) -> Protocol;

    /// Test connection to indexer
    async fn test(&self) -> Result<IndexerCapabilities>;

    /// Get indexer capabilities
    async fn get_capabilities(&self) -> Result<IndexerCapabilities>;

    /// Search for TV releases
    async fn search(&self, query: &SearchQuery) -> Result<Vec<ReleaseInfo>>;

    /// Fetch RSS feed
    async fn fetch_rss(&self, limit: Option<u32>) -> Result<Vec<ReleaseInfo>>;
}

/// Search query parameters
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub query: Option<String>,
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub categories: Vec<i32>,
    /// When true, use `moviesearch` action instead of `tvsearch` (Newznab/Torznab/Prowlarr)
    pub is_movie_search: bool,
}

/// Indexer capabilities from caps endpoint
#[derive(Debug, Clone, Default)]
pub struct IndexerCapabilities {
    pub search_available: bool,
    pub tv_search_available: bool,
    pub movie_search_available: bool,
    pub music_search_available: bool,
    pub book_search_available: bool,
    pub categories: Vec<IndexerCategory>,
    pub limits: IndexerLimits,
}

#[derive(Debug, Clone, Default)]
pub struct IndexerCategory {
    pub id: i32,
    pub name: String,
    pub subcategories: Vec<IndexerCategory>,
}

#[derive(Debug, Clone)]
pub struct IndexerLimits {
    pub default: u32,
    pub max: u32,
}

impl Default for IndexerLimits {
    fn default() -> Self {
        Self {
            default: 100,
            max: 100,
        }
    }
}

// ============================================================================
// Newznab Client Implementation
// ============================================================================

/// Newznab API client (used for Usenet indexers)
/// Specification: https://newznab.readthedocs.io/en/latest/misc/api/
pub struct NewznabClient {
    name: String,
    base_url: String,
    api_key: String,
    http_client: Client,
}

impl NewznabClient {
    pub fn new(name: String, url: String, api_key: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let base_url = url.trim_end_matches('/').to_string();

        Self {
            name,
            base_url,
            api_key,
            http_client,
        }
    }

    /// Build API URL with parameters
    fn build_url(&self, action: &str, extra_params: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{}/api?t={}&apikey={}&o=xml",
            self.base_url, action, self.api_key
        );

        for (key, value) in extra_params {
            url.push_str(&format!("&{}={}", key, urlencoding::encode(value)));
        }

        url
    }

    /// Parse Newznab XML response into releases
    fn parse_releases(&self, xml: &str, indexer_id: i64) -> Result<Vec<ReleaseInfo>> {
        let rss: NewznabRss = from_str(xml).context("Failed to parse Newznab XML response")?;

        let mut releases = Vec::new();

        if let Some(channel) = rss.channel {
            for item in channel.items {
                if let Some(release) = self.parse_item(&item, indexer_id) {
                    releases.push(release);
                }
            }
        }

        Ok(releases)
    }

    /// Parse a single Newznab item into ReleaseInfo
    fn parse_item(&self, item: &NewznabItem, indexer_id: i64) -> Option<ReleaseInfo> {
        let guid = item
            .guid
            .clone()
            .unwrap_or_else(|| item.link.clone().unwrap_or_default());

        let title = item.title.clone()?;

        // Extract attributes
        let mut size: i64 = 0;
        let mut tvdb_id: Option<i64> = None;
        let mut imdb_id: Option<String> = None;
        let mut season: Option<i32> = None;
        let mut episode: Option<i32> = None;
        let mut seeders: Option<i32> = None;
        let mut peers: Option<i32> = None;
        let mut categories: Vec<i32> = Vec::new();
        let mut info_hash: Option<String> = None;

        for attr in &item.newznab_attrs {
            match attr.name.as_str() {
                "size" => size = attr.value.parse().unwrap_or(0),
                "tvdbid" => tvdb_id = attr.value.parse().ok(),
                "imdb" | "imdbid" => imdb_id = Some(attr.value.clone()),
                "season" => season = attr.value.parse().ok(),
                "episode" | "ep" => episode = attr.value.parse().ok(),
                "seeders" => seeders = attr.value.parse().ok(),
                "peers" => peers = attr.value.parse().ok(),
                "category" => {
                    if let Ok(cat) = attr.value.parse::<i32>() {
                        categories.push(cat);
                    }
                }
                "infohash" => info_hash = Some(attr.value.clone()),
                _ => {}
            }
        }

        // Use enclosure size if available
        if size == 0 {
            if let Some(enc) = &item.enclosure {
                size = enc.length.parse().unwrap_or(0);
            }
        }

        // Parse publish date
        let publish_date = item
            .pub_date
            .as_ref()
            .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        // Get download URL from enclosure or link
        let download_url = item
            .enclosure
            .as_ref()
            .map(|e| e.url.clone())
            .or_else(|| item.link.clone());

        // Build episode numbers
        let episode_numbers = episode.map(|e| vec![e]).unwrap_or_default();

        Some(ReleaseInfo {
            guid,
            title,
            size,
            download_url,
            info_url: item.link.clone(),
            comment_url: item.comments.clone(),
            indexer_id,
            indexer: self.name.clone(),
            publish_date,
            download_protocol: Protocol::Usenet,
            tvdb_id,
            tv_rage_id: None,
            imdb_id,
            tmdb_id: None,
            series_title: None,
            season_number: season,
            episode_numbers,
            absolute_episode_numbers: vec![],
            mapped_season_number: None,
            mapped_episode_numbers: vec![],
            mapped_absolute_episode_numbers: vec![],
            release_group: None,
            release_hash: None,
            quality: parse_quality_from_title(&item.title.clone().unwrap_or_default()),
            languages: vec![Language::english()],
            approved: true,
            temporarily_rejected: false,
            rejected: false,
            rejections: vec![],
            seeders,
            leechers: peers.map(|p| p.saturating_sub(seeders.unwrap_or(0))),
            protocol: Protocol::Usenet,
            is_daily: false,
            is_absolute_numbering: false,
            is_possible_special_episode: false,
            special: false,
            series_id: None,
            episode_id: None,
            movie_id: None,
            download_client_id: None,
            download_client: None,
            episode_requested: false,
            download_url_generator: None,
            magnet_url: None,
            info_hash,
            seed_ratio: None,
            source_title: Some(item.title.clone().unwrap_or_default()),
            indexer_flags: 0,
            categories,
        })
    }
}

#[async_trait::async_trait]
impl IndexerClient for NewznabClient {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> Protocol {
        Protocol::Usenet
    }

    async fn test(&self) -> Result<IndexerCapabilities> {
        self.get_capabilities().await
    }

    async fn get_capabilities(&self) -> Result<IndexerCapabilities> {
        let url = self.build_url("caps", &[]);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to indexer")?;

        if !response.status().is_success() {
            anyhow::bail!("Indexer returned error: {}", response.status());
        }

        let body = response.text().await?;
        let caps: NewznabCaps = from_str(&body).context("Failed to parse capabilities XML")?;

        let mut capabilities = IndexerCapabilities::default();

        // Parse searching capabilities
        if let Some(searching) = caps.searching {
            capabilities.search_available = searching
                .search
                .as_ref()
                .map(|s| s.available == "yes")
                .unwrap_or(false);
            capabilities.tv_search_available = searching
                .tv_search
                .as_ref()
                .map(|s| s.available == "yes")
                .unwrap_or(false);
            capabilities.movie_search_available = searching
                .movie_search
                .as_ref()
                .map(|s| s.available == "yes")
                .unwrap_or(false);
        }

        // Parse categories
        if let Some(categories) = caps.categories {
            for cat in categories.categories {
                let mut category = IndexerCategory {
                    id: cat.id.parse().unwrap_or(0),
                    name: cat.name.clone(),
                    subcategories: vec![],
                };

                for subcat in cat.subcategories {
                    category.subcategories.push(IndexerCategory {
                        id: subcat.id.parse().unwrap_or(0),
                        name: subcat.name,
                        subcategories: vec![],
                    });
                }

                capabilities.categories.push(category);
            }
        }

        // Parse limits
        if let Some(limits) = caps.limits {
            capabilities.limits = IndexerLimits {
                default: limits.default.parse().unwrap_or(100),
                max: limits.max.parse().unwrap_or(100),
            };
        }

        Ok(capabilities)
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<ReleaseInfo>> {
        let mut params: Vec<(&str, String)> = Vec::new();

        // Select action: moviesearch for movies with IMDB ID, tvsearch for TV with TVDB ID, else generic search
        let action = if query.is_movie_search && query.imdb_id.is_some() {
            "moviesearch"
        } else if query.tvdb_id.is_some() {
            "tvsearch"
        } else {
            "search"
        };

        if let Some(q) = &query.query {
            params.push(("q", q.clone()));
        }
        if let Some(tvdb) = query.tvdb_id {
            params.push(("tvdbid", tvdb.to_string()));
        }
        if let Some(imdb) = &query.imdb_id {
            params.push(("imdbid", imdb.clone()));
        }
        if let Some(season) = query.season {
            params.push(("season", season.to_string()));
        }
        if let Some(ep) = query.episode {
            params.push(("ep", ep.to_string()));
        }
        if let Some(limit) = query.limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(offset) = query.offset {
            params.push(("offset", offset.to_string()));
        }
        if !query.categories.is_empty() {
            let cats: Vec<String> = query.categories.iter().map(|c| c.to_string()).collect();
            params.push(("cat", cats.join(",")));
        }

        let params_ref: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let url = self.build_url(action, &params_ref);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to search indexer")?;

        if !response.status().is_success() {
            anyhow::bail!("Search failed: {}", response.status());
        }

        let body = response.text().await?;
        self.parse_releases(&body, 0)
    }

    async fn fetch_rss(&self, limit: Option<u32>) -> Result<Vec<ReleaseInfo>> {
        // TV categories: 5000 (TV), 5010 (WEB-DL), 5020 (Foreign), 5030 (SD), 5040 (HD), 5045 (UHD), 5050 (Other), 5060 (Sport), 5070 (Anime), 5080 (Documentary)
        let mut params = vec![("cat", "5000,5010,5020,5030,5040,5045,5050,5060,5070,5080")];

        let limit_str;
        if let Some(l) = limit {
            limit_str = l.to_string();
            params.push(("limit", &limit_str));
        }

        let url = self.build_url("search", &params);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch RSS")?;

        if !response.status().is_success() {
            anyhow::bail!("RSS fetch failed: {}", response.status());
        }

        let body = response.text().await?;
        self.parse_releases(&body, 0)
    }
}

// ============================================================================
// Torznab Client Implementation (extends Newznab for torrents)
// ============================================================================

/// Torznab API client (used for torrent indexers)
/// Torznab is Newznab extended for torrents
pub struct TorznabClient {
    inner: NewznabClient,
}

impl TorznabClient {
    pub fn new(name: String, url: String, api_key: String) -> Self {
        Self {
            inner: NewznabClient::new(name, url, api_key),
        }
    }

    /// Parse releases and set torrent-specific fields
    fn parse_torznab_releases(&self, xml: &str, indexer_id: i64) -> Result<Vec<ReleaseInfo>> {
        let mut releases = self.inner.parse_releases(xml, indexer_id)?;

        // Override protocol for torrent releases
        for release in &mut releases {
            release.download_protocol = Protocol::Torrent;
            release.protocol = Protocol::Torrent;

            // If we have an info_hash, build magnet URL
            if let Some(hash) = &release.info_hash {
                let title_encoded = urlencoding::encode(&release.title);
                release.magnet_url =
                    Some(format!("magnet:?xt=urn:btih:{}&dn={}", hash, title_encoded));
            }
        }

        Ok(releases)
    }
}

#[async_trait::async_trait]
impl IndexerClient for TorznabClient {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn protocol(&self) -> Protocol {
        Protocol::Torrent
    }

    async fn test(&self) -> Result<IndexerCapabilities> {
        self.inner.get_capabilities().await
    }

    async fn get_capabilities(&self) -> Result<IndexerCapabilities> {
        self.inner.get_capabilities().await
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<ReleaseInfo>> {
        let mut params: Vec<(&str, String)> = Vec::new();

        let action = if query.is_movie_search && query.imdb_id.is_some() {
            "moviesearch"
        } else if query.tvdb_id.is_some() {
            "tvsearch"
        } else {
            "search"
        };

        if let Some(q) = &query.query {
            params.push(("q", q.clone()));
        }
        if let Some(tvdb) = query.tvdb_id {
            params.push(("tvdbid", tvdb.to_string()));
        }
        if let Some(imdb) = &query.imdb_id {
            params.push(("imdbid", imdb.clone()));
        }
        if let Some(season) = query.season {
            params.push(("season", season.to_string()));
        }
        if let Some(ep) = query.episode {
            params.push(("ep", ep.to_string()));
        }
        if let Some(limit) = query.limit {
            params.push(("limit", limit.to_string()));
        }
        if !query.categories.is_empty() {
            let cats: Vec<String> = query.categories.iter().map(|c| c.to_string()).collect();
            params.push(("cat", cats.join(",")));
        }

        let params_ref: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let url = self.inner.build_url(action, &params_ref);

        let response = self
            .inner
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to search indexer")?;

        if !response.status().is_success() {
            anyhow::bail!("Search failed: {}", response.status());
        }

        let body = response.text().await?;
        self.parse_torznab_releases(&body, 0)
    }

    async fn fetch_rss(&self, limit: Option<u32>) -> Result<Vec<ReleaseInfo>> {
        // Torrent TV categories: 5000
        let mut params = vec![("cat", "5000,5010,5020,5030,5040,5045,5050,5060,5070,5080")];

        let limit_str;
        if let Some(l) = limit {
            limit_str = l.to_string();
            params.push(("limit", &limit_str));
        }

        let url = self.inner.build_url("search", &params);

        let response = self
            .inner
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch RSS")?;

        if !response.status().is_success() {
            anyhow::bail!("RSS fetch failed: {}", response.status());
        }

        let body = response.text().await?;
        self.parse_torznab_releases(&body, 0)
    }
}

// ============================================================================
// Prowlarr Client Implementation (native REST API)
// ============================================================================

/// Prowlarr release from JSON API response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProwlarrRelease {
    guid: Option<String>,
    title: Option<String>,
    size: Option<i64>,
    download_url: Option<String>,
    info_url: Option<String>,
    indexer_id: Option<i64>,
    indexer: Option<String>,
    publish_date: Option<String>,
    tvdb_id: Option<i64>,
    imdb_id: Option<i64>,
    seeders: Option<i32>,
    leechers: Option<i32>,
    protocol: Option<String>,
    magnet_url: Option<String>,
    info_hash: Option<String>,
    #[serde(default)]
    categories: Vec<ProwlarrCategory>,
}

#[derive(Debug, Deserialize)]
struct ProwlarrCategory {
    id: Option<i32>,
}

/// Prowlarr native REST API client
/// Docs: https://prowlarr.com/docs/api/
pub struct ProwlarrClient {
    name: String,
    base_url: String,
    api_key: String,
    http_client: Client,
}

impl ProwlarrClient {
    pub fn new(name: String, url: String, api_key: String) -> Self {
        // Prowlarr aggregates across multiple indexers, so movie/music searches
        // can take significantly longer than direct indexer queries.
        let http_client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_default();

        let base_url = normalize_url(&url);

        Self {
            name,
            base_url,
            api_key,
            http_client,
        }
    }

    /// Build search query text from structured search parameters.
    /// Prowlarr's REST API doesn't accept tvdbId/season/episode as separate params,
    /// so we encode them into the query string (e.g. "Series Title S02E06").
    /// For movie searches, just use the title (no S/E appending).
    fn build_search_text(query: &SearchQuery) -> String {
        let mut text = query.query.clone().unwrap_or_default();
        if !query.is_movie_search {
            if let Some(season) = query.season {
                if let Some(episode) = query.episode {
                    text = format!("{} S{:02}E{:02}", text, season, episode);
                } else {
                    text = format!("{} S{:02}", text, season);
                }
            }
        }
        text.trim().to_string()
    }

    /// Convert a ProwlarrRelease into a ReleaseInfo
    fn map_release(&self, release: ProwlarrRelease) -> Option<ReleaseInfo> {
        let title = release.title?;
        tracing::debug!(
            "Prowlarr release '{}': guid_prefix={:?}, magnet_url={:?}, info_hash={:?}, download_url={:?}",
            title,
            release.guid.as_deref().map(|u| &u[..u.len().min(50)]),
            release.magnet_url.as_deref().map(|u| &u[..u.len().min(60)]),
            release.info_hash,
            release.download_url.as_deref().map(|u| &u[..u.len().min(80)]),
        );
        let guid_raw = release
            .guid
            .unwrap_or_else(|| release.download_url.clone().unwrap_or_default());
        // Some indexers (EZTV) put the magnet link in `guid` — extract it
        let guid_is_magnet = guid_raw.starts_with("magnet:");
        let guid = guid_raw;

        let protocol = match release.protocol.as_deref() {
            Some("usenet") => Protocol::Usenet,
            _ => Protocol::Torrent,
        };

        let publish_date = release
            .publish_date
            .as_deref()
            .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        // Prowlarr returns numeric IMDB IDs — format as "tt0000000"
        let imdb_id = release.imdb_id.map(|id| format!("tt{:07}", id));

        let categories: Vec<i32> = release.categories.iter().filter_map(|c| c.id).collect();

        // Some indexers put the magnet URI in download_url rather than magnet_url
        let download_url_magnet: Option<String> = release
            .download_url
            .as_deref()
            .filter(|u| u.starts_with("magnet:"))
            .map(String::from);

        Some(ReleaseInfo {
            guid: guid.clone(),
            title: title.clone(),
            size: release.size.unwrap_or(0),
            download_url: release.download_url,
            info_url: release.info_url,
            comment_url: None,
            indexer_id: release.indexer_id.unwrap_or(0),
            indexer: release.indexer.unwrap_or_else(|| self.name.clone()),
            publish_date,
            download_protocol: protocol,
            tvdb_id: release.tvdb_id,
            tv_rage_id: None,
            imdb_id,
            tmdb_id: None,
            series_title: None,
            season_number: None,
            episode_numbers: vec![],
            absolute_episode_numbers: vec![],
            mapped_season_number: None,
            mapped_episode_numbers: vec![],
            mapped_absolute_episode_numbers: vec![],
            release_group: None,
            release_hash: None,
            quality: parse_quality_from_title(&title),
            languages: vec![Language::english()],
            approved: true,
            temporarily_rejected: false,
            rejected: false,
            rejections: vec![],
            seeders: release.seeders,
            leechers: release.leechers,
            protocol,
            is_daily: false,
            is_absolute_numbering: false,
            is_possible_special_episode: false,
            special: false,
            series_id: None,
            episode_id: None,
            movie_id: None,
            download_client_id: None,
            download_client: None,
            episode_requested: false,
            download_url_generator: None,
            magnet_url: release
                .magnet_url
                .filter(|u| u.starts_with("magnet:"))
                .or(download_url_magnet.clone())
                .or_else(|| {
                    if guid_is_magnet {
                        Some(guid.clone())
                    } else {
                        None
                    }
                }),
            info_hash: release.info_hash.or_else(|| {
                // Extract info_hash from magnet URI in guid or download_url
                let magnet_source = if guid_is_magnet {
                    Some(guid.as_str())
                } else {
                    download_url_magnet.as_deref()
                };
                magnet_source.and_then(|uri| {
                    uri.split("btih:").nth(1).and_then(|s| {
                        let hash = s.split('&').next().unwrap_or(s);
                        if hash.is_empty() {
                            None
                        } else {
                            Some(hash.to_string())
                        }
                    })
                })
            }),
            seed_ratio: None,
            source_title: Some(title),
            indexer_flags: 0,
            categories,
        })
    }
}

#[async_trait::async_trait]
impl IndexerClient for ProwlarrClient {
    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> Protocol {
        Protocol::Torrent
    }

    async fn test(&self) -> Result<IndexerCapabilities> {
        let url = format!("{}/api/v1/health", self.base_url);

        let response = self
            .http_client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context("Failed to connect to Prowlarr")?;

        if !response.status().is_success() {
            anyhow::bail!("Prowlarr returned error: {}", response.status());
        }

        Ok(IndexerCapabilities {
            search_available: true,
            tv_search_available: true,
            movie_search_available: true,
            ..Default::default()
        })
    }

    async fn get_capabilities(&self) -> Result<IndexerCapabilities> {
        self.test().await
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<ReleaseInfo>> {
        let search_text = Self::build_search_text(query);
        let search_type = if query.is_movie_search {
            "moviesearch"
        } else {
            "tvsearch"
        };

        let mut url = format!(
            "{}/api/v1/search?query={}&type={}",
            self.base_url,
            urlencoding::encode(&search_text),
            search_type
        );

        // Prowlarr expects repeated `categories` params, not comma-separated
        for cat in &query.categories {
            url.push_str(&format!("&categories={}", cat));
        }

        if let Some(limit) = query.limit {
            url.push_str(&format!("&limit={}", limit));
        }
        if let Some(offset) = query.offset {
            url.push_str(&format!("&offset={}", offset));
        }

        tracing::debug!("Prowlarr search URL: {}", url);

        let response = self
            .http_client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context(format!("Failed to search Prowlarr (type={}, url={})", search_type, &url[..url.find('?').unwrap_or(url.len())]))?;

        if !response.status().is_success() {
            anyhow::bail!("Prowlarr search failed: status={}, type={}", response.status(), search_type);
        }

        let releases: Vec<ProwlarrRelease> = response
            .json()
            .await
            .context(format!("Failed to parse Prowlarr JSON response (type={})", search_type))?;

        Ok(releases
            .into_iter()
            .filter_map(|r| self.map_release(r))
            .collect())
    }

    async fn fetch_rss(&self, limit: Option<u32>) -> Result<Vec<ReleaseInfo>> {
        let mut url = format!(
            "{}/api/v1/search?type=search&categories=5000&categories=5010&categories=5020&categories=5030&categories=5040&categories=5045&categories=5050&categories=5060&categories=5070&categories=5080",
            self.base_url
        );

        if let Some(l) = limit {
            url.push_str(&format!("&limit={}", l));
        }

        let response = self
            .http_client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context("Failed to fetch Prowlarr RSS")?;

        if !response.status().is_success() {
            anyhow::bail!("Prowlarr RSS fetch failed: {}", response.status());
        }

        let releases: Vec<ProwlarrRelease> = response
            .json()
            .await
            .context("Failed to parse Prowlarr RSS JSON response")?;

        Ok(releases
            .into_iter()
            .filter_map(|r| self.map_release(r))
            .collect())
    }
}

/// Detect Prowlarr per-indexer proxy URLs (e.g., `https://prowlarr.example.com/5/api`).
/// These are Torznab/Newznab XML endpoints, NOT the Prowlarr REST API.
fn is_prowlarr_per_indexer_url(url: &str) -> bool {
    // Pattern: URL path ends with `/{digits}/api` (e.g., /5/api, /12/api)
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with("/api") {
        let before_api = &trimmed[..trimmed.len() - 4]; // strip "/api"
        if let Some(last_segment) = before_api.rsplit('/').next() {
            return !last_segment.is_empty() && last_segment.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

/// Normalize a URL by ensuring it has a scheme prefix
fn normalize_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    }
}

// ============================================================================
// XML Parsing Structures
// ============================================================================

#[derive(Debug, Deserialize)]
struct NewznabRss {
    channel: Option<NewznabChannel>,
}

#[derive(Debug, Deserialize)]
struct NewznabChannel {
    #[serde(rename = "item", default)]
    items: Vec<NewznabItem>,
}

#[derive(Debug, Deserialize)]
struct NewznabItem {
    title: Option<String>,
    guid: Option<String>,
    link: Option<String>,
    comments: Option<String>,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
    enclosure: Option<NewznabEnclosure>,
    #[serde(rename = "attr", default)]
    newznab_attrs: Vec<NewznabAttr>,
}

#[derive(Debug, Deserialize)]
struct NewznabEnclosure {
    url: String,
    #[serde(default)]
    length: String,
    #[serde(rename = "type", default)]
    mime_type: String,
}

#[derive(Debug, Deserialize)]
struct NewznabAttr {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@value")]
    value: String,
}

#[derive(Debug, Deserialize)]
struct NewznabCaps {
    searching: Option<NewznabSearching>,
    categories: Option<NewznabCategories>,
    limits: Option<NewznabLimits>,
}

#[derive(Debug, Deserialize)]
struct NewznabSearching {
    search: Option<SearchCapability>,
    #[serde(rename = "tv-search")]
    tv_search: Option<SearchCapability>,
    #[serde(rename = "movie-search")]
    movie_search: Option<SearchCapability>,
}

#[derive(Debug, Deserialize)]
struct SearchCapability {
    #[serde(rename = "@available")]
    available: String,
}

#[derive(Debug, Deserialize)]
struct NewznabCategories {
    #[serde(rename = "category", default)]
    categories: Vec<NewznabCategory>,
}

#[derive(Debug, Deserialize)]
struct NewznabCategory {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "subcat", default)]
    subcategories: Vec<NewznabSubcat>,
}

#[derive(Debug, Deserialize)]
struct NewznabSubcat {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@name")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct NewznabLimits {
    #[serde(rename = "@default")]
    default: String,
    #[serde(rename = "@max")]
    max: String,
}

// ============================================================================
// Quality Parsing
// ============================================================================

/// Parse quality from release title
fn parse_quality_from_title(title: &str) -> QualityModel {
    let title_lower = title.to_lowercase();

    let quality = if title_lower.contains("2160p")
        || title_lower.contains("4k")
        || title_lower.contains("uhd")
    {
        if title_lower.contains("remux") {
            Quality::Bluray2160pRemux
        } else if title_lower.contains("bluray") || title_lower.contains("blu-ray") {
            Quality::Bluray2160p
        } else if title_lower.contains("webdl")
            || title_lower.contains("web-dl")
            || title_lower.contains("webrip")
            || title_lower.contains("web-rip")
        {
            Quality::WebDl2160p
        } else {
            Quality::Hdtv2160p
        }
    } else if title_lower.contains("1080p") {
        if title_lower.contains("remux") {
            Quality::Bluray1080pRemux
        } else if title_lower.contains("bluray") || title_lower.contains("blu-ray") {
            Quality::Bluray1080p
        } else if title_lower.contains("webdl")
            || title_lower.contains("web-dl")
            || title_lower.contains("webrip")
            || title_lower.contains("web-rip")
        {
            Quality::WebDl1080p
        } else {
            Quality::Hdtv1080p
        }
    } else if title_lower.contains("720p") {
        if title_lower.contains("bluray") || title_lower.contains("blu-ray") {
            Quality::Bluray720p
        } else if title_lower.contains("webdl")
            || title_lower.contains("web-dl")
            || title_lower.contains("webrip")
            || title_lower.contains("web-rip")
        {
            Quality::WebDl720p
        } else {
            Quality::Hdtv720p
        }
    } else if title_lower.contains("480p") {
        if title_lower.contains("webdl")
            || title_lower.contains("web-dl")
            || title_lower.contains("webrip")
            || title_lower.contains("web-rip")
        {
            Quality::WebDl480p
        } else if title_lower.contains("dvd") {
            Quality::Dvd
        } else {
            Quality::SDTV
        }
    } else if title_lower.contains("hdtv") {
        if title_lower.contains("1080") {
            Quality::Hdtv1080p
        } else {
            Quality::Hdtv720p
        }
    } else if title_lower.contains("dvd") {
        Quality::Dvd
    } else {
        Quality::Unknown
    };

    let is_repack = title_lower.contains("repack") || title_lower.contains("proper");
    let version = if is_repack { 2 } else { 1 };

    QualityModel {
        quality,
        revision: Revision {
            version,
            real: 0,
            is_repack,
        },
    }
}

// ============================================================================
// Factory Function
// ============================================================================

/// Create an indexer client from database model
pub fn create_client_from_model(
    model: &crate::core::datastore::models::IndexerDbModel,
) -> Result<Box<dyn IndexerClient>> {
    let settings: serde_json::Value =
        serde_json::from_str(&model.settings).context("Failed to parse indexer settings")?;

    let base_url = settings
        .get("baseUrl")
        .or_else(|| settings.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let api_key = settings
        .get("apiKey")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match model.implementation.as_str() {
        "Newznab" => Ok(Box::new(NewznabClient::new(
            model.name.clone(),
            base_url.to_string(),
            api_key.to_string(),
        ))),
        "Torznab" => Ok(Box::new(TorznabClient::new(
            model.name.clone(),
            base_url.to_string(),
            api_key.to_string(),
        ))),
        "Prowlarr" => {
            // Prowlarr per-indexer proxy URLs (e.g., /5/api) are Torznab endpoints,
            // not the Prowlarr REST API. Detect this and use TorznabClient instead.
            // Strip trailing "/api" since TorznabClient.build_url() appends its own "/api".
            if is_prowlarr_per_indexer_url(base_url) {
                let proxy_base = base_url.trim_end_matches('/').trim_end_matches("/api");
                tracing::debug!(
                    "Indexer '{}' uses Prowlarr per-indexer proxy URL, using Torznab client (base={})",
                    model.name,
                    proxy_base
                );
                Ok(Box::new(TorznabClient::new(
                    model.name.clone(),
                    proxy_base.to_string(),
                    api_key.to_string(),
                )))
            } else {
                Ok(Box::new(ProwlarrClient::new(
                    model.name.clone(),
                    base_url.to_string(),
                    api_key.to_string(),
                )))
            }
        }
        _ => {
            anyhow::bail!(
                "Unsupported indexer implementation: {}",
                model.implementation
            )
        }
    }
}
