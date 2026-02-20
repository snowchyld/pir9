#![allow(dead_code)]
//! pir9 CLI tool
//! Command-line interface for pir9 management

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

static EMPTY_ARRAY: Vec<serde_json::Value> = Vec::new();

#[derive(Parser)]
#[command(name = "pir9-cli")]
#[command(about = "pir9 command-line interface")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// pir9 server URL
    #[arg(short, long, env = "PIR9_URL", default_value = "http://localhost:8989")]
    url: String,

    /// API key for authentication
    #[arg(short, long, env = "PIR9_API_KEY")]
    api_key: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Series operations
    Series {
        #[command(subcommand)]
        command: SeriesCommands,
    },

    /// System operations
    System {
        #[command(subcommand)]
        command: SystemCommands,
    },

    /// Configuration operations
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum SeriesCommands {
    /// List all series
    List {
        /// Output format (table or json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Add a new series
    Add {
        /// TVDB ID
        #[arg(short, long)]
        tvdb_id: i64,

        /// Series title
        #[arg(short, long)]
        title: String,

        /// Quality profile ID
        #[arg(short, long, default_value = "1")]
        quality_profile: i64,

        /// Root folder path
        #[arg(short, long, default_value = "/data/tv")]
        root_folder: String,
    },

    /// Delete a series
    Delete {
        /// Series ID
        id: i64,

        /// Also delete files
        #[arg(short, long)]
        delete_files: bool,
    },

    /// Refresh series from metadata
    Refresh {
        /// Series ID (or "all" for all series)
        id: String,
    },

    /// Search for episodes
    Search {
        /// Series ID
        #[arg(short, long)]
        series_id: i64,

        /// Season number
        #[arg(long)]
        season: Option<i32>,

        /// Episode number
        #[arg(long)]
        episode: Option<i32>,
    },
}

#[derive(Subcommand)]
enum SystemCommands {
    /// Show system status
    Status,

    /// Run health check
    Health,

    /// Show disk space
    Diskspace,

    /// Create backup
    Backup,

    /// Restore from backup
    Restore {
        /// Backup file path
        path: String,
    },

    /// Clear logs
    ClearLogs,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Set configuration value
    Set {
        /// Configuration key (e.g., "server.port")
        key: String,

        /// Configuration value
        value: String,
    },

    /// Validate configuration (tests API connectivity)
    Validate,
}

/// HTTP client wrapper for the pir9 API
struct ApiClient {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl ApiClient {
    fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent(format!("pir9-cli/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("Failed to create HTTP client"),
            api_key,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v3{}", self.base_url, path)
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let mut req = self.client.get(self.url(path));
        if let Some(ref key) = self.api_key {
            req = req.header("X-Api-Key", key);
        }
        let resp = req
            .send()
            .await
            .context("Failed to connect to pir9 server")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, body);
        }
        resp.json().await.context("Failed to parse API response")
    }

    async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let mut req = self.client.post(self.url(path)).json(body);
        if let Some(ref key) = self.api_key {
            req = req.header("X-Api-Key", key);
        }
        let resp = req
            .send()
            .await
            .context("Failed to connect to pir9 server")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, body);
        }
        resp.json().await.context("Failed to parse API response")
    }

    async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let mut req = self.client.put(self.url(path)).json(body);
        if let Some(ref key) = self.api_key {
            req = req.header("X-Api-Key", key);
        }
        let resp = req
            .send()
            .await
            .context("Failed to connect to pir9 server")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, body);
        }
        resp.json().await.context("Failed to parse API response")
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let mut req = self.client.delete(self.url(path));
        if let Some(ref key) = self.api_key {
            req = req.header("X-Api-Key", key);
        }
        let resp = req
            .send()
            .await
            .context("Failed to connect to pir9 server")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, body);
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let api = ApiClient::new(&cli.url, cli.api_key);

    match cli.command {
        Commands::Series { command } => handle_series_command(&api, command).await,
        Commands::System { command } => handle_system_command(&api, command).await,
        Commands::Config { command } => handle_config_command(&api, command).await,
    }
}

async fn handle_series_command(api: &ApiClient, command: SeriesCommands) -> Result<()> {
    match command {
        SeriesCommands::List { format } => {
            let series = api.get("/series").await?;
            let arr = series.as_array().unwrap_or(&EMPTY_ARRAY);

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&series)?);
                return Ok(());
            }

            // Table format
            println!(
                "{:<6} {:<40} {:<8} {:<6} {:<10} {:<8}",
                "ID", "Title", "Seasons", "Eps", "Status", "Network"
            );
            println!("{}", "-".repeat(80));

            for s in arr {
                let id = s["id"].as_i64().unwrap_or(0);
                let title = s["title"].as_str().unwrap_or("?");
                let seasons = s["seasonCount"].as_i64().unwrap_or(0);
                let eps = s["episodeCount"].as_i64().unwrap_or(0);
                let status = s["status"].as_str().unwrap_or("?");
                let network = s["network"].as_str().unwrap_or("?");

                let title_truncated = if title.len() > 38 {
                    format!("{}...", &title[..35])
                } else {
                    title.to_string()
                };

                println!(
                    "{:<6} {:<40} {:<8} {:<6} {:<10} {:<8}",
                    id, title_truncated, seasons, eps, status, network
                );
            }

            println!("\nTotal: {} series", arr.len());
            Ok(())
        }
        SeriesCommands::Add {
            tvdb_id,
            title,
            quality_profile,
            root_folder,
        } => {
            let body = serde_json::json!({
                "tvdbId": tvdb_id,
                "title": title,
                "qualityProfileId": quality_profile,
                "rootFolderPath": root_folder,
                "monitored": true,
                "addOptions": {
                    "searchForMissingEpisodes": true
                }
            });

            let result = api.post("/series", &body).await?;
            let id = result["id"].as_i64().unwrap_or(0);
            println!("Added series: {} (ID: {})", title, id);
            Ok(())
        }
        SeriesCommands::Delete { id, delete_files } => {
            let path = if delete_files {
                format!("/series/{}?deleteFiles=true", id)
            } else {
                format!("/series/{}", id)
            };
            api.delete(&path).await?;
            println!("Deleted series ID: {}", id);
            Ok(())
        }
        SeriesCommands::Refresh { id } => {
            let body = if id == "all" {
                serde_json::json!({"name": "RefreshSeries"})
            } else {
                let series_id: i64 = id.parse().context("Invalid series ID")?;
                serde_json::json!({"name": "RefreshSeries", "seriesId": series_id})
            };

            api.post("/command", &body).await?;
            println!("Refresh command sent for: {}", id);
            Ok(())
        }
        SeriesCommands::Search {
            series_id,
            season,
            episode,
        } => {
            let body = if let (Some(s), Some(e)) = (season, episode) {
                // Search for specific episode
                let episodes = api
                    .get(&format!(
                        "/episode?seriesId={}&seasonNumber={}&episodeNumber={}",
                        series_id, s, e
                    ))
                    .await?;
                let ep_ids: Vec<i64> = episodes
                    .as_array()
                    .unwrap_or(&EMPTY_ARRAY)
                    .iter()
                    .filter_map(|ep| ep["id"].as_i64())
                    .collect();
                serde_json::json!({"name": "EpisodeSearch", "episodeIds": ep_ids})
            } else if let Some(s) = season {
                serde_json::json!({"name": "SeasonSearch", "seriesId": series_id, "seasonNumber": s})
            } else {
                serde_json::json!({"name": "SeriesSearch", "seriesId": series_id})
            };

            api.post("/command", &body).await?;
            println!("Search command sent");
            Ok(())
        }
    }
}

async fn handle_system_command(api: &ApiClient, command: SystemCommands) -> Result<()> {
    match command {
        SystemCommands::Status => {
            let status = api.get("/system/status").await?;

            println!("pir9 System Status");
            println!("==================");
            println!("Version:    {}", status["version"].as_str().unwrap_or("?"));
            println!("OS:         {}", status["osName"].as_str().unwrap_or("?"));
            println!(
                "OS Version: {}",
                status["osVersion"].as_str().unwrap_or("?")
            );
            println!(
                "Docker:     {}",
                status["isDocker"].as_bool().unwrap_or(false)
            );
            println!(
                "Database:   {} {}",
                status["databaseType"].as_str().unwrap_or("?"),
                status["databaseVersion"].as_str().unwrap_or("")
            );
            println!(
                "Runtime:    {} {}",
                status["runtimeName"].as_str().unwrap_or("?"),
                status["runtimeVersion"].as_str().unwrap_or("")
            );
            println!(
                "Start Time: {}",
                status["startTime"].as_str().unwrap_or("?")
            );
            println!("App Data:   {}", status["appData"].as_str().unwrap_or("?"));
            Ok(())
        }
        SystemCommands::Health => {
            let health = api.get("/health").await?;
            let checks = health.as_array().unwrap_or(&EMPTY_ARRAY);

            if checks.is_empty() {
                println!("All health checks passed");
                return Ok(());
            }

            println!("{:<20} {:<10} Message", "Source", "Type");
            println!("{}", "-".repeat(70));

            for check in checks {
                let source = check["source"].as_str().unwrap_or("?");
                let health_type = check["type"].as_str().unwrap_or("?");
                let message = check["message"].as_str().unwrap_or("");
                println!("{:<20} {:<10} {}", source, health_type, message);
            }

            Ok(())
        }
        SystemCommands::Diskspace => {
            let disks = api.get("/diskspace").await?;
            let arr = disks.as_array().unwrap_or(&EMPTY_ARRAY);

            println!(
                "{:<30} {:>14} {:>14} {:>8}",
                "Path", "Free", "Total", "Used%"
            );
            println!("{}", "-".repeat(70));

            for disk in arr {
                let path = disk["path"].as_str().unwrap_or("?");
                let free = disk["freeSpace"].as_i64().unwrap_or(0);
                let total = disk["totalSpace"].as_i64().unwrap_or(0);
                let pct = if total > 0 {
                    ((total - free) as f64 / total as f64 * 100.0) as u32
                } else {
                    0
                };

                let path_truncated = if path.len() > 28 {
                    format!("...{}", &path[path.len() - 25..])
                } else {
                    path.to_string()
                };

                println!(
                    "{:<30} {:>14} {:>14} {:>7}%",
                    path_truncated,
                    fmt_bytes(free),
                    fmt_bytes(total),
                    pct
                );
            }

            Ok(())
        }
        SystemCommands::Backup => {
            println!("Creating backup...");
            let result = api.post("/system/backup", &serde_json::json!({})).await?;
            let name = result["name"].as_str().unwrap_or("?");
            let path = result["path"].as_str().unwrap_or("?");
            let size = result["size"].as_i64().unwrap_or(0);
            println!("Backup created: {} ({}) at {}", name, fmt_bytes(size), path);
            Ok(())
        }
        SystemCommands::Restore { path } => {
            println!("Restoring from: {}", path);
            api.post("/system/backup/restore", &serde_json::json!({"path": path}))
                .await?;
            println!("Restore initiated");
            Ok(())
        }
        SystemCommands::ClearLogs => {
            // Try to clear log files in common locations
            let config_dir =
                std::env::var("PIR9_CONFIG_DIR").unwrap_or_else(|_| "/config".to_string());
            let log_path = std::path::Path::new(&config_dir).join("logs");

            if log_path.exists() {
                let mut cleared = 0;
                if let Ok(entries) = std::fs::read_dir(&log_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let is_log = path.extension().and_then(|e| e.to_str()) == Some("log")
                            || path.extension().and_then(|e| e.to_str()) == Some("txt");
                        if is_log && std::fs::remove_file(&path).is_ok() {
                            cleared += 1;
                        }
                    }
                }
                println!(
                    "Cleared {} log file(s) from {}",
                    cleared,
                    log_path.display()
                );
            } else {
                println!("Log directory not found: {}", log_path.display());
            }

            Ok(())
        }
    }
}

async fn handle_config_command(api: &ApiClient, command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Show => {
            let config = api.get("/config/host").await?;
            println!("{}", serde_json::to_string_pretty(&config)?);
            Ok(())
        }
        ConfigCommands::Set { key, value } => {
            // Get current config, update the key, PUT back
            let mut config = api.get("/config/host").await?;

            // Try to parse value as number or bool, fall back to string
            let parsed_value: serde_json::Value = if let Ok(n) = value.parse::<i64>() {
                serde_json::Value::Number(n.into())
            } else if let Ok(b) = value.parse::<bool>() {
                serde_json::Value::Bool(b)
            } else {
                serde_json::Value::String(value.clone())
            };

            if let Some(obj) = config.as_object_mut() {
                // Support dotted keys: "server.port" → config["server"]["port"]
                let parts: Vec<&str> = key.split('.').collect();
                if parts.len() == 1 {
                    obj.insert(key.clone(), parsed_value);
                } else {
                    // For dotted keys, try camelCase conversion of the last part
                    let camel_key = to_camel_case(parts.last().unwrap_or(&""));
                    obj.insert(camel_key, parsed_value);
                }
            }

            api.put("/config/host", &config).await?;
            println!("Config updated: {} = {}", key, value);
            Ok(())
        }
        ConfigCommands::Validate => match api.get("/system/status").await {
            Ok(status) => {
                let version = status["version"].as_str().unwrap_or("?");
                println!("Connection successful - pir9 v{}", version);
                Ok(())
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
                std::process::exit(1);
            }
        },
    }
}

/// Format bytes as human-readable string
fn fmt_bytes(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.1} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Convert snake_case to camelCase
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}
