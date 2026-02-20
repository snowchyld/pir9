#![allow(dead_code, unused_imports)]
//! Job scheduler module
//! Handles periodic tasks like RSS sync, library refresh, etc.

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::core::datastore::Database;
use crate::core::datastore::repositories::{
    IndexerRepository, DownloadClientRepository, SeriesRepository, RootFolderRepository,
};
use crate::core::indexers::rss::RssSyncService;
use crate::core::download::clients::create_client_from_model as create_download_client;
use crate::core::download::import::ImportService;
use crate::core::indexers::create_client_from_model as create_indexer_client;

/// Job scheduler for managing background tasks
#[derive(Debug, Clone)]
pub struct JobScheduler {
    db: Database,
    jobs: Arc<RwLock<Vec<ScheduledJob>>>,
}

impl JobScheduler {
    pub fn new(db: Database) -> Result<Self> {
        Ok(Self {
            db,
            jobs: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    /// Initialize default scheduled jobs
    pub async fn initialize_default_jobs(&self) -> Result<()> {
        let default_jobs = vec![
            ScheduledJob {
                id: 1,
                name: "RssSync".to_string(),
                interval_minutes: 15,
                command: JobCommand::RssSync,
                enabled: true,
                last_execution: None,
                next_execution: None,
            },
            ScheduledJob {
                id: 2,
                name: "RefreshSeries".to_string(),
                interval_minutes: 360, // 6 hours
                command: JobCommand::RefreshSeries,
                enabled: true,
                last_execution: None,
                next_execution: None,
            },
            ScheduledJob {
                id: 3,
                name: "DownloadedEpisodesScan".to_string(),
                interval_minutes: 0, // On demand only
                command: JobCommand::DownloadedEpisodesScan,
                enabled: true,
                last_execution: None,
                next_execution: None,
            },
            ScheduledJob {
                id: 4,
                name: "Housekeeping".to_string(),
                interval_minutes: 1440, // Daily
                command: JobCommand::Housekeeping,
                enabled: true,
                last_execution: None,
                next_execution: None,
            },
            ScheduledJob {
                id: 5,
                name: "HealthCheck".to_string(),
                interval_minutes: 5,
                command: JobCommand::HealthCheck,
                enabled: true,
                last_execution: None,
                next_execution: None,
            },
            ScheduledJob {
                id: 6,
                name: "Backup".to_string(),
                interval_minutes: 10080, // Weekly
                command: JobCommand::Backup,
                enabled: true,
                last_execution: None,
                next_execution: None,
            },
            ScheduledJob {
                id: 9,
                name: "ProcessDownloadQueue".to_string(),
                interval_minutes: 1, // Every minute
                command: JobCommand::ProcessDownloadQueue,
                enabled: true,
                last_execution: None,
                next_execution: None,
            },
        ];
        
        let mut jobs = self.jobs.write().await;
        *jobs = default_jobs;
        
        info!("Initialized {} scheduled jobs", jobs.len());
        Ok(())
    }
    
    /// Start the scheduler
    pub async fn start(&self) -> Result<()> {
        info!("Starting job scheduler...");
        
        // Spawn task for each enabled job
        let jobs = self.jobs.read().await.clone();
        
        for job in jobs {
            if job.enabled && job.interval_minutes > 0 {
                let db = self.db.clone();
                tokio::spawn(async move {
                    run_job_loop(job, db).await;
                });
            }
        }
        
        Ok(())
    }
    
    /// Get all scheduled jobs
    pub async fn get_jobs(&self) -> Vec<ScheduledJob> {
        self.jobs.read().await.clone()
    }
    
    /// Execute a job immediately
    pub async fn execute_job(&self, job_id: i64) -> Result<()> {
        let jobs = self.jobs.read().await;
        let job = jobs.iter()
            .find(|j| j.id == job_id)
            .context("Job not found")?;
        
        info!("Executing job: {}", job.name);
        execute_job_command(&job.command, &self.db).await?;
        
        Ok(())
    }
    
    /// Enable/disable a job
    pub async fn set_job_enabled(&self, job_id: i64, enabled: bool) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.enabled = enabled;
            info!("Job '{}' enabled: {}", job.name, enabled);
        }
        Ok(())
    }
}

/// Run a job in a loop with its interval
async fn run_job_loop(job: ScheduledJob, db: Database) {
    let interval = tokio::time::Duration::from_secs(job.interval_minutes as u64 * 60);
    let mut interval_timer = tokio::time::interval(interval);
    
    info!("Started job loop for: {} (every {} minutes)", job.name, job.interval_minutes);
    
    loop {
        interval_timer.tick().await;
        
        if let Err(e) = execute_job_command(&job.command, &db).await {
            error!("Job '{}' failed: {}", job.name, e);
        }
    }
}

/// Execute a job command
async fn execute_job_command(command: &JobCommand, db: &Database) -> Result<()> {
    match command {
        JobCommand::RssSync => {
            execute_rss_sync(db).await?;
        }
        JobCommand::RefreshSeries => {
            execute_refresh_series(db).await?;
        }
        JobCommand::DownloadedEpisodesScan => {
            execute_downloaded_episodes_scan(db).await?;
        }
        JobCommand::Housekeeping => {
            execute_housekeeping(db).await?;
        }
        JobCommand::HealthCheck => {
            execute_health_check(db).await?;
        }
        JobCommand::Backup => {
            execute_backup(db).await?;
        }
        JobCommand::ProcessDownloadQueue => {
            execute_process_download_queue(db).await?;
        }
        JobCommand::Custom { name, action: _ } => {
            info!("Executing custom job: {}", name);
            // Custom actions would be implemented based on the action string
        }
    }

    Ok(())
}

// ============================================================================
// Job Implementations
// ============================================================================

/// RSS Sync: Fetch RSS feeds from all enabled indexers and process new releases
async fn execute_rss_sync(db: &Database) -> Result<()> {
    info!("Executing RSS sync...");

    // Get all enabled indexers
    let indexer_repo = IndexerRepository::new(db.clone());
    let indexers = indexer_repo.get_all().await?;

    let enabled_count = indexers.iter().filter(|i| i.enable_rss).count();
    if enabled_count == 0 {
        info!("No indexers with RSS enabled, skipping");
        return Ok(());
    }

    info!("Syncing RSS from {} indexers", enabled_count);

    // Create RSS sync service and fetch releases
    let mut rss_service = RssSyncService::new(indexers);
    let releases = rss_service.sync().await?;

    info!("RSS sync found {} releases", releases.len());

    // TODO: Process releases against wanted episodes
    // - Match releases to series/episodes
    // - Check quality profiles
    // - Add approved releases to download queue

    Ok(())
}

/// Refresh Series: Update metadata for all series that need it
async fn execute_refresh_series(db: &Database) -> Result<()> {
    info!("Executing series refresh...");

    let series_repo = SeriesRepository::new(db.clone());
    let all_series = series_repo.get_all().await?;

    let mut refreshed = 0;
    let mut errors = 0;
    let refresh_threshold = chrono::Duration::hours(12);

    for series in all_series {
        // Check if series needs refresh (older than 12 hours)
        let needs_refresh = match series.last_info_sync {
            None => true,
            Some(last_sync) => chrono::Utc::now() - last_sync > refresh_threshold,
        };

        if !needs_refresh {
            continue;
        }

        info!("Refreshing series: {} (TVDB: {})", series.title, series.tvdb_id);

        // Fetch updated metadata from Skyhook
        let url = format!("http://skyhook.sonarr.tv/v1/tvdb/shows/en/{}", series.tvdb_id);
        let client = reqwest::Client::new();

        match client.get(&url)
            .header("User-Agent", "pir9/0.1.0")
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                // Update series in database
                let mut updated_series = series.clone();
                updated_series.last_info_sync = Some(chrono::Utc::now());

                if let Err(e) = series_repo.update(&updated_series).await {
                    error!("Failed to update series {}: {}", series.title, e);
                    errors += 1;
                } else {
                    refreshed += 1;
                }
            }
            Ok(response) => {
                warn!("Skyhook returned {} for series {}", response.status(), series.title);
                errors += 1;
            }
            Err(e) => {
                warn!("Failed to fetch metadata for series {}: {}", series.title, e);
                errors += 1;
            }
        }
    }

    info!("Series refresh complete: {} refreshed, {} errors", refreshed, errors);
    Ok(())
}

/// Process Download Queue: Update tracked download statuses and trigger imports
async fn execute_process_download_queue(db: &Database) -> Result<()> {
    // Use TrackedDownloadService to process the queue
    let service = crate::core::queue::TrackedDownloadService::new(db.clone());

    match service.process_queue().await {
        Ok(()) => {
            // Silently succeed - this runs every minute, don't spam logs
        }
        Err(e) => {
            warn!("Failed to process download queue: {}", e);
        }
    }

    Ok(())
}

/// Downloaded Episodes Scan: Check download clients for completed downloads and import them
async fn execute_downloaded_episodes_scan(db: &Database) -> Result<()> {
    info!("Executing downloaded episodes scan...");

    let import_service = ImportService::new(db.clone());
    let results = import_service.process_completed_downloads(true).await?;

    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.len() - succeeded;

    if !results.is_empty() {
        info!(
            "Downloaded episodes scan complete: {} imported, {} failed",
            succeeded, failed
        );
    }

    Ok(())
}

/// Housekeeping: Clean up old data and maintain database health
async fn execute_housekeeping(db: &Database) -> Result<()> {
    info!("Executing housekeeping...");

    let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);
    let pool = db.pool();

    // Clean up old command history (older than 30 days)
    let result = sqlx::query(
        "DELETE FROM commands WHERE started_at < $1 AND status IN ('completed', 'failed')"
    )
    .bind(thirty_days_ago)
    .execute(pool)
    .await;

    if let Ok(r) = result {
        if r.rows_affected() > 0 {
            info!("Cleaned up {} old command records", r.rows_affected());
        }
    }

    // PostgreSQL: Run VACUUM ANALYZE for optimization
    info!("Running database optimization...");
    let _ = sqlx::query("VACUUM ANALYZE").execute(pool).await;

    info!("Housekeeping complete");
    Ok(())
}

/// Health Check: Verify all services are working
async fn execute_health_check(db: &Database) -> Result<()> {
    info!("Executing health check...");

    let mut all_healthy = true;

    // Check download clients
    let client_repo = DownloadClientRepository::new(db.clone());
    let clients = client_repo.get_all().await?;

    for client_model in clients {
        if !client_model.enable {
            continue;
        }

        match create_download_client(&client_model) {
            Ok(client) => {
                match client.test().await {
                    Ok(()) => {
                        info!("✓ Download client '{}' is healthy", client_model.name);
                    }
                    Err(e) => {
                        error!("✗ Download client '{}' failed: {}", client_model.name, e);
                        all_healthy = false;
                    }
                }
            }
            Err(e) => {
                error!("✗ Failed to create client '{}': {}", client_model.name, e);
                all_healthy = false;
            }
        }
    }

    // Check indexers
    let indexer_repo = IndexerRepository::new(db.clone());
    let indexers = indexer_repo.get_all().await?;

    for indexer in indexers {
        if !indexer.enable_rss && !indexer.enable_automatic_search {
            continue;
        }

        match create_indexer_client(&indexer) {
            Ok(client) => {
                // Try a minimal RSS fetch as a health check
                match client.fetch_rss(Some(1)).await {
                    Ok(_) => {
                        info!("✓ Indexer '{}' is healthy", indexer.name);
                    }
                    Err(e) => {
                        warn!("✗ Indexer '{}' check failed: {}", indexer.name, e);
                        all_healthy = false;
                    }
                }
            }
            Err(e) => {
                warn!("✗ Failed to create indexer client '{}': {}", indexer.name, e);
                all_healthy = false;
            }
        }
    }

    // Check disk space for root folders
    let root_folder_repo = RootFolderRepository::new(db.clone());
    if let Ok(folders) = root_folder_repo.get_all().await {
        for folder in &folders {
            let c_path = std::ffi::CString::new(folder.path.as_str()).ok();
            if let Some(c_path) = c_path {
                let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
                let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
                if ret == 0 {
                    let free_bytes = stat.f_bavail as u64 * stat.f_frsize as u64;
                    let total_bytes = stat.f_blocks as u64 * stat.f_frsize as u64;
                    // Warn if less than 2% free
                    if total_bytes > 0 && (free_bytes * 100 / total_bytes) < 2 {
                        warn!(
                            "Low disk space on '{}': {} MB free of {} MB total",
                            folder.path,
                            free_bytes / 1_048_576,
                            total_bytes / 1_048_576
                        );
                        all_healthy = false;
                    }
                }
            }
        }
    }

    if all_healthy {
        info!("Health check passed: all services healthy");
    } else {
        warn!("Health check completed with warnings");
    }

    Ok(())
}

/// Backup: Create a PostgreSQL database backup using pg_dump
async fn execute_backup(_db: &Database) -> Result<()> {
    info!("Executing backup...");

    let db_url = std::env::var("PIR9_DB_CONNECTION")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgresql://pir9:pir9@localhost:5432/pir9".to_string());

    // Create backup directory
    let backup_dir = std::env::var("PIR9_BACKUP_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/config/Backups"));
    tokio::fs::create_dir_all(&backup_dir).await
        .context("Failed to create backup directory")?;

    // Create timestamped backup filename
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("pir9_backup_{}.sql", timestamp);
    let backup_path = backup_dir.join(&backup_filename);

    info!("Creating backup: {}", backup_path.display());

    let output = tokio::process::Command::new("pg_dump")
        .arg(&db_url)
        .arg("--file")
        .arg(&backup_path)
        .arg("--format=plain")
        .arg("--no-owner")
        .arg("--no-acl")
        .output()
        .await
        .context("Failed to run pg_dump")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("pg_dump failed: {}", stderr);
    }

    // Clean up old backups (keep last 7)
    let mut dir_entries = tokio::fs::read_dir(&backup_dir).await?;
    let mut backups = Vec::new();

    while let Some(entry) = dir_entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "sql") {
            backups.push(entry);
        }
    }

    backups.sort_by_key(|e| std::cmp::Reverse(e.file_name()));

    for old_backup in backups.into_iter().skip(7) {
        let path = old_backup.path();
        info!("Removing old backup: {}", path.display());
        let _ = tokio::fs::remove_file(path).await;
    }

    info!("Backup complete: {}", backup_filename);
    Ok(())
}

/// Scheduled job definition
#[derive(Debug, Clone)]
pub struct ScheduledJob {
    pub id: i64,
    pub name: String,
    pub interval_minutes: i64,
    pub command: JobCommand,
    pub enabled: bool,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
    pub next_execution: Option<chrono::DateTime<chrono::Utc>>,
}

/// Job command types
#[derive(Debug, Clone)]
pub enum JobCommand {
    RssSync,
    RefreshSeries,
    DownloadedEpisodesScan,
    Housekeeping,
    HealthCheck,
    Backup,
    /// Process download queue (update statuses, trigger imports)
    ProcessDownloadQueue,
    Custom { name: String, action: String },
}
