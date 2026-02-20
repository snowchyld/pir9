#![allow(dead_code, unused_imports)]
//! Download client implementations
//! qBittorrent, SABnzbd, and other download client integrations

use anyhow::{Context, Result};
use reqwest::{multipart, Client};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Download client trait
#[async_trait::async_trait]
pub trait DownloadClient: Send + Sync {
    /// Get client name
    fn name(&self) -> &str;

    /// Get protocol type (usenet or torrent)
    fn protocol(&self) -> DownloadProtocol;

    /// Test connection to client
    async fn test(&self) -> Result<()>;

    /// Get client version
    async fn get_version(&self) -> Result<String>;

    /// Add download by URL
    async fn add_from_url(&self, url: &str, options: DownloadOptions) -> Result<String>;

    /// Add download by magnet link
    async fn add_from_magnet(&self, magnet: &str, options: DownloadOptions) -> Result<String>;

    /// Add download by file (NZB/Torrent)
    async fn add_from_file(
        &self,
        file_data: &[u8],
        filename: &str,
        options: DownloadOptions,
    ) -> Result<String>;

    /// Get all downloads
    async fn get_downloads(&self) -> Result<Vec<DownloadStatus>>;

    /// Get specific download
    async fn get_download(&self, id: &str) -> Result<Option<DownloadStatus>>;

    /// Remove download
    async fn remove(&self, id: &str, delete_files: bool) -> Result<()>;

    /// Pause download
    async fn pause(&self, id: &str) -> Result<()>;

    /// Resume download
    async fn resume(&self, id: &str) -> Result<()>;
}

/// Download protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadProtocol {
    Unknown = 0,
    Usenet = 1,
    Torrent = 2,
}

/// Download options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadOptions {
    pub category: Option<String>,
    pub priority: Option<i32>,
    pub download_dir: Option<String>,
    pub tags: Vec<String>,
}

/// Download status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatus {
    pub id: String,
    pub name: String,
    pub status: DownloadState,
    pub size: i64,
    pub size_left: i64,
    pub progress: f64,
    pub download_speed: i64,
    pub upload_speed: i64,
    pub eta: Option<i64>,
    pub error_message: Option<String>,
    pub output_path: Option<String>,
    pub category: Option<String>,
}

/// Download state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    Queued,
    Paused,
    Downloading,
    Seeding,
    Completed,
    Failed,
    Warning,
}

// ============================================================================
// qBittorrent Client Implementation
// ============================================================================

/// qBittorrent Web API client
/// Implements the qBittorrent Web API v2
/// https://github.com/qbittorrent/qBittorrent/wiki/WebUI-API-(qBittorrent-4.1)
pub struct QBittorrentClient {
    base_url: String,
    username: String,
    password: String,
    http_client: Client,
    /// Session cookie (SID) - stored after successful login
    session: Arc<RwLock<Option<String>>>,
}

impl QBittorrentClient {
    pub fn new(url: String, username: String, password: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .cookie_store(true)
            .build()
            .unwrap_or_default();

        // Normalize URL (remove trailing slash)
        let base_url = url.trim_end_matches('/').to_string();

        Self {
            base_url,
            username,
            password,
            http_client,
            session: Arc::new(RwLock::new(None)),
        }
    }

    /// Login to qBittorrent and store session cookie
    async fn login(&self) -> Result<()> {
        let url = format!("{}/api/v2/auth/login", self.base_url);

        tracing::debug!("Attempting qBittorrent login to: {}", url);

        let response = self
            .http_client
            .post(&url)
            .form(&[("username", &self.username), ("password", &self.password)])
            .send()
            .await
            .map_err(|e| {
                // Provide detailed error information
                if e.is_connect() {
                    anyhow::anyhow!(
                        "Cannot connect to qBittorrent at {}. Error: {}",
                        self.base_url,
                        e
                    )
                } else if e.is_timeout() {
                    anyhow::anyhow!("Connection to qBittorrent timed out at {}", self.base_url)
                } else {
                    anyhow::anyhow!(
                        "Failed to connect to qBittorrent at {}: {}",
                        self.base_url,
                        e
                    )
                }
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status.is_success() && body.contains("Ok") {
            tracing::debug!("qBittorrent login successful");
            Ok(())
        } else if body.contains("Fails") {
            anyhow::bail!("qBittorrent authentication failed: invalid username or password")
        } else if status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!(
                "qBittorrent returned 403 Forbidden. Check if Web UI is enabled and accessible."
            )
        } else {
            anyhow::bail!(
                "qBittorrent login failed (HTTP {}): {}",
                status.as_u16(),
                if body.is_empty() {
                    "no response body"
                } else {
                    &body
                }
            )
        }
    }

    /// Ensure we have a valid session, logging in if necessary
    async fn ensure_session(&self) -> Result<()> {
        // Try a simple API call to check if session is valid
        let url = format!("{}/api/v2/app/version", self.base_url);
        let response = self.http_client.get(&url).send().await;

        match response {
            Ok(r) if r.status().is_success() => Ok(()),
            _ => {
                // Need to login
                self.login().await
            }
        }
    }

    /// Make an authenticated GET request
    async fn get(&self, endpoint: &str) -> Result<String> {
        self.ensure_session().await?;

        let url = format!("{}{}", self.base_url, endpoint);
        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("qBittorrent request failed")?;

        if response.status() == reqwest::StatusCode::FORBIDDEN {
            // Session expired, try re-login
            self.login().await?;
            let response = self
                .http_client
                .get(&url)
                .send()
                .await
                .context("qBittorrent request failed after re-login")?;
            Ok(response.text().await?)
        } else if response.status().is_success() {
            Ok(response.text().await?)
        } else {
            anyhow::bail!("qBittorrent API error: {}", response.status())
        }
    }

    /// Make an authenticated POST request with form data
    async fn post_form(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<String> {
        self.ensure_session().await?;

        let url = format!("{}{}", self.base_url, endpoint);
        let response = self
            .http_client
            .post(&url)
            .form(params)
            .send()
            .await
            .context("qBittorrent request failed")?;

        if response.status() == reqwest::StatusCode::FORBIDDEN {
            self.login().await?;
            let response = self
                .http_client
                .post(&url)
                .form(params)
                .send()
                .await
                .context("qBittorrent request failed after re-login")?;
            Ok(response.text().await?)
        } else if response.status().is_success() {
            Ok(response.text().await?)
        } else {
            anyhow::bail!("qBittorrent API error: {}", response.status())
        }
    }
}

/// qBittorrent torrent info response
#[derive(Debug, Deserialize)]
struct QBTorrentInfo {
    hash: String,
    name: String,
    size: i64,
    progress: f64,
    dlspeed: i64,
    upspeed: i64,
    eta: i64,
    state: String,
    category: Option<String>,
    save_path: Option<String>,
    #[serde(default)]
    amount_left: i64,
}

impl QBTorrentInfo {
    fn to_download_status(&self) -> DownloadStatus {
        let state = match self.state.as_str() {
            "pausedDL" | "pausedUP" => DownloadState::Paused,
            "queuedDL" | "queuedUP" | "checkingDL" | "checkingUP" | "checkingResumeData" => {
                DownloadState::Queued
            }
            "downloading" | "forcedDL" | "metaDL" | "allocating" => DownloadState::Downloading,
            "uploading" | "forcedUP" | "stalledUP" => DownloadState::Seeding,
            "stalledDL" => DownloadState::Downloading,
            "error" | "missingFiles" => DownloadState::Failed,
            "moving" => DownloadState::Downloading,
            _ => DownloadState::Queued,
        };

        // Check if completed (progress >= 1.0 or amount_left == 0)
        let state = if self.progress >= 1.0 || (self.amount_left == 0 && self.size > 0) {
            if state == DownloadState::Paused {
                DownloadState::Paused
            } else {
                DownloadState::Completed
            }
        } else {
            state
        };

        DownloadStatus {
            id: self.hash.clone(),
            name: self.name.clone(),
            status: state,
            size: self.size,
            size_left: self.amount_left,
            progress: self.progress * 100.0,
            download_speed: self.dlspeed,
            upload_speed: self.upspeed,
            eta: if self.eta > 0 && self.eta < 8640000 {
                Some(self.eta)
            } else {
                None
            },
            error_message: None,
            output_path: self.save_path.clone(),
            category: self.category.clone(),
        }
    }
}

#[async_trait::async_trait]
impl DownloadClient for QBittorrentClient {
    fn name(&self) -> &str {
        "qBittorrent"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Torrent
    }

    async fn test(&self) -> Result<()> {
        self.login().await?;
        self.get_version().await?;
        Ok(())
    }

    async fn get_version(&self) -> Result<String> {
        let version = self.get("/api/v2/app/version").await?;
        Ok(version.trim().to_string())
    }

    async fn add_from_url(&self, url: &str, options: DownloadOptions) -> Result<String> {
        self.ensure_session().await?;

        let api_url = format!("{}/api/v2/torrents/add", self.base_url);

        let mut form = multipart::Form::new().text("urls", url.to_string());

        if let Some(category) = options.category {
            form = form.text("category", category);
        }
        if let Some(dir) = options.download_dir {
            form = form.text("savepath", dir);
        }

        let response = self
            .http_client
            .post(&api_url)
            .multipart(form)
            .send()
            .await
            .context("Failed to add torrent")?;

        if response.status().is_success() {
            // qBittorrent doesn't return the hash, we need to query for it
            // For now, return a placeholder - in real usage, we'd need to extract from magnet/URL
            Ok("added".to_string())
        } else {
            anyhow::bail!("Failed to add torrent: {}", response.status())
        }
    }

    async fn add_from_magnet(&self, magnet: &str, options: DownloadOptions) -> Result<String> {
        // Magnet links are added the same way as URLs in qBittorrent
        self.add_from_url(magnet, options).await
    }

    async fn add_from_file(
        &self,
        file_data: &[u8],
        filename: &str,
        options: DownloadOptions,
    ) -> Result<String> {
        self.ensure_session().await?;

        let api_url = format!("{}/api/v2/torrents/add", self.base_url);

        let file_part = multipart::Part::bytes(file_data.to_vec())
            .file_name(filename.to_string())
            .mime_str("application/x-bittorrent")?;

        let mut form = multipart::Form::new().part("torrents", file_part);

        if let Some(category) = options.category {
            form = form.text("category", category);
        }
        if let Some(dir) = options.download_dir {
            form = form.text("savepath", dir);
        }

        let response = self
            .http_client
            .post(&api_url)
            .multipart(form)
            .send()
            .await
            .context("Failed to add torrent file")?;

        if response.status().is_success() {
            Ok("added".to_string())
        } else {
            anyhow::bail!("Failed to add torrent: {}", response.status())
        }
    }

    async fn get_downloads(&self) -> Result<Vec<DownloadStatus>> {
        let body = self.get("/api/v2/torrents/info").await?;
        let torrents: Vec<QBTorrentInfo> =
            serde_json::from_str(&body).context("Failed to parse qBittorrent response")?;

        Ok(torrents.iter().map(|t| t.to_download_status()).collect())
    }

    async fn get_download(&self, id: &str) -> Result<Option<DownloadStatus>> {
        let body = self
            .get(&format!("/api/v2/torrents/info?hashes={}", id))
            .await?;
        let torrents: Vec<QBTorrentInfo> =
            serde_json::from_str(&body).context("Failed to parse qBittorrent response")?;

        Ok(torrents.first().map(|t| t.to_download_status()))
    }

    async fn remove(&self, id: &str, delete_files: bool) -> Result<()> {
        let delete_files_str = if delete_files { "true" } else { "false" };
        self.post_form(
            "/api/v2/torrents/delete",
            &[("hashes", id), ("deleteFiles", delete_files_str)],
        )
        .await?;
        Ok(())
    }

    async fn pause(&self, id: &str) -> Result<()> {
        self.post_form("/api/v2/torrents/pause", &[("hashes", id)])
            .await?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> Result<()> {
        self.post_form("/api/v2/torrents/resume", &[("hashes", id)])
            .await?;
        Ok(())
    }
}

// ============================================================================
// SABnzbd Client Implementation
// ============================================================================

/// SABnzbd API client
/// Implements the SABnzbd API
/// https://sabnzbd.org/wiki/advanced/api
pub struct SabnzbdClient {
    base_url: String,
    api_key: String,
    http_client: Client,
}

impl SabnzbdClient {
    pub fn new(url: String, api_key: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let base_url = url.trim_end_matches('/').to_string();

        Self {
            base_url,
            api_key,
            http_client,
        }
    }

    /// Make an API request
    async fn api_call(
        &self,
        mode: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let mut url = format!(
            "{}/api?output=json&apikey={}&mode={}",
            self.base_url, self.api_key, mode
        );

        for (key, value) in extra_params {
            url.push_str(&format!("&{}={}", key, urlencoding::encode(value)));
        }

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to SABnzbd")?;

        if !response.status().is_success() {
            anyhow::bail!("SABnzbd API error: {}", response.status());
        }

        let body = response.text().await?;
        let json: serde_json::Value =
            serde_json::from_str(&body).context("Failed to parse SABnzbd response")?;

        // Check for API errors
        if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
            if !error.is_empty() {
                anyhow::bail!("SABnzbd error: {}", error);
            }
        }

        Ok(json)
    }
}

/// SABnzbd queue slot info
#[derive(Debug, Deserialize)]
struct SABSlot {
    nzo_id: String,
    filename: String,
    #[serde(default)]
    mb: f64,
    #[serde(default)]
    mbleft: f64,
    #[serde(default)]
    percentage: String,
    status: String,
    #[serde(default)]
    timeleft: String,
    cat: Option<String>,
    #[serde(default)]
    storage: Option<String>,
}

impl SABSlot {
    fn to_download_status(&self) -> DownloadStatus {
        let state = match self.status.as_str() {
            "Queued" => DownloadState::Queued,
            "Paused" => DownloadState::Paused,
            "Downloading" => DownloadState::Downloading,
            "Grabbing" => DownloadState::Downloading,
            "Completed" => DownloadState::Completed,
            "Failed" => DownloadState::Failed,
            "Extracting" | "Verifying" | "Repairing" | "Moving" => DownloadState::Downloading,
            _ => DownloadState::Queued,
        };

        let progress: f64 = self.percentage.parse().unwrap_or(0.0);
        let size = (self.mb * 1024.0 * 1024.0) as i64;
        let size_left = (self.mbleft * 1024.0 * 1024.0) as i64;

        // Parse time left (format: "0:00:00" or "")
        let eta = parse_timeleft(&self.timeleft);

        DownloadStatus {
            id: self.nzo_id.clone(),
            name: self.filename.clone(),
            status: state,
            size,
            size_left,
            progress,
            download_speed: 0, // SABnzbd doesn't provide per-item speed
            upload_speed: 0,
            eta,
            error_message: None,
            output_path: self.storage.clone(),
            category: self.cat.clone(),
        }
    }
}

/// Parse SABnzbd time left format (HH:MM:SS or empty)
fn parse_timeleft(timeleft: &str) -> Option<i64> {
    if timeleft.is_empty() {
        return None;
    }

    let parts: Vec<&str> = timeleft.split(':').collect();
    if parts.len() == 3 {
        let hours: i64 = parts[0].parse().unwrap_or(0);
        let minutes: i64 = parts[1].parse().unwrap_or(0);
        let seconds: i64 = parts[2].parse().unwrap_or(0);
        Some(hours * 3600 + minutes * 60 + seconds)
    } else {
        None
    }
}

#[async_trait::async_trait]
impl DownloadClient for SabnzbdClient {
    fn name(&self) -> &str {
        "SABnzbd"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Usenet
    }

    async fn test(&self) -> Result<()> {
        self.get_version().await?;
        Ok(())
    }

    async fn get_version(&self) -> Result<String> {
        let json = self.api_call("version", &[]).await?;
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        Ok(version.to_string())
    }

    async fn add_from_url(&self, url: &str, options: DownloadOptions) -> Result<String> {
        let mut params = vec![("name", url)];

        let category_str;
        if let Some(cat) = &options.category {
            category_str = cat.clone();
            params.push(("cat", &category_str));
        }

        let json = self.api_call("addurl", &params).await?;

        // SABnzbd returns { "status": true, "nzo_ids": ["SABnzbd_nzo_xxx"] }
        if let Some(ids) = json.get("nzo_ids").and_then(|v| v.as_array()) {
            if let Some(id) = ids.first().and_then(|v| v.as_str()) {
                return Ok(id.to_string());
            }
        }

        // Check for status
        if json
            .get("status")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            Ok("added".to_string())
        } else {
            anyhow::bail!("Failed to add NZB: {:?}", json)
        }
    }

    async fn add_from_magnet(&self, _magnet: &str, _options: DownloadOptions) -> Result<String> {
        anyhow::bail!("SABnzbd does not support magnet links")
    }

    async fn add_from_file(
        &self,
        file_data: &[u8],
        filename: &str,
        options: DownloadOptions,
    ) -> Result<String> {
        let url = format!(
            "{}/api?output=json&apikey={}&mode=addfile",
            self.base_url, self.api_key
        );

        let file_part = multipart::Part::bytes(file_data.to_vec())
            .file_name(filename.to_string())
            .mime_str("application/x-nzb")?;

        let mut form = multipart::Form::new().part("nzbfile", file_part);

        if let Some(cat) = options.category {
            form = form.text("cat", cat);
        }

        let response = self
            .http_client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .context("Failed to add NZB file")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to add NZB: {}", response.status());
        }

        let json: serde_json::Value = response.json().await?;

        if let Some(ids) = json.get("nzo_ids").and_then(|v| v.as_array()) {
            if let Some(id) = ids.first().and_then(|v| v.as_str()) {
                return Ok(id.to_string());
            }
        }

        if json
            .get("status")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            Ok("added".to_string())
        } else {
            anyhow::bail!("Failed to add NZB: {:?}", json)
        }
    }

    async fn get_downloads(&self) -> Result<Vec<DownloadStatus>> {
        let json = self.api_call("queue", &[]).await?;

        let mut downloads = Vec::new();

        // Parse queue slots
        if let Some(queue) = json.get("queue") {
            if let Some(slots) = queue.get("slots").and_then(|s| s.as_array()) {
                for slot in slots {
                    if let Ok(sab_slot) = serde_json::from_value::<SABSlot>(slot.clone()) {
                        downloads.push(sab_slot.to_download_status());
                    }
                }
            }
        }

        // Also check history for recently completed
        let history_json = self.api_call("history", &[("limit", "20")]).await?;
        if let Some(history) = history_json.get("history") {
            if let Some(slots) = history.get("slots").and_then(|s| s.as_array()) {
                for slot in slots {
                    if let Some(status) = slot.get("status").and_then(|s| s.as_str()) {
                        if status == "Completed" || status == "Failed" {
                            let nzo_id = slot.get("nzo_id").and_then(|s| s.as_str()).unwrap_or("");
                            let name = slot.get("name").and_then(|s| s.as_str()).unwrap_or("");
                            let size = slot.get("bytes").and_then(|s| s.as_i64()).unwrap_or(0);
                            let storage = slot.get("storage").and_then(|s| s.as_str());
                            let cat = slot.get("category").and_then(|s| s.as_str());

                            downloads.push(DownloadStatus {
                                id: nzo_id.to_string(),
                                name: name.to_string(),
                                status: if status == "Completed" {
                                    DownloadState::Completed
                                } else {
                                    DownloadState::Failed
                                },
                                size,
                                size_left: 0,
                                progress: 100.0,
                                download_speed: 0,
                                upload_speed: 0,
                                eta: None,
                                error_message: slot
                                    .get("fail_message")
                                    .and_then(|s| s.as_str())
                                    .map(|s| s.to_string()),
                                output_path: storage.map(|s| s.to_string()),
                                category: cat.map(|s| s.to_string()),
                            });
                        }
                    }
                }
            }
        }

        Ok(downloads)
    }

    async fn get_download(&self, id: &str) -> Result<Option<DownloadStatus>> {
        let downloads = self.get_downloads().await?;
        Ok(downloads.into_iter().find(|d| d.id == id))
    }

    async fn remove(&self, id: &str, _delete_files: bool) -> Result<()> {
        // Try removing from queue first
        let _ = self
            .api_call("queue", &[("name", "delete"), ("value", id)])
            .await;

        // Also try removing from history
        let _ = self
            .api_call("history", &[("name", "delete"), ("value", id)])
            .await;

        Ok(())
    }

    async fn pause(&self, id: &str) -> Result<()> {
        self.api_call("queue", &[("name", "pause"), ("value", id)])
            .await?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> Result<()> {
        self.api_call("queue", &[("name", "resume"), ("value", id)])
            .await?;
        Ok(())
    }
}

// ============================================================================
// NZBGet Client Implementation
// ============================================================================

/// NZBGet JSON-RPC API client
/// Implements the NZBGet API
/// https://nzbget.net/api
pub struct NzbgetClient {
    base_url: String,
    username: String,
    password: String,
    http_client: Client,
}

impl NzbgetClient {
    pub fn new(url: String, username: String, password: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let base_url = url.trim_end_matches('/').to_string();

        Self {
            base_url,
            username,
            password,
            http_client,
        }
    }

    /// Make a JSON-RPC request to NZBGet
    async fn rpc_call(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/jsonrpc", self.base_url);

        let body = serde_json::json!({
            "method": method,
            "params": params,
        });

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&body)
            .send()
            .await
            .context("Failed to connect to NZBGet")?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!("NZBGet authentication failed: invalid username or password");
        }

        if !response.status().is_success() {
            anyhow::bail!("NZBGet API error: {}", response.status());
        }

        let resp_body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse NZBGet response")?;

        // Check for JSON-RPC error
        if let Some(error) = resp_body.get("error") {
            if !error.is_null() {
                anyhow::bail!("NZBGet RPC error: {}", error);
            }
        }

        Ok(resp_body
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }
}

/// NZBGet group (download) info from listgroups
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct NzbgetGroup {
    NZBID: i64,
    NZBName: String,
    #[serde(default)]
    FileSizeMB: f64,
    #[serde(default)]
    RemainingSizeMB: f64,
    Status: String,
    #[serde(default)]
    Category: String,
    #[serde(default)]
    DestDir: String,
    #[serde(default)]
    DownloadRate: i64,
    #[serde(default)]
    RemainingSec: i64,
}

impl NzbgetGroup {
    fn to_download_status(&self) -> DownloadStatus {
        let state = match self.Status.as_str() {
            "QUEUED" => DownloadState::Queued,
            "PAUSED" => DownloadState::Paused,
            "DOWNLOADING" => DownloadState::Downloading,
            "FETCHING" => DownloadState::Downloading,
            s if s.starts_with("PP_") || s == "POST_PROCESSING" => DownloadState::Downloading,
            "UNPACKING" => DownloadState::Downloading,
            _ => DownloadState::Queued,
        };

        let size = (self.FileSizeMB * 1024.0 * 1024.0) as i64;
        let size_left = (self.RemainingSizeMB * 1024.0 * 1024.0) as i64;
        let progress = if size > 0 {
            ((size - size_left) as f64 / size as f64) * 100.0
        } else {
            0.0
        };

        let eta = if self.RemainingSec > 0 {
            Some(self.RemainingSec)
        } else {
            None
        };

        DownloadStatus {
            id: self.NZBID.to_string(),
            name: self.NZBName.clone(),
            status: state,
            size,
            size_left,
            progress,
            download_speed: self.DownloadRate,
            upload_speed: 0,
            eta,
            error_message: None,
            output_path: if self.DestDir.is_empty() {
                None
            } else {
                Some(self.DestDir.clone())
            },
            category: if self.Category.is_empty() {
                None
            } else {
                Some(self.Category.clone())
            },
        }
    }
}

/// NZBGet history item
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct NzbgetHistoryItem {
    NZBID: i64,
    Name: String,
    #[serde(default)]
    FileSizeMB: f64,
    Status: String,
    #[serde(default)]
    Category: String,
    #[serde(default)]
    DestDir: String,
}

impl NzbgetHistoryItem {
    fn to_download_status(&self) -> DownloadStatus {
        let state = if self.Status.starts_with("SUCCESS") {
            DownloadState::Completed
        } else {
            DownloadState::Failed
        };

        let size = (self.FileSizeMB * 1024.0 * 1024.0) as i64;

        DownloadStatus {
            id: self.NZBID.to_string(),
            name: self.Name.clone(),
            status: state,
            size,
            size_left: 0,
            progress: 100.0,
            download_speed: 0,
            upload_speed: 0,
            eta: None,
            error_message: if state == DownloadState::Failed {
                Some(format!("NZBGet status: {}", self.Status))
            } else {
                None
            },
            output_path: if self.DestDir.is_empty() {
                None
            } else {
                Some(self.DestDir.clone())
            },
            category: if self.Category.is_empty() {
                None
            } else {
                Some(self.Category.clone())
            },
        }
    }
}

#[async_trait::async_trait]
impl DownloadClient for NzbgetClient {
    fn name(&self) -> &str {
        "NZBGet"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Usenet
    }

    async fn test(&self) -> Result<()> {
        self.get_version().await?;
        Ok(())
    }

    async fn get_version(&self) -> Result<String> {
        let result = self.rpc_call("version", vec![]).await?;
        Ok(result.as_str().unwrap_or("unknown").to_string())
    }

    async fn add_from_url(&self, url: &str, options: DownloadOptions) -> Result<String> {
        let category = options.category.unwrap_or_else(|| "tv-sonarr".to_string());

        // NZBGet append params: (NZBFilename, NZBContent, Category, Priority, DupeKey, DupeScore, DupeMode, PPParameters)
        // When NZBContent is empty string and NZBFilename is a URL, NZBGet downloads from the URL
        let result = self
            .rpc_call(
                "append",
                vec![
                    serde_json::Value::String(url.to_string()), // NZBFilename (URL)
                    serde_json::Value::String(String::new()), // NZBContent (empty = download from URL)
                    serde_json::Value::String(category),      // Category
                    serde_json::Value::Number(0.into()),      // Priority (0 = normal)
                    serde_json::Value::Bool(false),           // AddToTop
                    serde_json::Value::Bool(false),           // AddPaused
                    serde_json::Value::String(String::new()), // DupeKey
                    serde_json::Value::Number(0.into()),      // DupeScore
                    serde_json::Value::String("SCORE".to_string()), // DupeMode
                ],
            )
            .await?;

        // append returns the NZBID (integer) on success, or 0 on failure
        let nzb_id = result.as_i64().unwrap_or(0);
        if nzb_id == 0 {
            anyhow::bail!("NZBGet failed to add NZB from URL");
        }

        Ok(nzb_id.to_string())
    }

    async fn add_from_magnet(&self, _magnet: &str, _options: DownloadOptions) -> Result<String> {
        anyhow::bail!("NZBGet does not support magnet links")
    }

    async fn add_from_file(
        &self,
        file_data: &[u8],
        filename: &str,
        options: DownloadOptions,
    ) -> Result<String> {
        use base64::{engine::general_purpose, Engine as _};

        let category = options.category.unwrap_or_else(|| "tv-sonarr".to_string());
        let encoded = general_purpose::STANDARD.encode(file_data);

        let result = self
            .rpc_call(
                "append",
                vec![
                    serde_json::Value::String(filename.to_string()), // NZBFilename
                    serde_json::Value::String(encoded),              // NZBContent (base64)
                    serde_json::Value::String(category),             // Category
                    serde_json::Value::Number(0.into()),             // Priority
                    serde_json::Value::Bool(false),                  // AddToTop
                    serde_json::Value::Bool(false),                  // AddPaused
                    serde_json::Value::String(String::new()),        // DupeKey
                    serde_json::Value::Number(0.into()),             // DupeScore
                    serde_json::Value::String("SCORE".to_string()),  // DupeMode
                ],
            )
            .await?;

        let nzb_id = result.as_i64().unwrap_or(0);
        if nzb_id == 0 {
            anyhow::bail!("NZBGet failed to add NZB file");
        }

        Ok(nzb_id.to_string())
    }

    async fn get_downloads(&self) -> Result<Vec<DownloadStatus>> {
        let mut downloads = Vec::new();

        // Get active downloads from listgroups
        let groups_result = self.rpc_call("listgroups", vec![]).await?;
        if let Some(groups) = groups_result.as_array() {
            for group in groups {
                if let Ok(g) = serde_json::from_value::<NzbgetGroup>(group.clone()) {
                    downloads.push(g.to_download_status());
                }
            }
        }

        // Get recently completed/failed from history (last 20)
        let history_result = self
            .rpc_call(
                "history",
                vec![
                    serde_json::Value::Bool(false), // Hidden (false = not hidden only)
                ],
            )
            .await?;
        if let Some(items) = history_result.as_array() {
            for (i, item) in items.iter().enumerate() {
                if i >= 20 {
                    break;
                }
                if let Ok(h) = serde_json::from_value::<NzbgetHistoryItem>(item.clone()) {
                    downloads.push(h.to_download_status());
                }
            }
        }

        Ok(downloads)
    }

    async fn get_download(&self, id: &str) -> Result<Option<DownloadStatus>> {
        let downloads = self.get_downloads().await?;
        Ok(downloads.into_iter().find(|d| d.id == id))
    }

    async fn remove(&self, id: &str, delete_files: bool) -> Result<()> {
        let nzb_id: i64 = id.parse().unwrap_or(0);
        if nzb_id == 0 {
            anyhow::bail!("Invalid NZBGet download ID: {}", id);
        }

        let command = if delete_files {
            "GroupFinalDelete"
        } else {
            "GroupDelete"
        };

        // Try removing from queue first
        let _ = self
            .rpc_call(
                "editqueue",
                vec![
                    serde_json::Value::String(command.to_string()),
                    serde_json::Value::String(String::new()),
                    serde_json::json!([nzb_id]),
                ],
            )
            .await;

        // Also try removing from history
        let _ = self
            .rpc_call(
                "editqueue",
                vec![
                    serde_json::Value::String("HistoryFinalDelete".to_string()),
                    serde_json::Value::String(String::new()),
                    serde_json::json!([nzb_id]),
                ],
            )
            .await;

        Ok(())
    }

    async fn pause(&self, id: &str) -> Result<()> {
        let nzb_id: i64 = id.parse().unwrap_or(0);
        self.rpc_call(
            "editqueue",
            vec![
                serde_json::Value::String("GroupPause".to_string()),
                serde_json::Value::String(String::new()),
                serde_json::json!([nzb_id]),
            ],
        )
        .await?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> Result<()> {
        let nzb_id: i64 = id.parse().unwrap_or(0);
        self.rpc_call(
            "editqueue",
            vec![
                serde_json::Value::String("GroupResume".to_string()),
                serde_json::Value::String(String::new()),
                serde_json::json!([nzb_id]),
            ],
        )
        .await?;
        Ok(())
    }
}

// ============================================================================
// Transmission Client Implementation
// ============================================================================

/// Transmission RPC client
/// Uses JSON-RPC with X-Transmission-Session-Id CSRF protection
pub struct TransmissionClient {
    rpc_url: String,
    username: Option<String>,
    password: Option<String>,
    session_id: RwLock<Option<String>>,
    http_client: Client,
}

impl TransmissionClient {
    pub fn new(url: String, username: Option<String>, password: Option<String>) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let rpc_url = format!("{}/transmission/rpc", url.trim_end_matches('/'));

        Self {
            rpc_url,
            username,
            password,
            session_id: RwLock::new(None),
            http_client,
        }
    }

    /// Make an RPC request, handling 409 session ID refresh automatically
    async fn rpc_call(
        &self,
        method: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "method": method,
            "arguments": arguments,
        });

        // Try with current session ID
        let response = self.do_request(&body).await?;

        if response.status().as_u16() == 409 {
            // Extract new session ID from response header
            if let Some(new_id) = response
                .headers()
                .get("X-Transmission-Session-Id")
                .and_then(|v| v.to_str().ok())
            {
                let mut session = self.session_id.write().await;
                *session = Some(new_id.to_string());
            }
            // Retry with new session ID
            let response = self.do_request(&body).await?;
            self.parse_response(response).await
        } else {
            self.parse_response(response).await
        }
    }

    async fn do_request(&self, body: &serde_json::Value) -> Result<reqwest::Response> {
        let mut req = self.http_client.post(&self.rpc_url).json(body);

        // Add session ID header if available
        if let Some(ref id) = *self.session_id.read().await {
            req = req.header("X-Transmission-Session-Id", id);
        }

        // Add basic auth if credentials provided
        if let (Some(ref user), Some(ref pass)) = (&self.username, &self.password) {
            if !user.is_empty() {
                req = req.basic_auth(user, Some(pass));
            }
        }

        req.send()
            .await
            .context("Failed to connect to Transmission")
    }

    async fn parse_response(&self, response: reqwest::Response) -> Result<serde_json::Value> {
        if !response.status().is_success() {
            anyhow::bail!("Transmission returned HTTP {}", response.status().as_u16());
        }

        let json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Transmission response")?;

        if json["result"].as_str() != Some("success") {
            let err = json["result"]
                .as_str()
                .unwrap_or("unknown error")
                .to_string();
            anyhow::bail!("Transmission RPC error: {}", err);
        }

        Ok(json["arguments"].clone())
    }

    fn map_status(status: i64) -> DownloadState {
        match status {
            0 => DownloadState::Paused,        // TR_STATUS_STOPPED
            1 | 2 => DownloadState::Queued,    // CHECK_WAIT / CHECK
            3 => DownloadState::Queued,        // DOWNLOAD_WAIT
            4 => DownloadState::Downloading,   // DOWNLOADING
            5 | 6 => DownloadState::Completed, // SEED_WAIT / SEEDING
            _ => DownloadState::Queued,
        }
    }

    fn torrent_to_status(torrent: &serde_json::Value) -> DownloadStatus {
        let total_size = torrent["totalSize"].as_i64().unwrap_or(0);
        let left = torrent["leftUntilDone"].as_i64().unwrap_or(0);
        let percent = torrent["percentDone"].as_f64().unwrap_or(0.0) * 100.0;
        let status_code = torrent["status"].as_i64().unwrap_or(0);
        let error = torrent["error"].as_i64().unwrap_or(0);

        let state = if error > 0 {
            DownloadState::Failed
        } else {
            Self::map_status(status_code)
        };

        let eta = torrent["eta"].as_i64().filter(|&e| e >= 0);
        let error_message = if error > 0 {
            torrent["errorString"].as_str().map(String::from)
        } else {
            None
        };

        DownloadStatus {
            id: torrent["hashString"].as_str().unwrap_or("").to_string(),
            name: torrent["name"].as_str().unwrap_or("").to_string(),
            status: state,
            size: total_size,
            size_left: left,
            progress: percent,
            download_speed: torrent["rateDownload"].as_i64().unwrap_or(0),
            upload_speed: torrent["rateUpload"].as_i64().unwrap_or(0),
            eta,
            error_message,
            output_path: torrent["downloadDir"].as_str().map(String::from),
            category: None,
        }
    }
}

#[async_trait::async_trait]
impl DownloadClient for TransmissionClient {
    fn name(&self) -> &str {
        "Transmission"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Torrent
    }

    async fn test(&self) -> Result<()> {
        self.rpc_call("session-get", serde_json::json!({ "fields": ["version"] }))
            .await?;
        Ok(())
    }

    async fn get_version(&self) -> Result<String> {
        let result = self
            .rpc_call("session-get", serde_json::json!({ "fields": ["version"] }))
            .await?;
        Ok(result["version"].as_str().unwrap_or("unknown").to_string())
    }

    async fn add_from_url(&self, url: &str, options: DownloadOptions) -> Result<String> {
        let mut args = serde_json::json!({
            "filename": url,
            "paused": false,
        });
        if let Some(ref dir) = options.download_dir {
            args["download-dir"] = serde_json::Value::String(dir.clone());
        }

        let result = self.rpc_call("torrent-add", args).await?;

        // Response has either "torrent-added" or "torrent-duplicate"
        let torrent = result
            .get("torrent-added")
            .or_else(|| result.get("torrent-duplicate"))
            .ok_or_else(|| anyhow::anyhow!("Transmission did not return torrent info"))?;

        Ok(torrent["hashString"].as_str().unwrap_or("").to_string())
    }

    async fn add_from_magnet(&self, magnet: &str, options: DownloadOptions) -> Result<String> {
        self.add_from_url(magnet, options).await
    }

    async fn add_from_file(
        &self,
        file_data: &[u8],
        _filename: &str,
        options: DownloadOptions,
    ) -> Result<String> {
        use base64::{engine::general_purpose, Engine as _};
        let encoded = general_purpose::STANDARD.encode(file_data);

        let mut args = serde_json::json!({
            "metainfo": encoded,
            "paused": false,
        });
        if let Some(ref dir) = options.download_dir {
            args["download-dir"] = serde_json::Value::String(dir.clone());
        }

        let result = self.rpc_call("torrent-add", args).await?;

        let torrent = result
            .get("torrent-added")
            .or_else(|| result.get("torrent-duplicate"))
            .ok_or_else(|| anyhow::anyhow!("Transmission did not return torrent info"))?;

        Ok(torrent["hashString"].as_str().unwrap_or("").to_string())
    }

    async fn get_downloads(&self) -> Result<Vec<DownloadStatus>> {
        let result = self
            .rpc_call(
                "torrent-get",
                serde_json::json!({
                    "fields": [
                        "id", "name", "hashString", "status", "totalSize",
                        "percentDone", "leftUntilDone", "sizeWhenDone",
                        "rateDownload", "rateUpload", "eta",
                        "error", "errorString", "downloadDir",
                        "isFinished", "addedDate", "doneDate"
                    ]
                }),
            )
            .await?;

        let torrents = result["torrents"]
            .as_array()
            .map(|arr| arr.iter().map(Self::torrent_to_status).collect())
            .unwrap_or_default();

        Ok(torrents)
    }

    async fn get_download(&self, id: &str) -> Result<Option<DownloadStatus>> {
        let result = self
            .rpc_call(
                "torrent-get",
                serde_json::json!({
                    "ids": [id],
                    "fields": [
                        "id", "name", "hashString", "status", "totalSize",
                        "percentDone", "leftUntilDone", "sizeWhenDone",
                        "rateDownload", "rateUpload", "eta",
                        "error", "errorString", "downloadDir",
                        "isFinished"
                    ]
                }),
            )
            .await?;

        Ok(result["torrents"]
            .as_array()
            .and_then(|arr| arr.first())
            .map(Self::torrent_to_status))
    }

    async fn remove(&self, id: &str, delete_files: bool) -> Result<()> {
        self.rpc_call(
            "torrent-remove",
            serde_json::json!({
                "ids": [id],
                "delete-local-data": delete_files,
            }),
        )
        .await?;
        Ok(())
    }

    async fn pause(&self, id: &str) -> Result<()> {
        self.rpc_call("torrent-stop", serde_json::json!({ "ids": [id] }))
            .await?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> Result<()> {
        self.rpc_call("torrent-start", serde_json::json!({ "ids": [id] }))
            .await?;
        Ok(())
    }
}

// ============================================================================
// Deluge Client Implementation
// ============================================================================

/// Deluge Web UI JSON-RPC client
/// Authenticates via password + session cookie, connects to deluged daemon
pub struct DelugeClient {
    base_url: String,
    password: String,
    http_client: Client,
    request_id: std::sync::atomic::AtomicI64,
}

impl DelugeClient {
    pub fn new(url: String, password: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .cookie_store(true)
            .build()
            .unwrap_or_default();

        let base_url = url.trim_end_matches('/').to_string();

        Self {
            base_url,
            password,
            http_client,
            request_id: std::sync::atomic::AtomicI64::new(1),
        }
    }

    fn next_id(&self) -> i64 {
        self.request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Make a JSON-RPC call to Deluge Web UI
    async fn rpc_call(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let id = self.next_id();
        let body = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
        });

        let url = format!("{}/json", self.base_url);
        let resp = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to connect to Deluge")?;

        if !resp.status().is_success() {
            anyhow::bail!("Deluge returned HTTP {}", resp.status().as_u16());
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Deluge response")?;

        if let Some(error) = json.get("error") {
            if !error.is_null() {
                let msg = error["message"]
                    .as_str()
                    .unwrap_or("unknown error")
                    .to_string();
                anyhow::bail!("Deluge RPC error: {}", msg);
            }
        }

        Ok(json["result"].clone())
    }

    /// Authenticate with Deluge and ensure daemon connection
    async fn ensure_connected(&self) -> Result<()> {
        // Login
        let result = self
            .rpc_call(
                "auth.login",
                vec![serde_json::Value::String(self.password.clone())],
            )
            .await?;

        if result != serde_json::Value::Bool(true) {
            anyhow::bail!("Deluge authentication failed");
        }

        // Check if connected to daemon
        let connected = self.rpc_call("web.connected", vec![]).await?;
        if connected == serde_json::Value::Bool(true) {
            return Ok(());
        }

        // Get available hosts and connect to first one
        let hosts = self.rpc_call("web.get_hosts", vec![]).await?;
        if let Some(host_list) = hosts.as_array() {
            if let Some(host) = host_list.first() {
                if let Some(host_id) = host.get(0).and_then(|v| v.as_str()) {
                    self.rpc_call(
                        "web.connect",
                        vec![serde_json::Value::String(host_id.to_string())],
                    )
                    .await?;
                    return Ok(());
                }
            }
        }

        anyhow::bail!("No Deluge daemon hosts available")
    }

    fn map_state(state: &str) -> DownloadState {
        match state {
            "Downloading" => DownloadState::Downloading,
            "Seeding" => DownloadState::Completed,
            "Paused" => DownloadState::Paused,
            "Queued" => DownloadState::Queued,
            "Checking" => DownloadState::Queued,
            "Error" => DownloadState::Failed,
            "Allocating" | "Moving" => DownloadState::Downloading,
            _ => DownloadState::Queued,
        }
    }

    fn torrent_to_status(hash: &str, torrent: &serde_json::Value) -> DownloadStatus {
        let total_size = torrent["total_size"].as_i64().unwrap_or(0);
        let total_done = torrent["total_done"].as_i64().unwrap_or(0);
        let progress = torrent["progress"].as_f64().unwrap_or(0.0);
        let state_str = torrent["state"].as_str().unwrap_or("Queued");
        let message = torrent["message"].as_str().unwrap_or("");

        let state = if !message.is_empty() && state_str == "Error" {
            DownloadState::Failed
        } else {
            Self::map_state(state_str)
        };

        let eta = torrent["eta"].as_i64().filter(|&e| e > 0);
        let error_message = if !message.is_empty() {
            Some(message.to_string())
        } else {
            None
        };

        DownloadStatus {
            id: hash.to_string(),
            name: torrent["name"].as_str().unwrap_or("").to_string(),
            status: state,
            size: total_size,
            size_left: total_size - total_done,
            progress,
            download_speed: torrent["download_payload_rate"].as_i64().unwrap_or(0),
            upload_speed: torrent["upload_payload_rate"].as_i64().unwrap_or(0),
            eta,
            error_message,
            output_path: torrent["save_path"].as_str().map(String::from),
            category: torrent["label"].as_str().map(String::from),
        }
    }
}

#[async_trait::async_trait]
impl DownloadClient for DelugeClient {
    fn name(&self) -> &str {
        "Deluge"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Torrent
    }

    async fn test(&self) -> Result<()> {
        self.ensure_connected().await?;
        self.rpc_call("daemon.get_version", vec![]).await?;
        Ok(())
    }

    async fn get_version(&self) -> Result<String> {
        self.ensure_connected().await?;
        let version = self.rpc_call("daemon.get_version", vec![]).await?;
        Ok(version.as_str().unwrap_or("unknown").to_string())
    }

    async fn add_from_url(&self, url: &str, options: DownloadOptions) -> Result<String> {
        self.ensure_connected().await?;

        let mut opts = serde_json::json!({ "add_paused": false });
        if let Some(ref dir) = options.download_dir {
            opts["download_location"] = serde_json::Value::String(dir.clone());
        }

        let result = self
            .rpc_call(
                "core.add_torrent_url",
                vec![
                    serde_json::Value::String(url.to_string()),
                    opts,
                    serde_json::json!({}),
                ],
            )
            .await?;

        result
            .as_str()
            .map(String::from)
            .ok_or_else(|| anyhow::anyhow!("Deluge did not return torrent hash"))
    }

    async fn add_from_magnet(&self, magnet: &str, options: DownloadOptions) -> Result<String> {
        self.ensure_connected().await?;

        let mut opts = serde_json::json!({ "add_paused": false });
        if let Some(ref dir) = options.download_dir {
            opts["download_location"] = serde_json::Value::String(dir.clone());
        }

        let result = self
            .rpc_call(
                "core.add_torrent_magnet",
                vec![serde_json::Value::String(magnet.to_string()), opts],
            )
            .await?;

        result
            .as_str()
            .map(String::from)
            .ok_or_else(|| anyhow::anyhow!("Deluge did not return torrent hash"))
    }

    async fn add_from_file(
        &self,
        file_data: &[u8],
        filename: &str,
        options: DownloadOptions,
    ) -> Result<String> {
        self.ensure_connected().await?;
        use base64::{engine::general_purpose, Engine as _};
        let encoded = general_purpose::STANDARD.encode(file_data);

        let mut opts = serde_json::json!({ "add_paused": false });
        if let Some(ref dir) = options.download_dir {
            opts["download_location"] = serde_json::Value::String(dir.clone());
        }

        let result = self
            .rpc_call(
                "core.add_torrent_file",
                vec![
                    serde_json::Value::String(filename.to_string()),
                    serde_json::Value::String(encoded),
                    opts,
                ],
            )
            .await?;

        result
            .as_str()
            .map(String::from)
            .ok_or_else(|| anyhow::anyhow!("Deluge did not return torrent hash"))
    }

    async fn get_downloads(&self) -> Result<Vec<DownloadStatus>> {
        self.ensure_connected().await?;

        let result = self
            .rpc_call(
                "web.update_ui",
                vec![
                    serde_json::json!([
                        "hash",
                        "name",
                        "state",
                        "progress",
                        "eta",
                        "message",
                        "is_finished",
                        "save_path",
                        "total_size",
                        "total_done",
                        "time_added",
                        "download_payload_rate",
                        "upload_payload_rate",
                        "label"
                    ]),
                    serde_json::json!({}),
                ],
            )
            .await?;

        let mut downloads = Vec::new();
        if let Some(torrents) = result.get("torrents").and_then(|t| t.as_object()) {
            for (hash, torrent) in torrents {
                downloads.push(Self::torrent_to_status(hash, torrent));
            }
        }

        Ok(downloads)
    }

    async fn get_download(&self, id: &str) -> Result<Option<DownloadStatus>> {
        self.ensure_connected().await?;

        let result = self
            .rpc_call(
                "web.get_torrent_status",
                vec![
                    serde_json::Value::String(id.to_string()),
                    serde_json::json!([
                        "hash",
                        "name",
                        "state",
                        "progress",
                        "eta",
                        "save_path",
                        "total_size",
                        "total_done",
                        "download_payload_rate",
                        "upload_payload_rate",
                        "is_finished",
                        "message",
                        "label"
                    ]),
                ],
            )
            .await?;

        if result.is_null() || result.as_object().map(|o| o.is_empty()).unwrap_or(true) {
            return Ok(None);
        }

        Ok(Some(Self::torrent_to_status(id, &result)))
    }

    async fn remove(&self, id: &str, delete_files: bool) -> Result<()> {
        self.ensure_connected().await?;
        self.rpc_call(
            "core.remove_torrent",
            vec![
                serde_json::Value::String(id.to_string()),
                serde_json::Value::Bool(delete_files),
            ],
        )
        .await?;
        Ok(())
    }

    async fn pause(&self, id: &str) -> Result<()> {
        self.ensure_connected().await?;
        self.rpc_call(
            "core.pause_torrent",
            vec![serde_json::Value::String(id.to_string())],
        )
        .await?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> Result<()> {
        self.ensure_connected().await?;
        self.rpc_call(
            "core.resume_torrent",
            vec![serde_json::Value::String(id.to_string())],
        )
        .await?;
        Ok(())
    }
}

// ============================================================================
// Download Client Factory
// ============================================================================

/// Parse a value as port number (handles both string and integer)
fn parse_port(value: Option<&serde_json::Value>) -> i64 {
    match value {
        Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(8080),
        Some(serde_json::Value::String(s)) => s.parse().unwrap_or(8080),
        _ => 8080,
    }
}

/// Create a download client from database model
pub fn create_client_from_model(
    model: &crate::core::datastore::models::DownloadClientDbModel,
) -> Result<Box<dyn DownloadClient>> {
    let settings: serde_json::Value = serde_json::from_str(&model.settings)
        .context("Failed to parse download client settings")?;

    match model.implementation.as_str() {
        "QBittorrent" => {
            let host = settings
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("localhost");
            let port = parse_port(settings.get("port"));
            let use_ssl = settings
                .get("useSsl")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let username = settings
                .get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let password = settings
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let protocol = if use_ssl { "https" } else { "http" };
            let url = format!("{}://{}:{}", protocol, host, port);

            tracing::debug!(
                "Creating qBittorrent client: url={}, username={}",
                url,
                username
            );

            Ok(Box::new(QBittorrentClient::new(
                url,
                username.to_string(),
                password.to_string(),
            )))
        }
        "Sabnzbd" | "SABnzbd" => {
            let host = settings
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("localhost");
            let port = parse_port(settings.get("port"));
            let use_ssl = settings
                .get("useSsl")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let api_key = settings
                .get("apiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let protocol = if use_ssl { "https" } else { "http" };
            let url = format!("{}://{}:{}", protocol, host, port);

            tracing::debug!("Creating SABnzbd client: url={}", url);

            Ok(Box::new(SabnzbdClient::new(url, api_key.to_string())))
        }
        "Nzbget" | "NZBGet" | "NzbGet" => {
            let host = settings
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("localhost");
            let port = parse_port(settings.get("port"));
            let use_ssl = settings
                .get("useSsl")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let username = settings
                .get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("nzbget");
            let password = settings
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("tegbzn6789");

            let protocol = if use_ssl { "https" } else { "http" };
            let url = format!("{}://{}:{}", protocol, host, port);

            tracing::debug!("Creating NZBGet client: url={}, username={}", url, username);

            Ok(Box::new(NzbgetClient::new(
                url,
                username.to_string(),
                password.to_string(),
            )))
        }
        "Transmission" => {
            let host = settings
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("localhost");
            let port = parse_port(settings.get("port"));
            let use_ssl = settings
                .get("useSsl")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let username = settings
                .get("username")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            let password = settings
                .get("password")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);

            let protocol = if use_ssl { "https" } else { "http" };
            let url = format!("{}://{}:{}", protocol, host, port);

            tracing::debug!("Creating Transmission client: url={}", url);

            Ok(Box::new(TransmissionClient::new(url, username, password)))
        }
        "Deluge" => {
            let host = settings
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("localhost");
            let port = parse_port(settings.get("port"));
            let use_ssl = settings
                .get("useSsl")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let password = settings
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("deluge")
                .to_string();

            let protocol = if use_ssl { "https" } else { "http" };
            let url = format!("{}://{}:{}", protocol, host, port);

            tracing::debug!("Creating Deluge client: url={}", url);

            Ok(Box::new(DelugeClient::new(url, password)))
        }
        _ => {
            anyhow::bail!(
                "Unsupported download client implementation: {}",
                model.implementation
            )
        }
    }
}
