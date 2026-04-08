#![allow(dead_code, unused_imports)]
//! Job scheduler module
//! Handles periodic tasks like RSS sync, library refresh, etc.

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeRepository, IndexerRepository, QualityProfileRepository,
    RootFolderRepository, SeriesRepository,
};
use crate::core::datastore::Database;
use crate::core::download::clients::create_client_from_model as create_download_client;
use crate::core::download::import::ImportService;
use crate::core::indexers::create_client_from_model as create_indexer_client;
use crate::core::indexers::rss::RssSyncService;
use crate::core::parser::{best_series_match, parse_title};
use crate::core::profiles::QualityProfileItem;
use crate::core::queue::service::TrackedDownloadService;

/// Job scheduler for managing background tasks
#[derive(Debug, Clone)]
pub struct JobScheduler {
    db: Database,
    jobs: Arc<RwLock<Vec<ScheduledJob>>>,
    metadata_service: Option<crate::core::metadata::MetadataService>,
    media_config: Option<crate::core::configuration::MediaConfig>,
    /// Hybrid event bus for distributed scanning (set after AppState creation)
    hybrid_event_bus: Arc<tokio::sync::OnceCell<crate::core::messaging::HybridEventBus>>,
    /// Scan result consumer for registering download imports (set after consumer creation)
    scan_result_consumer: Arc<tokio::sync::OnceCell<Arc<crate::core::scanner::ScanResultConsumer>>>,
    /// Tracked downloads stores (set after load_or_migrate)
    tracked: Arc<tokio::sync::OnceCell<Arc<crate::core::queue::TrackedDownloads>>>,
}

impl JobScheduler {
    pub fn new(db: Database) -> Result<Self> {
        Ok(Self {
            db,
            jobs: Arc::new(RwLock::new(Vec::new())),
            metadata_service: None,
            media_config: None,
            hybrid_event_bus: Arc::new(tokio::sync::OnceCell::new()),
            scan_result_consumer: Arc::new(tokio::sync::OnceCell::new()),
            tracked: Arc::new(tokio::sync::OnceCell::new()),
        })
    }

    /// Set the metadata service for IMDB-enriched refreshes
    pub fn set_metadata_service(&mut self, service: crate::core::metadata::MetadataService) {
        self.metadata_service = Some(service);
    }

    /// Set the media configuration for episode naming during imports
    pub fn set_media_config(&mut self, config: crate::core::configuration::MediaConfig) {
        self.media_config = Some(config);
    }

    /// Set the hybrid event bus for distributed scanning (late binding via OnceCell)
    pub fn set_hybrid_event_bus(&self, bus: crate::core::messaging::HybridEventBus) {
        let _ = self.hybrid_event_bus.set(bus);
    }

    /// Set the scan result consumer for registering download imports (late binding via OnceCell)
    pub fn set_scan_result_consumer(
        &self,
        consumer: Arc<crate::core::scanner::ScanResultConsumer>,
    ) {
        let _ = self.scan_result_consumer.set(consumer);
    }

    /// Set the tracked downloads store (late binding via OnceCell)
    pub fn set_tracked(&self, tracked: Arc<crate::core::queue::TrackedDownloads>) {
        let _ = self.tracked.set(tracked);
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
            ScheduledJob {
                id: 10,
                name: "ReconcileDownloads".to_string(),
                interval_minutes: 5, // Every 5 minutes — discover externally-added downloads
                command: JobCommand::ReconcileDownloads,
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
                let metadata_service = self.metadata_service.clone();
                let media_config = self.media_config.clone();
                let hybrid_event_bus = self.hybrid_event_bus.clone();
                let scan_result_consumer = self.scan_result_consumer.clone();
                let tracked = self.tracked.clone();
                tokio::spawn(async move {
                    run_job_loop(
                        job,
                        db,
                        metadata_service,
                        media_config,
                        hybrid_event_bus,
                        scan_result_consumer,
                        tracked,
                    )
                    .await;
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
        let job = jobs
            .iter()
            .find(|j| j.id == job_id)
            .context("Job not found")?;

        info!("Executing job: {}", job.name);
        execute_job_command(
            &job.command,
            &self.db,
            self.metadata_service.as_ref(),
            self.media_config.as_ref(),
            self.hybrid_event_bus.get(),
            self.scan_result_consumer.get(),
            self.tracked.get(),
        )
        .await?;

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
async fn run_job_loop(
    job: ScheduledJob,
    db: Database,
    metadata_service: Option<crate::core::metadata::MetadataService>,
    media_config: Option<crate::core::configuration::MediaConfig>,
    hybrid_event_bus: Arc<tokio::sync::OnceCell<crate::core::messaging::HybridEventBus>>,
    scan_result_consumer: Arc<tokio::sync::OnceCell<Arc<crate::core::scanner::ScanResultConsumer>>>,
    tracked: Arc<tokio::sync::OnceCell<Arc<crate::core::queue::TrackedDownloads>>>,
) {
    let interval = tokio::time::Duration::from_secs(job.interval_minutes as u64 * 60);
    let mut interval_timer = tokio::time::interval(interval);

    info!(
        "Started job loop for: {} (every {} minutes)",
        job.name, job.interval_minutes
    );

    loop {
        interval_timer.tick().await;

        if let Err(e) = execute_job_command(
            &job.command,
            &db,
            metadata_service.as_ref(),
            media_config.as_ref(),
            hybrid_event_bus.get(),
            scan_result_consumer.get(),
            tracked.get(),
        )
        .await
        {
            error!("Job '{}' failed: {}", job.name, e);
        }
    }
}

/// Execute a job command
async fn execute_job_command(
    command: &JobCommand,
    db: &Database,
    metadata_service: Option<&crate::core::metadata::MetadataService>,
    media_config: Option<&crate::core::configuration::MediaConfig>,
    hybrid_event_bus: Option<&crate::core::messaging::HybridEventBus>,
    scan_result_consumer: Option<&Arc<crate::core::scanner::ScanResultConsumer>>,
    tracked: Option<&Arc<crate::core::queue::TrackedDownloads>>,
) -> Result<()> {
    match command {
        JobCommand::RssSync => {
            execute_rss_sync(db, tracked).await?;
        }
        JobCommand::RefreshSeries => {
            execute_refresh_series(db, metadata_service).await?;
        }
        JobCommand::DownloadedEpisodesScan => {
            execute_downloaded_episodes_scan(
                db,
                media_config,
                hybrid_event_bus,
                scan_result_consumer,
            )
            .await?;
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
            execute_process_download_queue(db, tracked).await?;
        }
        JobCommand::ReconcileDownloads => {
            execute_reconcile_downloads(db, tracked).await?;
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

/// RSS Sync: Fetch RSS feeds from all enabled indexers and auto-grab wanted releases
async fn execute_rss_sync(
    db: &Database,
    tracked: Option<&Arc<crate::core::queue::TrackedDownloads>>,
) -> Result<()> {
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
    if releases.is_empty() {
        return Ok(());
    }

    // Load all monitored series and their quality profiles
    let series_repo = SeriesRepository::new(db.clone());
    let episode_repo = EpisodeRepository::new(db.clone());
    let quality_repo = QualityProfileRepository::new(db.clone());
    let tracked_store = tracked
        .cloned()
        .unwrap_or_else(|| Arc::new(crate::core::queue::TrackedDownloads::empty()));
    let tracked_service = TrackedDownloadService::new(db.clone(), tracked_store.clone());

    let all_series = series_repo.get_all().await?;
    let monitored_series: Vec<_> = all_series.into_iter().filter(|s| s.monitored).collect();
    if monitored_series.is_empty() {
        info!("No monitored series, skipping release matching");
        return Ok(());
    }

    // Pre-load quality profiles into a map for fast lookup
    let all_profiles = quality_repo.get_all().await?;
    let profiles: std::collections::HashMap<i64, _> =
        all_profiles.into_iter().map(|p| (p.id, p)).collect();

    // Get currently downloading episode IDs to avoid duplicate grabs
    let active_downloads = tracked_store.get_all_any().await;
    let downloading_episode_ids: std::collections::HashSet<i64> = active_downloads
        .iter()
        .flat_map(|d| d.episode_ids.iter().copied())
        .collect();

    let mut grabbed = 0u32;
    let mut rejected = 0u32;

    for mut release in releases {
        // 1. Parse the release title
        let parsed = match parse_title(&release.title) {
            Some(p) if p.season_number.is_some() && !p.episode_numbers.is_empty() => p,
            _ => {
                rejected += 1;
                continue;
            }
        };

        // 2. Match to a monitored series
        let series_idx = match best_series_match(&parsed, &monitored_series) {
            Some(idx) => idx,
            None => {
                rejected += 1;
                continue;
            }
        };
        let series = &monitored_series[series_idx];

        // 3. Get the quality profile for this series
        let profile = match profiles.get(&series.quality_profile_id) {
            Some(p) => p,
            None => {
                warn!(
                    "RSS: quality profile {} not found for series '{}'",
                    series.quality_profile_id, series.title
                );
                rejected += 1;
                continue;
            }
        };

        // Parse profile items from JSON
        let profile_items: Vec<QualityProfileItem> =
            serde_json::from_str(&profile.items).unwrap_or_default();

        // A profile with cutoff=0 and only "Unknown" allowed means "accept any quality"
        let accept_any = profile.cutoff == 0
            && profile_items
                .iter()
                .all(|item| !item.allowed || item.quality.id == 0);

        // 4. Check if the release quality is allowed by the profile
        let release_weight = release.quality.quality.weight();
        if !accept_any {
            let is_quality_allowed = profile_items.iter().any(|item| {
                item.allowed
                    && (item.quality.id == release_weight
                        || item.items.iter().any(|q| q.id == release_weight))
            });

            if !is_quality_allowed {
                rejected += 1;
                continue;
            }

            // 5. Check quality meets cutoff (cutoff is stored as quality weight/ID)
            if release_weight < profile.cutoff {
                rejected += 1;
                continue;
            }
        }

        // 6. Find matching wanted episodes
        let season = parsed.season_number.unwrap();
        let mut episode_ids = Vec::new();

        for &ep_num in &parsed.episode_numbers {
            if let Ok(Some(ep)) = episode_repo
                .get_by_series_season_episode(series.id, season, ep_num)
                .await
            {
                // Episode must be monitored, missing, and already aired
                if ep.monitored
                    && !ep.has_file
                    && ep.air_date_utc.is_some_and(|d| d < chrono::Utc::now())
                    && !downloading_episode_ids.contains(&ep.id)
                {
                    episode_ids.push(ep.id);
                }
            }
        }

        if episode_ids.is_empty() {
            rejected += 1;
            continue;
        }

        // 7. Grab the release
        release.series_id = Some(series.id);
        info!(
            "RSS auto-grab: '{}' → {} S{:02}E{} ({:?})",
            release.title,
            series.title,
            season,
            parsed
                .episode_numbers
                .iter()
                .map(|e| format!("{:02}", e))
                .collect::<Vec<_>>()
                .join("E"),
            release.quality.quality
        );

        match tracked_service
            .grab_release(&release, episode_ids, None, "series")
            .await
        {
            Ok(tracked_id) => {
                grabbed += 1;
                info!("Grabbed successfully (tracked_id={})", tracked_id);
            }
            Err(e) => {
                warn!("Failed to grab '{}': {}", release.title, e);
                rejected += 1;
            }
        }
    }

    info!(
        "RSS sync complete: {} grabbed, {} skipped out of {} releases",
        grabbed,
        rejected,
        grabbed + rejected
    );
    Ok(())
}

/// Refresh Series: Update metadata for all series that need it
async fn execute_refresh_series(
    db: &Database,
    metadata_service: Option<&crate::core::metadata::MetadataService>,
) -> Result<()> {
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

        info!(
            "Refreshing series: {} (TVDB: {})",
            series.title, series.tvdb_id
        );

        // Fetch metadata using MetadataService (IMDB-first) or Skyhook-only fallback
        let metadata = if let Some(svc) = metadata_service {
            svc.fetch_series_metadata(series.tvdb_id, series.imdb_id.as_deref())
                .await
        } else {
            crate::core::metadata::MetadataService::fetch_skyhook_only(series.tvdb_id).await
        };

        match metadata {
            Ok(m) => {
                let mut updated_series = series.clone();
                updated_series.last_info_sync = Some(chrono::Utc::now());

                // Apply merged metadata
                updated_series.overview = m.overview;
                updated_series.status =
                    match m.status.as_deref().map(|s| s.to_lowercase()).as_deref() {
                        Some("continuing") => 0,
                        Some("ended") => 1,
                        Some("upcoming") => 2,
                        _ => updated_series.status,
                    };
                updated_series.network = m.network;
                updated_series.runtime = m.runtime.unwrap_or(updated_series.runtime);
                updated_series.certification = m.certification;
                if let Some(year) = m.year {
                    updated_series.year = year;
                }
                if let Some(ref imdb_id) = m.imdb_id {
                    updated_series.imdb_id = Some(imdb_id.clone());
                }
                updated_series.imdb_rating = m.imdb_rating;
                updated_series.imdb_votes = m.imdb_votes;

                if let Err(e) = series_repo.update(&updated_series).await {
                    error!("Failed to update series {}: {}", series.title, e);
                    errors += 1;
                } else {
                    refreshed += 1;
                }
            }
            Err(e) => {
                warn!(
                    "Failed to fetch metadata for series {}: {}",
                    series.title, e
                );
                errors += 1;
            }
        }
    }

    info!(
        "Series refresh complete: {} refreshed, {} errors",
        refreshed, errors
    );
    Ok(())
}

/// Process Download Queue: Update tracked download statuses and trigger imports
async fn execute_process_download_queue(
    db: &Database,
    tracked: Option<&Arc<crate::core::queue::TrackedDownloads>>,
) -> Result<()> {
    let tracked = tracked
        .cloned()
        .unwrap_or_else(|| Arc::new(crate::core::queue::TrackedDownloads::empty()));
    let service = crate::core::queue::TrackedDownloadService::new(db.clone(), tracked);

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

/// Reconcile Downloads: Discover externally-added downloads in download clients and match
/// them to tracked series/movies so they appear in the activity queue
async fn execute_reconcile_downloads(
    db: &Database,
    tracked: Option<&Arc<crate::core::queue::TrackedDownloads>>,
) -> Result<()> {
    let tracked = tracked
        .cloned()
        .unwrap_or_else(|| Arc::new(crate::core::queue::TrackedDownloads::empty()));
    let service = crate::core::queue::TrackedDownloadService::new(db.clone(), tracked);

    match service.reconcile_downloads().await {
        Ok(count) => {
            if count > 0 {
                info!("Reconciled {} new download(s) from download clients", count);
            }
        }
        Err(e) => {
            warn!("Failed to reconcile downloads: {}", e);
        }
    }

    Ok(())
}

/// Downloaded Episodes Scan: Check download clients for completed downloads and import them.
///
/// When Redis workers are available, dispatches file discovery to workers instead
/// of scanning over NFS — matching the v5 command handler behavior.
async fn execute_downloaded_episodes_scan(
    db: &Database,
    media_config: Option<&crate::core::configuration::MediaConfig>,
    hybrid_event_bus: Option<&crate::core::messaging::HybridEventBus>,
    scan_result_consumer: Option<&Arc<crate::core::scanner::ScanResultConsumer>>,
) -> Result<()> {
    info!("Executing downloaded episodes scan...");

    let import_service = ImportService::new(db.clone(), media_config.cloned().unwrap_or_default());

    // If Redis/workers available, use the distributed path (same as v5 command handler)
    if let (Some(hybrid_bus), Some(consumer)) = (hybrid_event_bus, scan_result_consumer) {
        if hybrid_bus.is_redis_enabled() {
            let pending = import_service.check_for_completed_downloads().await?;
            if pending.is_empty() {
                return Ok(());
            }

            info!(
                "Downloaded episodes scan: dispatching {} completed downloads to workers",
                pending.len()
            );

            // Reuse the same distributed dispatch logic from v5 command handler
            use crate::core::messaging::{Message, ScanType};
            use crate::core::scanner::DownloadImportInfo;

            for item in &pending {
                let job_id = uuid::Uuid::new_v4().to_string();
                let output_path_str = item.output_path.to_string_lossy().to_string();

                let import_info = DownloadImportInfo {
                    download_id: item.download_id.clone(),
                    download_client_id: item.download_client_id,
                    download_client_name: item.download_client_name.clone(),
                    title: item.title.clone(),
                    output_path: item.output_path.clone(),
                    parsed_info: item.parsed_info.clone(),
                    series: item.series.clone(),
                    episodes: item.episodes.clone(),
                    overrides: std::collections::HashMap::new(),
                    force_reimport: std::collections::HashSet::new(),
                    skip_files: std::collections::HashSet::new(),
                    force_import_all: false,
                };

                consumer
                    .register_download_import(&job_id, vec![import_info])
                    .await;
                consumer
                    .register_job(&job_id, ScanType::DownloadedEpisodesScan, vec![0])
                    .await;

                let message = Message::ScanRequest {
                    job_id,
                    scan_type: ScanType::DownloadedEpisodesScan,
                    series_ids: vec![0],
                    paths: vec![output_path_str],
                    known_files: std::collections::HashMap::new(),
                };
                hybrid_bus.enqueue_job(message).await;
            }

            return Ok(());
        }
    }

    // Fallback: local import (no worker available)
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
        "DELETE FROM commands WHERE started_at < $1 AND status IN ('completed', 'failed')",
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
            Ok(client) => match client.test().await {
                Ok(()) => {
                    info!("✓ Download client '{}' is healthy", client_model.name);
                }
                Err(e) => {
                    error!("✗ Download client '{}' failed: {}", client_model.name, e);
                    all_healthy = false;
                }
            },
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
                warn!(
                    "✗ Failed to create indexer client '{}': {}",
                    indexer.name, e
                );
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
    tokio::fs::create_dir_all(&backup_dir)
        .await
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
        if path.extension().is_some_and(|ext| ext == "sql") {
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
    /// Reconcile external downloads with tracked series/movies
    ReconcileDownloads,
    Custom {
        name: String,
        action: String,
    },
}
