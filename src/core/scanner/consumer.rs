//! Scan result consumer service
//!
//! This service runs on the server and processes scan results from distributed workers.
//! It updates the database with discovered files and links them to episodes.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::core::datastore::models::EpisodeFileDbModel;
use crate::core::datastore::repositories::{
    EpisodeFileRepository, EpisodeRepository, SeriesRepository,
};
use crate::core::datastore::Database;
use crate::core::messaging::{HybridEventBus, Message, ScannedFile};

/// Tracks pending scan jobs and their results
#[derive(Debug, Default)]
pub struct PendingScanJobs {
    /// Maps job_id -> (series_id, expected results, received results)
    jobs: HashMap<String, PendingJob>,
}

#[derive(Debug)]
struct PendingJob {
    series_ids: Vec<i64>,
    results_received: usize,
    completed: bool,
}

/// Service that consumes scan results from workers and updates the database
pub struct ScanResultConsumer {
    db: Database,
    event_bus: HybridEventBus,
    pending_jobs: Arc<RwLock<PendingScanJobs>>,
}

impl ScanResultConsumer {
    /// Create a new scan result consumer
    pub fn new(db: Database, event_bus: HybridEventBus) -> Self {
        Self {
            db,
            event_bus,
            pending_jobs: Arc::new(RwLock::new(PendingScanJobs::default())),
        }
    }

    /// Register a pending scan job
    pub async fn register_job(&self, job_id: &str, series_ids: Vec<i64>) {
        let mut jobs = self.pending_jobs.write().await;
        jobs.jobs.insert(
            job_id.to_string(),
            PendingJob {
                series_ids,
                results_received: 0,
                completed: false,
            },
        );
        debug!("Registered scan job: {}", job_id);
    }

    /// Start the consumer loop
    ///
    /// This subscribes to the event bus and processes incoming ScanResult messages.
    /// It runs until the event bus is closed or an error occurs.
    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("Starting scan result consumer");

        let mut receiver = self.event_bus.subscribe();

        loop {
            match receiver.recv().await {
                Ok(message) => {
                    if let Message::ScanResult {
                        job_id,
                        series_id,
                        worker_id,
                        files_found,
                        errors,
                    } = message
                    {
                        self.handle_scan_result(
                            &job_id,
                            series_id,
                            &worker_id,
                            files_found,
                            errors,
                        )
                        .await;
                    }
                    // Ignore other message types
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Scan result consumer lagged by {} messages", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("Event bus closed, shutting down scan result consumer");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a scan result from a worker
    async fn handle_scan_result(
        &self,
        job_id: &str,
        series_id: i64,
        worker_id: &str,
        files_found: Vec<ScannedFile>,
        errors: Vec<String>,
    ) {
        info!(
            "Received scan result: job_id={}, series_id={}, worker={}, files={}, errors={}",
            job_id,
            series_id,
            worker_id,
            files_found.len(),
            errors.len()
        );

        // Log any errors from the worker
        for error in &errors {
            warn!(
                "Worker {} reported error for job {}: {}",
                worker_id, job_id, error
            );
        }

        // Process the files
        if let Err(e) = self.process_scanned_files(series_id, files_found).await {
            error!(
                "Failed to process scanned files for series {}: {}",
                series_id, e
            );
        }

        // Update job tracking
        {
            let mut jobs = self.pending_jobs.write().await;
            if let Some(job) = jobs.jobs.get_mut(job_id) {
                job.results_received += 1;
                debug!("Job {} received result {}", job_id, job.results_received);
            }
        }
    }

    /// Process scanned files and update the database
    async fn process_scanned_files(&self, series_id: i64, files: Vec<ScannedFile>) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        let series_repo = SeriesRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let episode_file_repo = EpisodeFileRepository::new(self.db.clone());

        // Get series info
        let series = series_repo
            .get_by_id(series_id)
            .await
            .context("Failed to fetch series")?
            .ok_or_else(|| anyhow::anyhow!("Series {} not found", series_id))?;

        // Get episodes for this series
        let episodes = episode_repo
            .get_by_series_id(series_id)
            .await
            .context("Failed to fetch episodes")?;

        let mut new_files = 0;
        let mut matched_episodes = 0;

        for file in files {
            let file_path_str = file.path.to_string_lossy().to_string();

            // Skip if we don't have episode info
            if file.episode_numbers.is_empty() {
                debug!("Skipping file without episode info: {}", file.filename);
                continue;
            }

            let season_number = file.season_number.unwrap_or(1);

            // Check if file already exists in database
            let existing = episode_file_repo
                .get_by_path(&file_path_str)
                .await
                .ok()
                .flatten();

            let episode_file_id = if let Some(ef) = existing {
                ef.id
            } else {
                // Calculate relative path
                let relative_path = file
                    .path
                    .strip_prefix(&series.path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file.filename.clone());

                // Create quality JSON (default to HDTV-720p)
                let quality_json = serde_json::json!({
                    "quality": {
                        "id": 4,
                        "name": "HDTV-720p",
                        "source": "television",
                        "resolution": 720
                    },
                    "revision": {
                        "version": 1,
                        "real": 0,
                        "isRepack": false
                    }
                });

                let languages_json = serde_json::json!([{
                    "id": 1,
                    "name": "English"
                }]);

                let episode_file = EpisodeFileDbModel {
                    id: 0,
                    series_id,
                    season_number,
                    relative_path,
                    path: file_path_str.clone(),
                    size: file.size,
                    date_added: Utc::now(),
                    scene_name: Some(file.filename.clone()),
                    release_group: file.release_group.clone(),
                    quality: quality_json.to_string(),
                    languages: languages_json.to_string(),
                    media_info: None,
                    original_file_path: Some(file_path_str.clone()),
                };

                match episode_file_repo.insert(&episode_file).await {
                    Ok(id) => {
                        new_files += 1;
                        debug!("Created episode file: id={}, path={}", id, file_path_str);
                        id
                    }
                    Err(e) => {
                        warn!("Failed to create episode file for {}: {}", file_path_str, e);
                        continue;
                    }
                }
            };

            // Link episodes to file
            for episode_num in &file.episode_numbers {
                if let Some(mut ep) = episodes
                    .iter()
                    .find(|e| e.season_number == season_number && e.episode_number == *episode_num)
                    .cloned()
                {
                    if !ep.has_file || ep.episode_file_id != Some(episode_file_id) {
                        ep.has_file = true;
                        ep.episode_file_id = Some(episode_file_id);
                        if episode_repo.update(&ep).await.is_ok() {
                            matched_episodes += 1;
                            debug!(
                                "Linked S{:02}E{:02} to file {}",
                                season_number, episode_num, episode_file_id
                            );
                        }
                    }
                }
            }

            // Log multi-episode files
            if file.episode_numbers.len() > 1 {
                let ep_list: Vec<String> = file
                    .episode_numbers
                    .iter()
                    .map(|e| format!("E{:02}", e))
                    .collect();
                info!(
                    "Multi-episode file: {} -> S{:02}{}",
                    file.filename,
                    season_number,
                    ep_list.join("")
                );
            }
        }

        info!(
            "Processed scan for series {}: {} new files, {} episodes linked",
            series.title, new_files, matched_episodes
        );

        // Publish series scanned event
        self.event_bus
            .publish(Message::SeriesScanned {
                series_id,
                title: series.title,
                files_found: new_files,
                episodes_matched: matched_episodes,
            })
            .await;

        Ok(())
    }
}

/// Create a scan request message
pub fn create_scan_request(series_ids: Vec<i64>, paths: Vec<String>) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: crate::core::messaging::ScanType::RescanSeries,
        series_ids: series_ids.clone(),
        paths,
    };

    (job_id, message)
}
