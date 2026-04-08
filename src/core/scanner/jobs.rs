#![allow(dead_code, unused_imports)]
//! Scan job tracking with timeout and retry support
//!
//! Tracks pending scan jobs and handles:
//! - Job timeouts (no result received within timeout period)
//! - Retry logic with exponential backoff
//! - Fallback to local scanning after max retries

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::core::datastore::Database;
use crate::core::messaging::{HybridEventBus, Message, ScanType};

/// Default timeout for scan jobs (5 minutes)
pub const DEFAULT_JOB_TIMEOUT: Duration = Duration::from_secs(300);

/// Maximum number of retries before giving up
pub const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (doubles each retry)
pub const RETRY_BASE_DELAY: Duration = Duration::from_secs(5);

/// State of a scan job
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    /// Waiting for worker to pick up
    Pending,
    /// Worker acknowledged, scanning in progress
    InProgress,
    /// Completed successfully
    Completed,
    /// Failed (timeout or error)
    Failed,
    /// Retrying after failure
    Retrying,
}

/// A tracked scan job
#[derive(Debug, Clone)]
pub struct ScanJob {
    /// Unique job ID
    pub job_id: String,
    /// Series IDs to scan
    pub series_ids: Vec<i64>,
    /// Paths to scan
    pub paths: Vec<String>,
    /// Scan type
    pub scan_type: ScanType,
    /// Current state
    pub state: JobState,
    /// When the job was created
    pub created_at: Instant,
    /// When the job was last updated
    pub updated_at: Instant,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Worker that picked up the job (if any)
    pub assigned_worker: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Results received (for multi-series scans)
    pub results_received: usize,
    /// Total results expected
    pub results_expected: usize,
}

impl ScanJob {
    /// Create a new pending job
    pub fn new(
        job_id: String,
        series_ids: Vec<i64>,
        paths: Vec<String>,
        scan_type: ScanType,
    ) -> Self {
        let now = Instant::now();
        let results_expected = paths.len();
        Self {
            job_id,
            series_ids,
            paths,
            scan_type,
            state: JobState::Pending,
            created_at: now,
            updated_at: now,
            retry_count: 0,
            assigned_worker: None,
            error: None,
            results_received: 0,
            results_expected,
        }
    }

    /// Check if job has timed out
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        matches!(self.state, JobState::Pending | JobState::InProgress)
            && self.updated_at.elapsed() > timeout
    }

    /// Check if job can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < MAX_RETRIES
    }

    /// Calculate delay before next retry (exponential backoff)
    pub fn retry_delay(&self) -> Duration {
        RETRY_BASE_DELAY * 2u32.pow(self.retry_count.min(5))
    }

    /// Mark as in progress
    pub fn mark_in_progress(&mut self, worker_id: &str) {
        self.state = JobState::InProgress;
        self.assigned_worker = Some(worker_id.to_string());
        self.updated_at = Instant::now();
    }

    /// Record a result received
    pub fn record_result(&mut self) {
        self.results_received += 1;
        self.updated_at = Instant::now();
        if self.results_received >= self.results_expected {
            self.state = JobState::Completed;
        }
    }

    /// Mark as failed
    pub fn mark_failed(&mut self, error: &str) {
        self.state = JobState::Failed;
        self.error = Some(error.to_string());
        self.updated_at = Instant::now();
    }

    /// Prepare for retry
    pub fn prepare_retry(&mut self) {
        self.retry_count += 1;
        self.state = JobState::Retrying;
        self.results_received = 0;
        self.assigned_worker = None;
        self.error = None;
        self.updated_at = Instant::now();
    }
}

/// Manages scan jobs with timeout and retry support
pub struct JobTracker {
    /// Active jobs by ID
    jobs: HashMap<String, ScanJob>,
    /// Job timeout duration
    timeout: Duration,
}

impl JobTracker {
    /// Create a new job tracker
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
            timeout: DEFAULT_JOB_TIMEOUT,
        }
    }

    /// Create with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            jobs: HashMap::new(),
            timeout,
        }
    }

    /// Add a new job
    pub fn add_job(&mut self, job: ScanJob) {
        debug!("Tracking job: {} ({} paths)", job.job_id, job.paths.len());
        self.jobs.insert(job.job_id.clone(), job);
    }

    /// Get a job by ID
    pub fn get_job(&self, job_id: &str) -> Option<&ScanJob> {
        self.jobs.get(job_id)
    }

    /// Get a mutable job by ID
    pub fn get_job_mut(&mut self, job_id: &str) -> Option<&mut ScanJob> {
        self.jobs.get_mut(job_id)
    }

    /// Mark job as in progress by a worker
    pub fn mark_in_progress(&mut self, job_id: &str, worker_id: &str) {
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.mark_in_progress(worker_id);
            debug!("Job {} picked up by worker {}", job_id, worker_id);
        }
    }

    /// Record a result for a job
    pub fn record_result(&mut self, job_id: &str) -> Option<JobState> {
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.record_result();
            debug!(
                "Job {} received result {}/{}",
                job_id, job.results_received, job.results_expected
            );
            Some(job.state.clone())
        } else {
            None
        }
    }

    /// Check for timed out jobs and return them
    pub fn check_timeouts(&mut self) -> Vec<ScanJob> {
        let mut timed_out = Vec::new();

        for job in self.jobs.values_mut() {
            if job.is_timed_out(self.timeout) {
                warn!(
                    "Job {} timed out (last update: {:?} ago)",
                    job.job_id,
                    job.updated_at.elapsed()
                );
                job.mark_failed("Timeout - no response from workers");
                timed_out.push(job.clone());
            }
        }

        timed_out
    }

    /// Get jobs that need retry
    pub fn get_jobs_for_retry(&mut self) -> Vec<ScanJob> {
        let mut to_retry = Vec::new();

        for job in self.jobs.values_mut() {
            if job.state == JobState::Failed && job.can_retry() {
                job.prepare_retry();
                to_retry.push(job.clone());
                info!(
                    "Preparing job {} for retry (attempt {})",
                    job.job_id, job.retry_count
                );
            }
        }

        to_retry
    }

    /// Remove completed/failed jobs older than max_age
    pub fn cleanup(&mut self, max_age: Duration) {
        self.jobs.retain(|id, job| {
            if matches!(job.state, JobState::Completed | JobState::Failed)
                && job.updated_at.elapsed() > max_age
            {
                debug!("Cleaning up old job: {}", id);
                return false;
            }
            true
        });
    }

    /// Get count of jobs in each state
    pub fn stats(&self) -> JobStats {
        let mut stats = JobStats::default();
        for job in self.jobs.values() {
            match job.state {
                JobState::Pending => stats.pending += 1,
                JobState::InProgress => stats.in_progress += 1,
                JobState::Completed => stats.completed += 1,
                JobState::Failed => stats.failed += 1,
                JobState::Retrying => stats.retrying += 1,
            }
        }
        stats
    }
}

impl Default for JobTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about tracked jobs
#[derive(Debug, Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStats {
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub failed: usize,
    pub retrying: usize,
}

/// Service that manages job tracking with automatic timeout/retry
pub struct JobTrackerService {
    tracker: Arc<RwLock<JobTracker>>,
    db: Database,
    event_bus: HybridEventBus,
}

impl JobTrackerService {
    /// Create a new job tracker service
    pub fn new(db: Database, event_bus: HybridEventBus) -> Self {
        Self {
            tracker: Arc::new(RwLock::new(JobTracker::new())),
            db,
            event_bus,
        }
    }

    /// Get a reference to the tracker
    pub fn tracker(&self) -> Arc<RwLock<JobTracker>> {
        self.tracker.clone()
    }

    /// Run the job tracker service
    ///
    /// Periodically checks for timeouts and triggers retries.
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        info!("Starting job tracker service");

        // Check interval (every 30 seconds)
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            // Check for timed out jobs
            let timed_out = {
                let mut tracker = self.tracker.write().await;
                tracker.check_timeouts()
            };

            // Handle timed out jobs
            for job in timed_out {
                if job.can_retry() {
                    self.retry_job(&job).await;
                } else {
                    self.fallback_to_local(&job).await;
                }
            }

            // Check for jobs ready to retry
            let to_retry = {
                let mut tracker = self.tracker.write().await;
                tracker.get_jobs_for_retry()
            };

            for job in to_retry {
                self.dispatch_job(&job).await;
            }

            // Cleanup old completed/failed jobs (older than 1 hour)
            {
                let mut tracker = self.tracker.write().await;
                tracker.cleanup(Duration::from_secs(3600));
            }
        }
    }

    /// Dispatch a job to workers
    async fn dispatch_job(&self, job: &ScanJob) {
        info!(
            "Dispatching job {} (attempt {})",
            job.job_id,
            job.retry_count + 1
        );

        let message = Message::ScanRequest {
            job_id: job.job_id.clone(),
            scan_type: job.scan_type.clone(),
            series_ids: job.series_ids.clone(),
            paths: job.paths.clone(),
            known_files: std::collections::HashMap::new(),
        };

        self.event_bus.enqueue_job(message).await;

        // Mark as pending in tracker
        let mut tracker = self.tracker.write().await;
        if let Some(tracked_job) = tracker.get_job_mut(&job.job_id) {
            tracked_job.state = JobState::Pending;
            tracked_job.updated_at = Instant::now();
        }
    }

    /// Retry a failed job
    async fn retry_job(&self, job: &ScanJob) {
        let delay = job.retry_delay();
        info!(
            "Retrying job {} in {:?} (attempt {})",
            job.job_id,
            delay,
            job.retry_count + 1
        );

        // Wait for retry delay
        tokio::time::sleep(delay).await;

        // Dispatch again
        self.dispatch_job(job).await;
    }

    /// Fall back to local scanning after exhausting retries
    async fn fallback_to_local(&self, job: &ScanJob) {
        warn!(
            "Job {} exhausted retries ({}), falling back to local scan",
            job.job_id, job.retry_count
        );

        // Execute local scan
        match self.execute_local_scan(job).await {
            Ok(()) => {
                info!("Local fallback scan completed for job {}", job.job_id);
                let mut tracker = self.tracker.write().await;
                if let Some(tracked_job) = tracker.get_job_mut(&job.job_id) {
                    tracked_job.state = JobState::Completed;
                    tracked_job.updated_at = Instant::now();
                }
            }
            Err(e) => {
                error!("Local fallback scan failed for job {}: {}", job.job_id, e);
            }
        }
    }

    /// Execute a scan locally (fallback)
    async fn execute_local_scan(&self, job: &ScanJob) -> anyhow::Result<()> {
        use crate::core::datastore::models::EpisodeFileDbModel;
        use crate::core::datastore::repositories::{
            EpisodeFileRepository, EpisodeRepository, SeriesRepository,
        };
        use crate::core::mediafiles::{
            compute_file_hash, derive_quality_from_media, MediaAnalyzer,
        };
        use crate::core::scanner;
        use chrono::Utc;
        use std::path::Path;

        let series_repo = SeriesRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let episode_file_repo = EpisodeFileRepository::new(self.db.clone());

        for series_id in &job.series_ids {
            let series = match series_repo.get_by_id(*series_id).await? {
                Some(s) => s,
                None => continue,
            };

            let series_path = Path::new(&series.path);
            if !series_path.exists() {
                continue;
            }

            let episodes = episode_repo.get_by_series_id(*series_id).await?;
            let files = scanner::scan_series_directory(series_path);

            for file in files {
                let file_path_str = file.path.to_string_lossy().to_string();

                // Check if file exists
                if episode_file_repo
                    .get_by_path(&file_path_str)
                    .await?
                    .is_some()
                {
                    continue;
                }

                let season_number = file.season_number.unwrap_or(1);
                let relative_path = file
                    .path
                    .strip_prefix(&series.path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file.filename.clone());

                let languages_json = serde_json::json!([{"id": 1, "name": "English"}]);

                // Real media analysis via FFmpeg probe
                let media_info_result = MediaAnalyzer::analyze(Path::new(&file_path_str)).await;
                let media_info = media_info_result
                    .as_ref()
                    .ok()
                    .and_then(|info| serde_json::to_string(info).ok());

                // Quality derived from actual resolution
                let quality_json = match &media_info_result {
                    Ok(info) => derive_quality_from_media(info, &file.filename),
                    Err(_) => serde_json::json!({
                        "quality": {"id": 1, "name": "SDTV", "source": "unknown", "resolution": 0},
                        "revision": {"version": 1, "real": 0, "isRepack": false}
                    }),
                };

                // BLAKE3 content hash
                let file_hash = compute_file_hash(Path::new(&file_path_str)).await.ok();

                let episode_file = EpisodeFileDbModel {
                    id: 0,
                    series_id: *series_id,
                    season_number,
                    relative_path,
                    path: file_path_str.clone(),
                    size: file.size,
                    date_added: Utc::now(),
                    scene_name: Some(file.filename.clone()),
                    release_group: file.release_group.clone(),
                    quality: quality_json.to_string().into(),
                    languages: languages_json.to_string().into(),
                    media_info: media_info.map(Into::into),
                    original_file_path: Some(file_path_str.clone()),
                    file_hash,
                };

                let file_id = episode_file_repo.insert(&episode_file).await?;

                // Link episodes
                for ep_num in &file.episode_numbers {
                    if let Some(mut ep) = episodes
                        .iter()
                        .find(|e| e.season_number == season_number && e.episode_number == *ep_num)
                        .cloned()
                    {
                        ep.has_file = true;
                        ep.episode_file_id = Some(file_id);
                        episode_repo.update(&ep).await?;
                    }
                }
            }
        }

        Ok(())
    }
}
