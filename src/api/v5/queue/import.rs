//! Import handler — imports completed downloads into the library.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Json,
};

use super::common::QueueActionResponse;
use super::fetch::fetch_all_downloads;
use super::preview::{EpisodeOverride, ImportQueueBody};
use crate::core::datastore::repositories::{
    ArtistRepository, AudiobookRepository, DownloadClientRepository, EpisodeRepository,
    MovieFileRepository, MovieRepository, SeriesRepository,
};
use crate::core::download::clients::create_client_from_model;
use crate::core::parser::normalize_title;
use crate::core::queue::TrackedDownloadState;
use crate::web::AppState;

/// POST /api/v5/queue/{id}/import
/// Import a completed download into the library
pub(super) async fn import_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    body: axum::body::Bytes,
) -> Json<QueueActionResponse> {
    use crate::core::datastore::repositories::TrackedDownloadRepository;
    use crate::core::download::import::ImportService;

    // Parse optional overrides, explicit series ID, force-reimport, and skip list from request body
    let (overrides, explicit_series_id, force_reimport, skip_files): (
        std::collections::HashMap<String, EpisodeOverride>,
        Option<i64>,
        std::collections::HashSet<String>,
        std::collections::HashSet<String>,
    ) = if body.is_empty() {
        (
            std::collections::HashMap::new(),
            None,
            std::collections::HashSet::new(),
            std::collections::HashSet::new(),
        )
    } else {
        match serde_json::from_slice::<ImportQueueBody>(&body) {
            Ok(b) => (
                b.overrides.unwrap_or_default(),
                b.series_id,
                b.force_reimport.unwrap_or_default().into_iter().collect(),
                b.skip_files.unwrap_or_default().into_iter().collect(),
            ),
            Err(_) => (
                std::collections::HashMap::new(),
                None,
                std::collections::HashSet::new(),
                std::collections::HashSet::new(),
            ),
        }
    };

    let td_repo = TrackedDownloadRepository::new(state.db.clone());
    let client_repo = DownloadClientRepository::new(state.db.clone());
    let import_service = ImportService::new(state.db.clone(), state.config.read().media.clone());

    // Find the download — either tracked (id < 10000) or untracked
    let (
        download_id,
        download_client_id,
        title,
        tracked_movie_id,
        tracked_artist_id,
        tracked_audiobook_id,
    ) = if id < 10000 {
        // Tracked download — look up from DB
        match td_repo.get_by_id(id).await {
            Ok(Some(td)) => (
                td.download_id,
                td.download_client_id,
                td.title,
                td.movie_id,
                td.artist_id,
                td.audiobook_id,
            ),
            _ => {
                tracing::warn!("Import: tracked download {} not found", id);
                return Json(QueueActionResponse { success: false });
            }
        }
    } else {
        // Untracked download — find from queue data
        let downloads = fetch_all_downloads(&state, true).await;
        match downloads.into_iter().find(|d| d.id == id) {
            Some(dl) => {
                let dl_id = match dl.download_id {
                    Some(id) => id,
                    None => return Json(QueueActionResponse { success: false }),
                };
                let client_name = dl.download_client.unwrap_or_default();
                // Look up client ID by name
                let clients = client_repo.get_all().await.unwrap_or_default();
                let client_id = clients
                    .iter()
                    .find(|c| c.name == client_name)
                    .map(|c| c.id)
                    .unwrap_or(0);
                (
                    dl_id,
                    client_id,
                    dl.title,
                    dl.movie_id,
                    dl.artist_id,
                    dl.audiobook_id,
                )
            }
            None => {
                tracing::warn!("Import: queue item {} not found", id);
                return Json(QueueActionResponse { success: false });
            }
        }
    };

    // Get live download status from the client (for the real content path)
    let client_model = match client_repo.get_by_id(download_client_id).await {
        Ok(Some(c)) => c,
        _ => {
            tracing::warn!("Import: download client {} not found", download_client_id);
            return Json(QueueActionResponse { success: false });
        }
    };

    let client = match create_client_from_model(&client_model) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Import: failed to create client: {}", e);
            return Json(QueueActionResponse { success: false });
        }
    };

    let live_status = match client.get_download(&download_id).await {
        Ok(Some(dl)) => dl,
        Ok(None) => {
            tracing::warn!("Import: download {} not found in client", download_id);
            return Json(QueueActionResponse { success: false });
        }
        Err(e) => {
            tracing::warn!("Import: failed to query client: {}", e);
            return Json(QueueActionResponse { success: false });
        }
    };

    let raw_output_path = match live_status.output_path {
        Some(ref p) => p.clone(),
        None => {
            tracing::warn!("Import: no output path for download {}", download_id);
            return Json(QueueActionResponse { success: false });
        }
    };

    // Apply remote path mappings (translate client paths to local paths)
    let output_path = {
        use crate::core::datastore::repositories::RemotePathMappingRepository;
        let mapping_repo = RemotePathMappingRepository::new(state.db.clone());
        let mut mapped = raw_output_path.clone();
        let mut did_map = false;
        if let Ok(mappings) = mapping_repo.get_all().await {
            for m in &mappings {
                if mapped.starts_with(&m.remote_path) {
                    mapped = mapped.replacen(&m.remote_path, &m.local_path, 1);
                    tracing::debug!("Import: mapped path '{}' -> '{}'", raw_output_path, mapped);
                    did_map = true;
                    break;
                }
            }
        }
        if !did_map {
            tracing::warn!(
                "Import: no remote path mapping matched '{}' — check Settings > Download Clients > Remote Path Mappings",
                raw_output_path
            );
        }
        mapped
    };

    // Movie import: if this download is matched to a movie, use the movie import flow
    if let Some(movie_id) = tracked_movie_id {
        // Dispatch to worker when Redis is available (fast local disk access)
        if let Some(ref hybrid_bus) = state.hybrid_event_bus {
            if hybrid_bus.is_redis_enabled() {
                let movie_repo = MovieRepository::new(state.db.clone());
                let movie_title = match movie_repo.get_by_id(movie_id).await {
                    Ok(Some(m)) => m.title,
                    _ => title.clone(),
                };

                // Mark tracked download as Importing so the queue UI shows import progress
                if id < 10000 {
                    let _ = td_repo
                        .update_status(id, TrackedDownloadState::Importing as i32, "[]", None)
                        .await;
                }

                let job_id = uuid::Uuid::new_v4().to_string();
                let message = crate::core::messaging::Message::ScanRequest {
                    job_id: job_id.clone(),
                    scan_type: crate::core::messaging::ScanType::DownloadedMovieScan,
                    series_ids: vec![movie_id],
                    paths: vec![output_path.clone()],
                    known_files: std::collections::HashMap::new(),
                };

                if let Some(consumer) = state.scan_result_consumer.get() {
                    consumer
                        .register_job(
                            &job_id,
                            crate::core::messaging::ScanType::DownloadedMovieScan,
                            vec![movie_id],
                        )
                        .await;
                    consumer
                        .register_movie_download_import(
                            &job_id,
                            movie_id,
                            movie_title.clone(),
                            download_id.clone(),
                            download_client_id,
                            title.clone(),
                        )
                        .await;
                    // Link job to download_id so queue API can show progress
                    consumer.set_job_download_id(&job_id, &download_id).await;
                }
                hybrid_bus.enqueue_job(message).await;
                tracing::info!(
                    "Queue movie import: dispatched '{}' to worker (job_id={})",
                    movie_title,
                    job_id
                );
                return Json(QueueActionResponse { success: true });
            }
        }

        // Local fallback: scan + probe + hash over NFS
        let movie_repo = MovieRepository::new(state.db.clone());
        let movie_file_repo = MovieFileRepository::new(state.db.clone());

        let movie = match movie_repo.get_by_id(movie_id).await {
            Ok(Some(m)) => m,
            _ => {
                tracing::warn!(
                    "Import: movie {} not found for download {}",
                    movie_id,
                    download_id
                );
                return Json(QueueActionResponse { success: false });
            }
        };

        let db = state.db.clone();
        let movie_title = movie.title.clone();
        let dl_title = title.clone();
        tokio::spawn(async move {
            // scan_movie_folder finds the largest video file in the output path
            if let Some(mut movie_file) =
                super::super::movies::scan_movie_folder(&output_path, movie_id)
            {
                // Check if this file is already tracked (prevent duplicate imports)
                if let Ok(existing_files) = movie_file_repo.get_by_movie_id(movie_id).await {
                    if existing_files.iter().any(|f| f.path == movie_file.path) {
                        tracing::info!(
                            "Movie import: file already tracked for '{}': {}",
                            movie_title,
                            movie_file.path
                        );
                        return;
                    }
                }

                // Media analysis: probe with FFmpeg + BLAKE3 hash (same as folder import)
                let file_path = std::path::Path::new(&movie_file.path);
                if let Ok(info) = crate::core::mediafiles::MediaAnalyzer::analyze(file_path).await {
                    movie_file.media_info = serde_json::to_string(&info).ok();
                    let quality =
                        crate::core::mediafiles::derive_quality_from_media(&info, &movie_file.path);
                    movie_file.quality = serde_json::to_string(&quality)
                        .unwrap_or_else(|_| movie_file.quality.clone());
                }
                movie_file.file_hash = crate::core::mediafiles::compute_file_hash(file_path)
                    .await
                    .ok();

                match movie_file_repo.insert(&movie_file).await {
                    Ok(file_id) => {
                        // Update movie to have a file
                        let pool = db.pool();
                        let _ = sqlx::query(
                            "UPDATE movies SET has_file = true, movie_file_id = $1 WHERE id = $2",
                        )
                        .bind(file_id)
                        .bind(movie_id)
                        .execute(pool)
                        .await;

                        tracing::info!(
                            "Movie imported: '{}' → movie {} (file: {})",
                            dl_title,
                            movie_title,
                            movie_file.path,
                        );

                        // Delete tracked download — the import is complete and the
                        // record no longer needs to linger. Leaving it as status=4
                        // caused the torrent to re-appear as "untracked" because
                        // get_all_active() (status < 4) excluded it from suppression.
                        if id < 10000 {
                            let td_repo = TrackedDownloadRepository::new(db);
                            let _ = td_repo.delete(id).await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Import: failed to insert movie file for {}: {}",
                            movie_title,
                            e
                        );
                    }
                }
            } else {
                tracing::warn!(
                    "Import: no video file found in '{}' for movie {}",
                    output_path,
                    movie_title
                );
            }
        });

        return Json(QueueActionResponse { success: true });
    }

    // Music import: if this download is matched to an artist, scan for audio files and move to library
    if let Some(artist_id) = tracked_artist_id {
        let artist_repo = ArtistRepository::new(state.db.clone());
        let artist = match artist_repo.get_by_id(artist_id).await {
            Ok(Some(a)) => a,
            _ => {
                tracing::warn!(
                    "Import: artist {} not found for download {}",
                    artist_id,
                    download_id
                );
                return Json(QueueActionResponse { success: false });
            }
        };

        let db = state.db.clone();
        let artist_name = artist.name.clone();
        let artist_path = artist.path.clone();
        let dl_title = title.clone();
        tokio::spawn(async move {
            use crate::core::datastore::repositories::{
                AlbumRepository, TrackFileRepository, TrackRepository,
            };

            let album_repo = AlbumRepository::new(db.clone());
            let track_repo = TrackRepository::new(db.clone());
            let track_file_repo = TrackFileRepository::new(db.clone());

            // Collect audio files from the download directory (recurse one level for album folders)
            let audio_extensions: &[&str] = &[
                "mp3", "flac", "m4a", "ogg", "opus", "wav", "aac", "wma", "alac", "ape", "dsf",
                "dff",
            ];
            let mut audio_files: Vec<(String, String, i64)> = Vec::new(); // (path, filename, size)
            let base = std::path::Path::new(&output_path);

            if let Ok(mut dir) = tokio::fs::read_dir(base).await {
                while let Ok(Some(entry)) = dir.next_entry().await {
                    let path = entry.path();
                    if path.is_file() {
                        let ext = path
                            .extension()
                            .map(|e| e.to_string_lossy().to_lowercase())
                            .unwrap_or_default();
                        if audio_extensions.contains(&ext.as_str()) {
                            let filename = path
                                .file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let size = entry.metadata().await.map(|m| m.len() as i64).unwrap_or(0);
                            audio_files.push((path.to_string_lossy().to_string(), filename, size));
                        }
                    } else if path.is_dir() {
                        if let Ok(mut subdir) = tokio::fs::read_dir(&path).await {
                            while let Ok(Some(sub_entry)) = subdir.next_entry().await {
                                let sub_path = sub_entry.path();
                                if sub_path.is_file() {
                                    let ext = sub_path
                                        .extension()
                                        .map(|e| e.to_string_lossy().to_lowercase())
                                        .unwrap_or_default();
                                    if audio_extensions.contains(&ext.as_str()) {
                                        let filename = sub_path
                                            .file_name()
                                            .map(|f| f.to_string_lossy().to_string())
                                            .unwrap_or_default();
                                        let size = sub_entry
                                            .metadata()
                                            .await
                                            .map(|m| m.len() as i64)
                                            .unwrap_or(0);
                                        audio_files.push((
                                            sub_path.to_string_lossy().to_string(),
                                            filename,
                                            size,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if audio_files.is_empty() {
                tracing::warn!(
                    "Import: no audio files found in '{}' for artist '{}'",
                    output_path,
                    artist_name
                );
                return;
            }

            audio_files.sort_by(|a, b| a.0.cmp(&b.0));

            // Match to album
            let albums = album_repo
                .get_by_artist_id(artist_id)
                .await
                .unwrap_or_default();
            let title_lower = dl_title.to_lowercase();
            let matched_album = albums.iter().find(|a| {
                let clean = a.clean_title.to_lowercase();
                clean.len() >= 4 && title_lower.contains(&clean)
            });

            let album_id = matched_album.map(|a| a.id);
            let album_folder = matched_album
                .map(|a| sanitize_filename::sanitize(&a.title))
                .unwrap_or_else(|| {
                    // Derive album folder from download title: strip "Artist - " prefix
                    let t = dl_title.trim();
                    let after_dash = t.find(" - ").map(|i| &t[i + 3..]).unwrap_or(t);
                    sanitize_filename::sanitize(after_dash)
                });

            // Create destination directory: {artist_path}/{album_folder}/
            let dest_dir = std::path::Path::new(&artist_path).join(&album_folder);
            if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
                tracing::warn!(
                    "Import: failed to create album dir '{}': {}",
                    dest_dir.display(),
                    e
                );
                return;
            }

            let existing_tracks = if let Some(aid) = album_id {
                track_repo.get_by_album_id(aid).await.unwrap_or_default()
            } else {
                Vec::new()
            };

            let mut added = 0;
            let target_album_id = album_id.unwrap_or(0);

            for (file_path, filename, size) in &audio_files {
                // Copy file to library
                let dest_file = dest_dir.join(&filename);
                if dest_file.exists() {
                    tracing::debug!("Import: skipping existing file '{}'", dest_file.display());
                    continue;
                }
                if let Err(e) = tokio::fs::copy(&file_path, &dest_file).await {
                    tracing::warn!(
                        "Import: failed to copy '{}' → '{}': {}",
                        file_path,
                        dest_file.display(),
                        e
                    );
                    continue;
                }

                let dest_path_str = dest_file.to_string_lossy().to_string();
                let (track_num, track_title) = super::super::music::parse_track_filename(filename);

                // Find or create track record
                let track_id = if target_album_id > 0 {
                    if let Some(et) = existing_tracks
                        .iter()
                        .find(|t| t.track_number == track_num && !t.has_file)
                    {
                        Some(et.id)
                    } else if existing_tracks.iter().any(|t| t.track_number == track_num) {
                        None // already has file
                    } else {
                        let new_track = crate::core::datastore::models::TrackDbModel {
                            id: 0,
                            album_id: target_album_id,
                            artist_id,
                            title: track_title.clone(),
                            track_number: track_num,
                            disc_number: 1,
                            duration_ms: None,
                            has_file: true,
                            track_file_id: None,
                            monitored: true,
                            air_date_utc: None,
                        };
                        track_repo.insert(&new_track).await.ok()
                    }
                } else {
                    None
                };

                let relative_path = format!("{}/{}", album_folder, filename);
                let ext = std::path::Path::new(filename)
                    .extension()
                    .map(|e| e.to_string_lossy().to_uppercase())
                    .unwrap_or_else(|| "MP3".to_string());

                let new_file = crate::core::datastore::models::TrackFileDbModel {
                    id: 0,
                    artist_id,
                    album_id: target_album_id,
                    relative_path,
                    path: dest_path_str,
                    size: *size,
                    quality: serde_json::json!({ "codec": ext }).to_string(),
                    media_info: Some(serde_json::json!({ "audio_format": ext }).to_string()),
                    date_added: chrono::Utc::now(),
                };

                if let Ok(file_id) = track_file_repo.insert(&new_file).await {
                    if let Some(tid) = track_id {
                        let pool = db.pool();
                        let _ = sqlx::query(
                            "UPDATE tracks SET has_file = true, track_file_id = $1 WHERE id = $2",
                        )
                        .bind(file_id)
                        .bind(tid)
                        .execute(pool)
                        .await;
                    }
                    added += 1;
                }
            }

            tracing::info!(
                "Music imported: '{}' → '{}' ({} files copied to {})",
                dl_title,
                artist_name,
                added,
                dest_dir.display(),
            );

            if id < 10000 {
                let td_repo = TrackedDownloadRepository::new(db);
                let _ = td_repo.delete(id).await;
            }
        });

        return Json(QueueActionResponse { success: true });
    }

    // Audiobook import: if this download is matched to an audiobook, scan for audio files and move to library
    if let Some(audiobook_id) = tracked_audiobook_id {
        let audiobook_repo = AudiobookRepository::new(state.db.clone());
        let audiobook = match audiobook_repo.get_by_id(audiobook_id).await {
            Ok(Some(ab)) => ab,
            _ => {
                tracing::warn!(
                    "Import: audiobook {} not found for download {}",
                    audiobook_id,
                    download_id
                );
                return Json(QueueActionResponse { success: false });
            }
        };

        let db = state.db.clone();
        let audiobook_title = audiobook.title.clone();
        let audiobook_path = audiobook.path.clone();
        tokio::spawn(async move {
            use crate::core::datastore::repositories::AudiobookFileRepository;

            let file_repo = AudiobookFileRepository::new(db.clone());

            // Collect audio files from download directory
            let audio_extensions: &[&str] = &[
                "mp3", "flac", "m4a", "m4b", "ogg", "opus", "wav", "aac", "wma", "alac", "ape",
            ];
            let mut audio_files: Vec<(String, String, i64)> = Vec::new();
            let base = std::path::Path::new(&output_path);

            if let Ok(mut dir) = tokio::fs::read_dir(base).await {
                while let Ok(Some(entry)) = dir.next_entry().await {
                    let path = entry.path();
                    if path.is_file() {
                        let ext = path
                            .extension()
                            .map(|e| e.to_string_lossy().to_lowercase())
                            .unwrap_or_default();
                        if audio_extensions.contains(&ext.as_str()) {
                            let filename = path
                                .file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let size = entry.metadata().await.map(|m| m.len() as i64).unwrap_or(0);
                            audio_files.push((path.to_string_lossy().to_string(), filename, size));
                        }
                    } else if path.is_dir() {
                        if let Ok(mut subdir) = tokio::fs::read_dir(&path).await {
                            while let Ok(Some(sub_entry)) = subdir.next_entry().await {
                                let sub_path = sub_entry.path();
                                if sub_path.is_file() {
                                    let ext = sub_path
                                        .extension()
                                        .map(|e| e.to_string_lossy().to_lowercase())
                                        .unwrap_or_default();
                                    if audio_extensions.contains(&ext.as_str()) {
                                        let filename = sub_path
                                            .file_name()
                                            .map(|f| f.to_string_lossy().to_string())
                                            .unwrap_or_default();
                                        let size = sub_entry
                                            .metadata()
                                            .await
                                            .map(|m| m.len() as i64)
                                            .unwrap_or(0);
                                        audio_files.push((
                                            sub_path.to_string_lossy().to_string(),
                                            filename,
                                            size,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if audio_files.is_empty() {
                tracing::warn!(
                    "Import: no audio files found in '{}' for audiobook '{}'",
                    output_path,
                    audiobook_title
                );
                return;
            }

            // Create destination directory
            let dest_dir = std::path::Path::new(&audiobook_path);
            if let Err(e) = tokio::fs::create_dir_all(dest_dir).await {
                tracing::warn!(
                    "Import: failed to create audiobook dir '{}': {}",
                    dest_dir.display(),
                    e
                );
                return;
            }

            let mut added = 0;
            let mut total_size: i64 = 0;

            for (file_path, filename, size) in &audio_files {
                let dest_file = dest_dir.join(filename);
                if dest_file.exists() {
                    continue;
                }
                if let Err(e) = tokio::fs::copy(&file_path, &dest_file).await {
                    tracing::warn!(
                        "Import: failed to copy '{}' → '{}': {}",
                        file_path,
                        dest_file.display(),
                        e
                    );
                    continue;
                }

                let dest_path_str = dest_file.to_string_lossy().to_string();
                let ext = std::path::Path::new(filename)
                    .extension()
                    .map(|e| e.to_string_lossy().to_uppercase())
                    .unwrap_or_else(|| "MP3".to_string());

                let new_file = crate::core::datastore::models::AudiobookFileDbModel {
                    id: 0,
                    audiobook_id,
                    relative_path: filename.clone(),
                    path: dest_path_str,
                    size: *size,
                    quality: serde_json::json!({ "codec": ext }).to_string(),
                    media_info: Some(serde_json::json!({ "audio_format": ext }).to_string()),
                    date_added: chrono::Utc::now(),
                };

                if let Ok(file_id) = file_repo.insert(&new_file).await {
                    total_size += size;
                    added += 1;
                    // Update audiobook to point to the first file
                    if added == 1 {
                        let pool = db.pool();
                        let _ = sqlx::query("UPDATE audiobooks SET has_file = true, audiobook_file_id = $1 WHERE id = $2")
                            .bind(file_id).bind(audiobook_id).execute(pool).await;
                    }
                }
            }

            tracing::info!(
                "Audiobook imported: '{}' ({} files copied to {})",
                audiobook_title,
                added,
                dest_dir.display(),
            );

            if id < 10000 {
                let td_repo = TrackedDownloadRepository::new(db);
                let _ = td_repo.delete(id).await;
            }
        });

        return Json(QueueActionResponse { success: true });
    }

    // Build a PendingImport using the import service's matching logic
    let mut parsed = crate::core::parser::parse_title(&title);
    let mut series = None;
    let mut episodes = Vec::new();

    if let Some(ref info) = parsed {
        if let Ok(s) = import_service.match_series(info).await {
            if let Some(ref matched) = s {
                if let Ok(eps) = import_service.match_episodes(matched, info).await {
                    episodes = eps;
                }
            }
            series = s;
        }
    }

    // Fallback: when parser can't extract structured data (complete series
    // packs without S01E02 markers), or when it extracts quality info but no
    // season/episode numbers (so match_episodes returns empty), match the raw
    // title against known series and treat it as a multi-season pack so
    // per-file parsing handles individual episode assignment.
    if series.is_none() || episodes.is_empty() {
        let series_repo = SeriesRepository::new(state.db.clone());
        let episode_repo = EpisodeRepository::new(state.db.clone());

        // If series was already matched via primary path, use it directly
        // instead of re-matching against all series.
        if let Some(ref matched) = series {
            tracing::info!(
                "Import fallback: series '{}' matched but no episodes resolved, loading all episodes for pack import",
                matched.title
            );
            if let Ok(eps) = episode_repo.get_by_series_id(matched.id).await {
                episodes = eps;
            }
            parsed = Some(crate::core::parser::ParsedEpisodeInfo {
                series_title: matched.clean_title.clone(),
                full_season: true,
                is_multi_season: true,
                raw_title: title.clone(),
                ..Default::default()
            });
        } else if let Ok(all_series) = series_repo.get_all().await {
            let name_normalized = normalize_title(&title);
            let mut best_match: Option<(usize, usize)> = None;

            for (idx, s) in all_series.iter().enumerate() {
                let clean = normalize_title(&s.clean_title);
                let clean_no_year = clean
                    .trim_end()
                    .rsplit_once(' ')
                    .and_then(|(prefix, suffix)| {
                        if suffix.len() == 4 && suffix.chars().all(|c| c.is_ascii_digit()) {
                            Some(prefix.to_string())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| clean.clone());

                for candidate in [&clean, &clean_no_year] {
                    if candidate.len() >= 4 && name_normalized.contains(candidate.as_str()) {
                        if best_match.is_none() || candidate.len() > best_match.as_ref().unwrap().1
                        {
                            best_match = Some((idx, candidate.len()));
                        }
                    }
                }
            }

            if let Some((idx, _)) = best_match {
                let matched = all_series.into_iter().nth(idx).unwrap();
                tracing::info!(
                    "Import fallback: matched '{}' to series '{}'",
                    title,
                    matched.title
                );
                if let Ok(eps) = episode_repo.get_by_series_id(matched.id).await {
                    episodes = eps;
                }
                parsed = Some(crate::core::parser::ParsedEpisodeInfo {
                    series_title: matched.clean_title.clone(),
                    full_season: true,
                    is_multi_season: true,
                    raw_title: title.clone(),
                    ..Default::default()
                });
                series = Some(matched);
            }
        }
    }

    // Fallback: use explicit series_id from the import preview UI when title matching failed
    if series.is_none() {
        if let Some(sid) = explicit_series_id {
            let series_repo = SeriesRepository::new(state.db.clone());
            if let Ok(Some(s)) = series_repo.get_by_id(sid).await {
                tracing::info!(
                    "Import: using explicit series_id={} '{}' (title '{}' didn't match)",
                    sid,
                    s.title,
                    title
                );
                series = Some(s);
            }
        }
    }

    // Apply manual episode overrides from the UI
    if !overrides.is_empty() {
        if let Some(ref matched_series) = series {
            let episode_repo = EpisodeRepository::new(state.db.clone());
            if let Ok(all_eps) = episode_repo.get_by_series_id(matched_series.id).await {
                for ov in overrides.values() {
                    for ep_num in ov.episodes() {
                        if let Some(ep) = all_eps.iter().find(|e| {
                            e.season_number == ov.season_number && e.episode_number == ep_num
                        }) {
                            if !episodes.iter().any(|e| e.id == ep.id) {
                                episodes.push(ep.clone());
                            }
                        }
                    }
                }
                // Ensure we have full_season/is_multi_season set for pack-style import
                if parsed.is_none() || episodes.len() > 1 {
                    parsed = Some(crate::core::parser::ParsedEpisodeInfo {
                        series_title: matched_series.clean_title.clone(),
                        full_season: true,
                        is_multi_season: true,
                        raw_title: title.clone(),
                        ..Default::default()
                    });
                }
            }
        }
        tracing::info!(
            "Import: {} manual override(s) applied for '{}'",
            overrides.len(),
            title
        );
    }

    // Convert overrides to Vec<(season, episode)> per file
    let override_map: std::collections::HashMap<String, Vec<(i32, i32)>> = overrides
        .iter()
        .map(|(k, v)| {
            let pairs: Vec<(i32, i32)> = v
                .episodes()
                .into_iter()
                .map(|ep| (v.season_number, ep))
                .collect();
            (k.clone(), pairs)
        })
        .collect();

    let pending = crate::core::download::import::PendingImport {
        download_id: download_id.clone(),
        download_client_id,
        download_client_name: client_model.name.clone(),
        title: title.clone(),
        output_path: std::path::PathBuf::from(&output_path),
        parsed_info: parsed,
        series,
        episodes,
        overrides: override_map.clone(),
        force_reimport: force_reimport.clone(),
        skip_files: skip_files.clone(),
    };

    // Dispatch to Redis worker when available — worker has fast local disk access
    // for FFmpeg probing + BLAKE3 hashing instead of going over NFS
    if let Some(ref hybrid_bus) = state.hybrid_event_bus {
        if hybrid_bus.is_redis_enabled() {
            if let Some(consumer) = state.scan_result_consumer.get() {
                // Mark tracked download as Importing so the queue UI shows import progress
                if id < 10000 {
                    let _ = td_repo
                        .update_status(id, TrackedDownloadState::Importing as i32, "[]", None)
                        .await;
                }

                let job_id = uuid::Uuid::new_v4().to_string();

                let import_info = crate::core::scanner::DownloadImportInfo {
                    download_id: pending.download_id.clone(),
                    download_client_id: pending.download_client_id,
                    download_client_name: pending.download_client_name.clone(),
                    title: pending.title.clone(),
                    output_path: pending.output_path.clone(),
                    parsed_info: pending.parsed_info.clone(),
                    series: pending.series.clone(),
                    episodes: pending.episodes.clone(),
                    overrides: override_map.clone(),
                    force_reimport: force_reimport.clone(),
                    skip_files: skip_files.clone(),
                    force_import_all: true, // user-initiated: bypass same-size skip
                };

                consumer
                    .register_download_import(&job_id, vec![import_info])
                    .await;
                consumer
                    .register_job(
                        &job_id,
                        crate::core::messaging::ScanType::DownloadedEpisodesScan,
                        vec![0],
                    )
                    .await;
                // Link job to download_id so queue API can show progress
                consumer.set_job_download_id(&job_id, &download_id).await;

                let message = crate::core::messaging::Message::ScanRequest {
                    job_id: job_id.clone(),
                    scan_type: crate::core::messaging::ScanType::DownloadedEpisodesScan,
                    series_ids: vec![0],
                    paths: vec![output_path.clone()],
                    known_files: std::collections::HashMap::new(),
                };
                hybrid_bus.enqueue_job(message).await;

                tracing::info!(
                    "Queue series import: dispatched '{}' to worker (job_id={})",
                    pending.title,
                    job_id,
                );
                return Json(QueueActionResponse { success: true });
            }
        }
    }

    // Fallback: run the import locally (no Redis worker available).
    // Season/multi-season packs can take minutes (FFmpeg probing + hashing per file).
    let db = state.db.clone();
    tokio::spawn(async move {
        match import_service.import(&pending).await {
            Ok(result) if result.success => {
                tracing::info!(
                    "Imported '{}': {} files, {} episodes",
                    pending.title,
                    result.files_imported,
                    result.episode_ids.len()
                );

                // Delete tracked download — import is complete, no need to
                // keep the record (status=4 records caused ghost duplicates).
                if id < 10000 {
                    let td_repo = TrackedDownloadRepository::new(db);
                    let _ = td_repo.delete(id).await;
                }
            }
            Ok(result) => {
                let msg = result
                    .error_message
                    .unwrap_or_else(|| "Unknown error".to_string());
                tracing::warn!("Import failed for '{}': {}", pending.title, msg);
            }
            Err(e) => {
                tracing::warn!("Import error for '{}': {}", pending.title, e);
            }
        }
    });

    Json(QueueActionResponse { success: true })
}
