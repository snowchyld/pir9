//! IMDB Dataset Sync
//!
//! Downloads and parses IMDB non-commercial datasets from https://datasets.imdbws.com/

use std::io::{BufRead, BufReader};

use anyhow::Result;
use chrono::Utc;
use flate2::read::GzDecoder;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::db::{DbRepository, ResumeInfo};
use crate::models::*;

/// IMDB datasets base URL
const IMDB_BASE_URL: &str = "https://datasets.imdbws.com";

/// Progress log interval (rows processed between DB updates)
const PROGRESS_INTERVAL: i64 = 10_000;

/// Cancel check interval (check every N rows — is_cancelled() is just an atomic read)
const CANCEL_CHECK_INTERVAL: i64 = 10_000;

/// Batch size for DB upserts (rows per INSERT statement)
const BATCH_SIZE: usize = 1000;

/// Minimum hours between re-syncing the same dataset
const SKIP_IF_RECENT_HOURS: i64 = 24;

/// TV title types we care about
const TV_TITLE_TYPES: &[&str] = &["tvSeries", "tvMiniSeries"];

/// Movie title types we care about (includes TV movies and direct-to-video)
const MOVIE_TITLE_TYPES: &[&str] = &["movie", "tvMovie", "video"];

/// Download a dataset file, racing against the cancellation token.
/// Returns the raw bytes on success, or DatasetResult::Cancelled if the token fires.
async fn download_dataset(
    url: &str,
    token: &CancellationToken,
) -> Result<std::result::Result<Vec<u8>, DatasetResult>> {
    let client = reqwest::Client::builder()
        .user_agent("pir9-IMDB/0.1.0")
        .timeout(std::time::Duration::from_secs(3600))
        .build()?;

    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download: {}", response.status());
    }

    // Race the download against cancellation
    tokio::select! {
        biased;
        _ = token.cancelled() => {
            info!("Download of {} cancelled", url);
            Ok(Err(DatasetResult::Cancelled))
        }
        result = response.bytes() => {
            Ok(Ok(result?.to_vec()))
        }
    }
}

/// Result of a dataset sync attempt
pub enum DatasetResult {
    /// Sync completed successfully
    Completed(SyncStats),
    /// Sync was skipped (recently completed)
    Skipped,
    /// Sync was cancelled
    Cancelled,
}

/// Run a full sync of all IMDB datasets
pub async fn run_full_sync(db: &DbRepository, token: CancellationToken) -> Result<SyncReport> {
    let mut report = SyncReport::default();

    // Sync title.basics (series)
    info!("Syncing title.basics...");
    match sync_title_basics(db, &token).await {
        Ok(DatasetResult::Completed(stats)) => {
            info!(
                "title.basics completed: {} processed, {} inserted",
                stats.rows_processed, stats.rows_inserted
            );
            report.title_basics = Some(stats);
        }
        Ok(DatasetResult::Skipped) => {
            info!("title.basics skipped (recently synced)");
        }
        Ok(DatasetResult::Cancelled) => {
            info!("title.basics cancelled");
            return Ok(report);
        }
        Err(e) => {
            error!("title.basics failed: {}", e);
            report.errors.push(format!("title.basics: {}", e));
        }
    }

    if token.is_cancelled() {
        return Ok(report);
    }

    // Sync title.episode (episodes) - only if we have series
    info!("Syncing title.episode...");
    match sync_title_episodes(db, &token).await {
        Ok(DatasetResult::Completed(stats)) => {
            info!(
                "title.episode completed: {} processed, {} inserted",
                stats.rows_processed, stats.rows_inserted
            );
            report.title_episodes = Some(stats);
        }
        Ok(DatasetResult::Skipped) => {
            info!("title.episode skipped (recently synced)");
        }
        Ok(DatasetResult::Cancelled) => {
            info!("title.episode cancelled");
            return Ok(report);
        }
        Err(e) => {
            error!("title.episode failed: {}", e);
            report.errors.push(format!("title.episode: {}", e));
        }
    }

    if token.is_cancelled() {
        return Ok(report);
    }

    // Sync title.ratings
    info!("Syncing title.ratings...");
    match sync_title_ratings(db, &token).await {
        Ok(DatasetResult::Completed(stats)) => {
            info!(
                "title.ratings completed: {} processed, {} updated",
                stats.rows_processed, stats.rows_updated
            );
            report.title_ratings = Some(stats);
        }
        Ok(DatasetResult::Skipped) => {
            info!("title.ratings skipped (recently synced)");
        }
        Ok(DatasetResult::Cancelled) => {
            info!("title.ratings cancelled");
            return Ok(report);
        }
        Err(e) => {
            error!("title.ratings failed: {}", e);
            report.errors.push(format!("title.ratings: {}", e));
        }
    }

    Ok(report)
}

/// Check if a dataset was completed recently enough to skip
async fn should_skip_dataset(db: &DbRepository, dataset: &str) -> bool {
    match db.last_completed_sync_time(dataset).await {
        Ok(Some(completed_at)) => {
            let hours_ago = (Utc::now() - completed_at).num_hours();
            if hours_ago < SKIP_IF_RECENT_HOURS {
                info!(
                    "{} was synced {}h ago (threshold: {}h), skipping",
                    dataset, hours_ago, SKIP_IF_RECENT_HOURS
                );
                return true;
            }
            false
        }
        _ => false,
    }
}

/// Get or create a sync record, handling resume from previous failed/cancelled sync.
/// Returns (sync_id, ResumeInfo) where ResumeInfo contains the checkpoint and old counters.
async fn get_or_resume_sync(db: &DbRepository, dataset: &str) -> Result<(i64, ResumeInfo)> {
    // Check for a resumable sync first
    if let Some(resume) = db.get_resumable_sync(dataset).await? {
        info!(
            "Resuming {} from sync_id={}, last_processed_id={}, prior progress: {} processed",
            dataset, resume.sync_id, resume.last_processed_id, resume.rows_processed
        );
        db.resume_sync(resume.sync_id).await?;
        let sync_id = resume.sync_id;
        return Ok((sync_id, resume));
    }

    // No resumable sync, create a new one
    let sync_id = db.start_sync(dataset).await?;
    Ok((sync_id, ResumeInfo {
        sync_id,
        last_processed_id: 0,
        rows_processed: 0,
        rows_inserted: 0,
        rows_updated: 0,
    }))
}

/// Sync title.basics.tsv.gz (TV series)
async fn sync_title_basics(db: &DbRepository, token: &CancellationToken) -> Result<DatasetResult> {
    let dataset = "title.basics.tsv.gz";

    if should_skip_dataset(db, dataset).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", IMDB_BASE_URL, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_title_basics_inner(db, &url, sync_id, &resume, token).await;

    match result {
        Ok(DatasetResult::Completed(stats)) => {
            db.complete_sync(sync_id).await?;
            Ok(DatasetResult::Completed(SyncStats {
                rows_processed: stats.rows_processed,
                rows_inserted: stats.rows_inserted,
                rows_updated: stats.rows_updated,
                duration_seconds: start_time.elapsed().as_secs() as i64,
            }))
        }
        Ok(DatasetResult::Cancelled) => {
            db.cancel_sync(sync_id).await?;
            Ok(DatasetResult::Cancelled)
        }
        Ok(DatasetResult::Skipped) => Ok(DatasetResult::Skipped),
        Err(e) => {
            db.fail_sync(sync_id, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn sync_title_basics_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
) -> Result<DatasetResult> {
    let resume_from = resume.last_processed_id;
    if resume_from > 0 {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    let bytes = match download_dataset(url, token).await? {
        Ok(b) => b,
        Err(cancelled) => return Ok(cancelled),
    };

    let decoder = GzDecoder::new(&bytes[..]);
    let reader = BufReader::new(decoder);

    // Start counters from previous values when resuming
    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut rows_skipped: i64 = 0;
    let mut last_id: i64 = 0;

    // Batch buffers
    let mut series_batch: Vec<DbSeries> = Vec::with_capacity(BATCH_SIZE);
    let mut movie_batch: Vec<DbMovie> = Vec::with_capacity(BATCH_SIZE);

    for (line_num, line_result) in reader.lines().enumerate() {
        if line_num == 0 {
            continue; // Skip header
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 9 {
            continue;
        }

        let title_type = fields[1];
        let is_tv = TV_TITLE_TYPES.contains(&title_type);
        let is_movie = MOVIE_TITLE_TYPES.contains(&title_type);

        // Only process TV series and movie types
        if !is_tv && !is_movie {
            continue;
        }

        let imdb_id = match parse_imdb_id(fields[0]) {
            Some(id) => id,
            None => continue,
        };

        // Skip rows we've already processed (resume support)
        if imdb_id <= resume_from {
            rows_skipped += 1;
            continue;
        }

        if is_tv {
            series_batch.push(DbSeries {
                imdb_id,
                title: fields[2].to_string(),
                original_title: if fields[3] != "\\N" && fields[3] != fields[2] {
                    Some(fields[3].to_string())
                } else {
                    None
                },
                is_adult: fields[4] == "1",
                start_year: parse_int(fields[5]),
                end_year: parse_int(fields[6]),
                runtime_minutes: parse_int(fields[7]),
                genres: if fields[8] != "\\N" {
                    Some(fields[8].to_string())
                } else {
                    None
                },
                title_type: title_type.to_string(),
                rating: None,
                votes: None,
                last_synced_at: Utc::now(),
            });

            if series_batch.len() >= BATCH_SIZE {
                rows_inserted += flush_series(db, &mut series_batch).await? as i64;
            }
        } else {
            movie_batch.push(DbMovie {
                imdb_id,
                title: fields[2].to_string(),
                original_title: if fields[3] != "\\N" && fields[3] != fields[2] {
                    Some(fields[3].to_string())
                } else {
                    None
                },
                is_adult: fields[4] == "1",
                year: parse_int(fields[5]),
                runtime_minutes: parse_int(fields[7]),
                genres: if fields[8] != "\\N" {
                    Some(fields[8].to_string())
                } else {
                    None
                },
                rating: None,
                votes: None,
                last_synced_at: Utc::now(),
            });

            if movie_batch.len() >= BATCH_SIZE {
                rows_inserted += flush_movies(db, &mut movie_batch).await? as i64;
            }
        }

        rows_processed += 1;
        last_id = imdb_id;

        if rows_processed % PROGRESS_INTERVAL == 0 {
            // Flush before checkpoint so last_processed_id is never ahead of DB state
            rows_inserted += flush_series(db, &mut series_batch).await? as i64;
            rows_inserted += flush_movies(db, &mut movie_batch).await? as i64;

            if rows_skipped > 0 {
                info!(
                    "title.basics progress: {} rows (skipped {} resumed)",
                    rows_processed, rows_skipped
                );
                rows_skipped = 0; // Only log once
            } else {
                info!("title.basics progress: {} rows", rows_processed);
            }
            db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
                .await?;
        }

        // Cooperative cancellation check
        if rows_processed % CANCEL_CHECK_INTERVAL == 0 && token.is_cancelled() {
            rows_inserted += flush_series(db, &mut series_batch).await? as i64;
            rows_inserted += flush_movies(db, &mut movie_batch).await? as i64;
            info!(
                "title.basics cancelled at row {} (last_id={})",
                rows_processed, last_id
            );
            db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
                .await?;
            return Ok(DatasetResult::Cancelled);
        }
    }

    // Flush remaining
    rows_inserted += flush_series(db, &mut series_batch).await? as i64;
    rows_inserted += flush_movies(db, &mut movie_batch).await? as i64;

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
        .await?;

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

/// Sync title.episode.tsv.gz (episodes)
async fn sync_title_episodes(
    db: &DbRepository,
    token: &CancellationToken,
) -> Result<DatasetResult> {
    let dataset = "title.episode.tsv.gz";

    if should_skip_dataset(db, dataset).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", IMDB_BASE_URL, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_title_episodes_inner(db, &url, sync_id, &resume, token).await;

    match result {
        Ok(DatasetResult::Completed(stats)) => {
            db.complete_sync(sync_id).await?;
            Ok(DatasetResult::Completed(SyncStats {
                rows_processed: stats.rows_processed,
                rows_inserted: stats.rows_inserted,
                rows_updated: stats.rows_updated,
                duration_seconds: start_time.elapsed().as_secs() as i64,
            }))
        }
        Ok(DatasetResult::Cancelled) => {
            db.cancel_sync(sync_id).await?;
            Ok(DatasetResult::Cancelled)
        }
        Ok(DatasetResult::Skipped) => Ok(DatasetResult::Skipped),
        Err(e) => {
            db.fail_sync(sync_id, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn sync_title_episodes_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
) -> Result<DatasetResult> {
    let resume_from = resume.last_processed_id;
    if resume_from > 0 {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    // First, get all our series IDs for filtering
    let series_ids: std::collections::HashSet<i64> =
        sqlx::query_scalar("SELECT imdb_id FROM imdb_series")
            .fetch_all(db.pool())
            .await?
            .into_iter()
            .collect();

    info!("Filtering episodes for {} series", series_ids.len());

    let bytes = match download_dataset(url, token).await? {
        Ok(b) => b,
        Err(cancelled) => return Ok(cancelled),
    };

    let decoder = GzDecoder::new(&bytes[..]);
    let reader = BufReader::new(decoder);

    // Start counters from previous values when resuming
    let mut rows_processed = resume.rows_processed;
    let mut rows_inserted = resume.rows_inserted;
    let mut last_id: i64 = 0;

    let mut episode_batch: Vec<DbEpisode> = Vec::with_capacity(BATCH_SIZE);

    for (line_num, line_result) in reader.lines().enumerate() {
        if line_num == 0 {
            continue;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            continue;
        }

        let episode_id = match parse_imdb_id(fields[0]) {
            Some(id) => id,
            None => continue,
        };

        let parent_id = match parse_imdb_id(fields[1]) {
            Some(id) => id,
            None => continue,
        };

        // Only process episodes for series we have
        if !series_ids.contains(&parent_id) {
            continue;
        }

        // Skip rows we've already processed (resume support)
        if episode_id <= resume_from {
            continue;
        }

        episode_batch.push(DbEpisode {
            imdb_id: episode_id,
            parent_imdb_id: parent_id,
            season_number: parse_int(fields[2]),
            episode_number: parse_int(fields[3]),
            title: None,
            runtime_minutes: None,
            rating: None,
            votes: None,
            air_date: None,
            last_synced_at: Utc::now(),
        });

        if episode_batch.len() >= BATCH_SIZE {
            rows_inserted += flush_episodes(db, &mut episode_batch).await? as i64;
        }

        rows_processed += 1;
        last_id = episode_id;

        if rows_processed % PROGRESS_INTERVAL == 0 {
            rows_inserted += flush_episodes(db, &mut episode_batch).await? as i64;
            info!("title.episode progress: {} rows", rows_processed);
            db.update_sync_progress_with_resume(
                sync_id,
                rows_processed,
                rows_inserted,
                0,
                last_id,
            )
            .await?;
        }

        // Cooperative cancellation check
        if rows_processed % CANCEL_CHECK_INTERVAL == 0 && token.is_cancelled() {
            rows_inserted += flush_episodes(db, &mut episode_batch).await? as i64;
            info!(
                "title.episode cancelled at row {} (last_id={})",
                rows_processed, last_id
            );
            db.update_sync_progress_with_resume(
                sync_id,
                rows_processed,
                rows_inserted,
                0,
                last_id,
            )
            .await?;
            return Ok(DatasetResult::Cancelled);
        }
    }

    // Flush remaining
    rows_inserted += flush_episodes(db, &mut episode_batch).await? as i64;

    db.update_sync_progress_with_resume(sync_id, rows_processed, rows_inserted, 0, last_id)
        .await?;

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted,
        rows_updated: 0,
        duration_seconds: 0,
    }))
}

/// Sync title.ratings.tsv.gz
async fn sync_title_ratings(
    db: &DbRepository,
    token: &CancellationToken,
) -> Result<DatasetResult> {
    let dataset = "title.ratings.tsv.gz";

    if should_skip_dataset(db, dataset).await {
        return Ok(DatasetResult::Skipped);
    }

    let url = format!("{}/{}", IMDB_BASE_URL, dataset);
    let start_time = std::time::Instant::now();

    let (sync_id, resume) = get_or_resume_sync(db, dataset).await?;

    let result = sync_title_ratings_inner(db, &url, sync_id, &resume, token).await;

    match result {
        Ok(DatasetResult::Completed(stats)) => {
            db.complete_sync(sync_id).await?;
            Ok(DatasetResult::Completed(SyncStats {
                rows_processed: stats.rows_processed,
                rows_inserted: stats.rows_inserted,
                rows_updated: stats.rows_updated,
                duration_seconds: start_time.elapsed().as_secs() as i64,
            }))
        }
        Ok(DatasetResult::Cancelled) => {
            db.cancel_sync(sync_id).await?;
            Ok(DatasetResult::Cancelled)
        }
        Ok(DatasetResult::Skipped) => Ok(DatasetResult::Skipped),
        Err(e) => {
            db.fail_sync(sync_id, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn sync_title_ratings_inner(
    db: &DbRepository,
    url: &str,
    sync_id: i64,
    resume: &ResumeInfo,
    token: &CancellationToken,
) -> Result<DatasetResult> {
    let resume_from = resume.last_processed_id;
    if resume_from > 0 {
        info!("Downloading {} (resuming from id {})", url, resume_from);
    } else {
        info!("Downloading {}", url);
    }

    let bytes = match download_dataset(url, token).await? {
        Ok(b) => b,
        Err(cancelled) => return Ok(cancelled),
    };

    let decoder = GzDecoder::new(&bytes[..]);
    let reader = BufReader::new(decoder);

    // Start counters from previous values when resuming
    let mut rows_processed = resume.rows_processed;
    let mut rows_updated = resume.rows_updated;
    let mut last_id: i64 = 0;

    // Batch buffers: (imdb_id, rating, votes)
    let mut rating_ids: Vec<i64> = Vec::with_capacity(BATCH_SIZE);
    let mut rating_vals: Vec<f64> = Vec::with_capacity(BATCH_SIZE);
    let mut rating_votes: Vec<i64> = Vec::with_capacity(BATCH_SIZE);

    for (line_num, line_result) in reader.lines().enumerate() {
        if line_num == 0 {
            continue;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            continue;
        }

        let imdb_id = match parse_imdb_id(fields[0]) {
            Some(id) => id,
            None => continue,
        };

        // Skip rows we've already processed (resume support)
        if imdb_id <= resume_from {
            continue;
        }

        let rating: f64 = match fields[1].parse() {
            Ok(r) => r,
            Err(_) => continue,
        };

        let votes: i64 = match fields[2].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        rating_ids.push(imdb_id);
        rating_vals.push(rating);
        rating_votes.push(votes);

        if rating_ids.len() >= BATCH_SIZE {
            rows_updated += flush_ratings(db, &mut rating_ids, &mut rating_vals, &mut rating_votes).await?;
        }

        rows_processed += 1;
        last_id = imdb_id;

        if rows_processed % (PROGRESS_INTERVAL * 10) == 0 {
            rows_updated += flush_ratings(db, &mut rating_ids, &mut rating_vals, &mut rating_votes).await?;
            info!("title.ratings progress: {} rows", rows_processed);
            db.update_sync_progress_with_resume(
                sync_id,
                rows_processed,
                0,
                rows_updated,
                last_id,
            )
            .await?;
        }

        // Cooperative cancellation check
        if rows_processed % CANCEL_CHECK_INTERVAL == 0 && token.is_cancelled() {
            rows_updated += flush_ratings(db, &mut rating_ids, &mut rating_vals, &mut rating_votes).await?;
            info!(
                "title.ratings cancelled at row {} (last_id={})",
                rows_processed, last_id
            );
            db.update_sync_progress_with_resume(
                sync_id,
                rows_processed,
                0,
                rows_updated,
                last_id,
            )
            .await?;
            return Ok(DatasetResult::Cancelled);
        }
    }

    // Flush remaining
    rows_updated += flush_ratings(db, &mut rating_ids, &mut rating_vals, &mut rating_votes).await?;

    db.update_sync_progress_with_resume(sync_id, rows_processed, 0, rows_updated, last_id)
        .await?;

    Ok(DatasetResult::Completed(SyncStats {
        rows_processed,
        rows_inserted: 0,
        rows_updated,
        duration_seconds: 0,
    }))
}

// ── Batch flush helpers ──────────────────────────────────────────────

async fn flush_series(db: &DbRepository, batch: &mut Vec<DbSeries>) -> Result<u64> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = db.upsert_series_batch(batch).await?;
    batch.clear();
    Ok(count)
}

async fn flush_movies(db: &DbRepository, batch: &mut Vec<DbMovie>) -> Result<u64> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = db.upsert_movie_batch(batch).await?;
    batch.clear();
    Ok(count)
}

async fn flush_episodes(db: &DbRepository, batch: &mut Vec<DbEpisode>) -> Result<u64> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = db.upsert_episode_batch(batch).await?;
    batch.clear();
    Ok(count)
}

async fn flush_ratings(
    db: &DbRepository,
    ids: &mut Vec<i64>,
    ratings: &mut Vec<f64>,
    votes: &mut Vec<i64>,
) -> Result<i64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let series_count = db.update_series_ratings_batch(ids, ratings, votes).await?;
    let movie_count = db.update_movie_ratings_batch(ids, ratings, votes).await?;
    ids.clear();
    ratings.clear();
    votes.clear();
    Ok((series_count + movie_count) as i64)
}

/// Parse an integer from a string, handling IMDB null values
fn parse_int<T: std::str::FromStr>(s: &str) -> Option<T> {
    if s == "\\N" || s.is_empty() {
        None
    } else {
        s.parse().ok()
    }
}

/// Parse IMDB ID string to numeric
fn parse_imdb_id(s: &str) -> Option<i64> {
    if s.len() > 2 && s.starts_with("tt") {
        s[2..].parse().ok()
    } else {
        None
    }
}
