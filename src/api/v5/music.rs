#![allow(dead_code, unused_imports, unused_variables)]
//! Music API endpoints
//! CRUD operations for artists, albums, and tracks

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::{ArtistDbModel, AlbumDbModel, TrackDbModel, TrackFileDbModel};
use crate::core::datastore::repositories::{
    ArtistRepository, AlbumRepository, TrackRepository, TrackFileRepository,
};
use crate::web::AppState;

// Re-use ApiError from series module
use super::series::ApiError;

// Re-use helper functions from movies module
use super::movies::{clean_title, generate_slug};

/// Query parameters for listing artists
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArtistListQuery {
    pub musicbrainz_id: Option<String>,
}

/// Query parameters for listing albums
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlbumListQuery {
    pub artist_id: Option<i64>,
}

/// Query parameters for listing tracks
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrackListQuery {
    pub album_id: Option<i64>,
    pub artist_id: Option<i64>,
}

/// Query parameters for artist lookup
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupQuery {
    pub term: String,
}

/// Create artist router
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_artists).post(create_artist))
        .route(
            "/{id}",
            get(get_artist).put(update_artist).delete(delete_artist),
        )
        .route("/{id}/refresh", post(refresh_artist))
        .route("/{id}/rescan", post(rescan_artist))
        .route("/lookup", get(lookup_artist))
}

/// Create album router
pub fn album_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_albums))
        .route("/{id}", get(get_album).put(update_album))
        .route("/{id}/rescan", post(rescan_album))
        .route("/{id}/rename", post(rename_album_files))
}

/// Create track router
pub fn track_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_tracks))
        .route("/{id}", get(get_track))
        .route("/{id}/lyrics", get(get_track_lyrics))
}

/// Lyrics query params
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LyricsQuery {
    pub artist: Option<String>,
}

/// Lyrics cache directory
const LYRICS_CACHE_DIR: &str = "cache/lyrics";

/// Get lyrics for a track — checks disk cache first, then fetches from lyrics.ovh
async fn get_track_lyrics(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<LyricsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let track_repo = TrackRepository::new(state.db.clone());
    let track = track_repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch track: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Get artist name from query param or look up from DB
    let artist_name = if let Some(name) = query.artist {
        name
    } else {
        let artist_repo = ArtistRepository::new(state.db.clone());
        match artist_repo.get_by_id(track.artist_id).await {
            Ok(Some(a)) => a.name,
            _ => return Err(ApiError::Internal("Artist not found".to_string())),
        }
    };

    // Check disk cache first: cache/lyrics/{track_id}.lyric
    let cache_dir = std::path::Path::new(LYRICS_CACHE_DIR);
    let cache_file = cache_dir.join(format!("{}.lyric", id));

    if cache_file.exists() {
        let cached = tokio::fs::read_to_string(&cache_file).await.unwrap_or_default();
        // Empty file = "not found" sentinel (avoid re-fetching)
        let lyrics = if cached.is_empty() { None } else { Some(cached) };
        return Ok(Json(serde_json::json!({
            "trackId": id,
            "artist": artist_name,
            "title": track.title,
            "lyrics": lyrics,
        })));
    }

    // Cache miss — fetch from lyrics.ovh
    let url = format!(
        "https://api.lyrics.ovh/v1/{}/{}",
        urlencoding::encode(&artist_name),
        urlencoding::encode(&track.title)
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let lyrics = match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
            body.get("lyrics")
                .and_then(|l| l.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        }
        _ => None,
    };

    // Cache to disk (fire and forget) — empty file for "not found" to avoid re-fetch
    let cache_content = lyrics.clone().unwrap_or_default();
    let cache_file_clone = cache_file.clone();
    tokio::spawn(async move {
        let _ = tokio::fs::create_dir_all(LYRICS_CACHE_DIR).await;
        let _ = tokio::fs::write(&cache_file_clone, &cache_content).await;
    });

    Ok(Json(serde_json::json!({
        "trackId": id,
        "artist": artist_name,
        "title": track.title,
        "lyrics": lyrics,
    })))
}

// ============================================================================
// Artist endpoints
// ============================================================================

/// List all artists (or filter by musicbrainzId)
async fn list_artists(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArtistListQuery>,
) -> Result<Json<Vec<ArtistResponse>>, ApiError> {
    let repo = ArtistRepository::new(state.db.clone());

    // If musicbrainzId filter is provided, return just that artist
    if let Some(mbid) = query.musicbrainz_id {
        let artist = repo
            .get_by_musicbrainz_id(&mbid)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch artist: {}", e)))?;
        return match artist {
            Some(a) => {
                let mut response = ArtistResponse::from(a);
                enrich_artist_response(&mut response, &state.db).await;
                Ok(Json(vec![response]))
            }
            None => Ok(Json(vec![])),
        };
    }

    let db_artists = repo
        .get_all()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch artists: {}", e)))?;

    // Bulk-load artist statistics in a single query (avoids N+1)
    let stats_map = bulk_load_artist_stats(&state.db).await;

    let artists: Vec<ArtistResponse> = db_artists
        .into_iter()
        .map(|a| {
            let mut response = ArtistResponse::from(a);
            if let Some(stats) = stats_map.get(&response.id) {
                response.statistics = Some(stats.clone());
            }
            response
        })
        .collect();

    Ok(Json(artists))
}

/// Get a single artist by ID
async fn get_artist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ArtistResponse>, ApiError> {
    let repo = ArtistRepository::new(state.db.clone());

    let artist = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch artist: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let mut response = ArtistResponse::from(artist);
    enrich_artist_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Bulk-load artist statistics for all artists in a single query
async fn bulk_load_artist_stats(
    db: &crate::core::datastore::Database,
) -> std::collections::HashMap<i64, ArtistStatistics> {
    use sqlx::Row;

    let pool = db.pool();

    // Album counts per artist
    let album_rows = sqlx::query(
        "SELECT artist_id, COUNT(*)::bigint as album_count FROM albums GROUP BY artist_id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut album_counts: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for row in &album_rows {
        if let (Ok(id), Ok(count)) = (
            row.try_get::<i64, _>("artist_id"),
            row.try_get::<i64, _>("album_count"),
        ) {
            album_counts.insert(id, count);
        }
    }

    // Track counts per artist
    let track_rows = sqlx::query(
        "SELECT artist_id, COUNT(*)::bigint as track_count, SUM(CASE WHEN has_file THEN 1 ELSE 0 END)::bigint as track_file_count FROM tracks GROUP BY artist_id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut track_counts: std::collections::HashMap<i64, (i64, i64)> =
        std::collections::HashMap::new();
    for row in &track_rows {
        if let (Ok(id), Ok(count), Ok(file_count)) = (
            row.try_get::<i64, _>("artist_id"),
            row.try_get::<i64, _>("track_count"),
            row.try_get::<i64, _>("track_file_count"),
        ) {
            track_counts.insert(id, (count, file_count));
        }
    }

    // Size on disk per artist
    let size_rows = sqlx::query(
        "SELECT artist_id, COALESCE(SUM(size), 0)::bigint as size_on_disk FROM track_files GROUP BY artist_id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut size_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for row in &size_rows {
        if let (Ok(id), Ok(size)) = (
            row.try_get::<i64, _>("artist_id"),
            row.try_get::<i64, _>("size_on_disk"),
        ) {
            size_map.insert(id, size);
        }
    }

    // Combine into ArtistStatistics
    let mut all_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    all_ids.extend(album_counts.keys());
    all_ids.extend(track_counts.keys());
    all_ids.extend(size_map.keys());

    let mut result = std::collections::HashMap::new();
    for id in all_ids {
        let album_count = album_counts.get(&id).copied().unwrap_or(0);
        let (track_count, track_file_count) = track_counts.get(&id).copied().unwrap_or((0, 0));
        let size_on_disk = size_map.get(&id).copied().unwrap_or(0);
        let percent_of_tracks = if track_count > 0 {
            (track_file_count as f64 / track_count as f64) * 100.0
        } else {
            0.0
        };

        result.insert(
            id,
            ArtistStatistics {
                album_count,
                track_count,
                track_file_count,
                size_on_disk,
                percent_of_tracks,
            },
        );
    }

    result
}

/// Enrich an ArtistResponse with statistics from the database
async fn enrich_artist_response(
    response: &mut ArtistResponse,
    db: &crate::core::datastore::Database,
) {
    use sqlx::Row;

    let pool = db.pool();

    let album_count: i64 = sqlx::query("SELECT COUNT(*)::bigint as cnt FROM albums WHERE artist_id = $1")
        .bind(response.id)
        .fetch_one(pool)
        .await
        .and_then(|row| row.try_get("cnt").map_err(Into::into))
        .unwrap_or(0);

    let (track_count, track_file_count): (i64, i64) = sqlx::query(
        "SELECT COUNT(*)::bigint as cnt, SUM(CASE WHEN has_file THEN 1 ELSE 0 END)::bigint as file_cnt FROM tracks WHERE artist_id = $1",
    )
    .bind(response.id)
    .fetch_one(pool)
    .await
    .map(|row| {
        let cnt: i64 = row.try_get("cnt").unwrap_or(0);
        let file_cnt: i64 = row.try_get("file_cnt").unwrap_or(0);
        (cnt, file_cnt)
    })
    .unwrap_or((0, 0));

    let size_on_disk: i64 = sqlx::query(
        "SELECT COALESCE(SUM(size), 0)::bigint as size_on_disk FROM track_files WHERE artist_id = $1",
    )
    .bind(response.id)
    .fetch_one(pool)
    .await
    .and_then(|row| row.try_get("size_on_disk").map_err(Into::into))
    .unwrap_or(0);

    let percent_of_tracks = if track_count > 0 {
        (track_file_count as f64 / track_count as f64) * 100.0
    } else {
        0.0
    };

    response.statistics = Some(ArtistStatistics {
        album_count,
        track_count,
        track_file_count,
        size_on_disk,
        percent_of_tracks,
    });
}

/// Create a new artist
async fn create_artist(
    State(state): State<Arc<AppState>>,
    Json(options): Json<CreateArtistRequest>,
) -> Result<Json<ArtistResponse>, ApiError> {
    options.validate()?;

    let repo = ArtistRepository::new(state.db.clone());

    // Check if artist already exists by musicbrainzId
    if let Some(ref mbid) = options.musicbrainz_id {
        if let Some(_existing) = repo
            .get_by_musicbrainz_id(mbid)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        {
            return Err(ApiError::Validation("Artist already exists".to_string()));
        }
    }

    let clean = clean_title(&options.name);
    let sort = clean.clone();
    let slug = generate_slug(&options.name);

    let full_path = options.get_full_path();
    let root_folder_path = options.get_root_folder_path();

    let genres_json =
        serde_json::to_string(&options.genres).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(&options.tags).unwrap_or_else(|_| "[]".to_string());
    let images_json =
        serde_json::to_string(&options.images).unwrap_or_else(|_| "[]".to_string());

    let db_artist = ArtistDbModel {
        id: 0,
        musicbrainz_id: options.musicbrainz_id.clone(),
        name: options.name.clone(),
        clean_name: clean,
        sort_name: sort,
        overview: options.overview.clone(),
        artist_type: options.artist_type.clone().unwrap_or_else(|| "group".to_string()),
        status: options.status.clone().unwrap_or_else(|| "continuing".to_string()),
        genres: genres_json,
        images: images_json,
        tags: tags_json,
        path: full_path.clone(),
        root_folder_path,
        quality_profile_id: options.quality_profile_id,
        monitored: options.monitored,
        added: Utc::now(),
        last_info_sync: None,
        title_slug: slug,
    };

    let id = repo
        .insert(&db_artist)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create artist: {}", e)))?;

    tracing::info!("Created artist: id={}, name={}", id, options.name);

    // Create the artist folder on disk
    if !full_path.is_empty() {
        let path = std::path::Path::new(&full_path);
        if !path.exists() {
            match tokio::fs::create_dir_all(path).await {
                Ok(()) => tracing::info!("Created artist folder: {}", full_path),
                Err(e) => tracing::warn!("Failed to create artist folder {}: {}", full_path, e),
            }
        }
    }

    // Auto-fetch albums from MusicBrainz when searchForAlbums is requested
    let should_search = options.add_options.as_ref().map_or(false, |o| o.search_for_albums);
    if should_search {
        if let Some(ref mbid) = options.musicbrainz_id {
            upsert_albums_from_musicbrainz(id, mbid, &state).await;
        }
    }

    let mut created = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch created artist: {}", e)))?
        .ok_or(ApiError::Internal(
            "Artist not found after creation".to_string(),
        ))?;

    // Re-fetch images with the real artist_id so URLs point to /MediaCover/Artists/{id}/
    // (lookup stored remote fanart URLs because artist_id didn't exist yet)
    if let Some(ref mbid) = created.musicbrainz_id {
        let images = fetch_fanart_artist_images(mbid, Some(id)).await;
        if !images.is_empty() {
            created.images =
                serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());
        }
    }

    // Mark as synced if we fetched albums
    if should_search {
        created.last_info_sync = Some(Utc::now());
    }
    let _ = repo.update(&created).await;

    let mut response = ArtistResponse::from(created);
    enrich_artist_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Update an existing artist
async fn update_artist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(update): Json<UpdateArtistRequest>,
) -> Result<Json<ArtistResponse>, ApiError> {
    let repo = ArtistRepository::new(state.db.clone());

    let mut artist = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch artist: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Apply updates
    if let Some(name) = update.name {
        artist.name = name.clone();
        artist.clean_name = clean_title(&name);
        artist.sort_name = artist.clean_name.clone();
    }
    if let Some(monitored) = update.monitored {
        artist.monitored = monitored;
    }
    if let Some(quality_profile_id) = update.quality_profile_id {
        artist.quality_profile_id = quality_profile_id;
    }
    if let Some(path) = update.path {
        artist.path = path;
    }
    if let Some(tags) = update.tags {
        artist.tags = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    }

    repo.update(&artist)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update artist: {}", e)))?;

    tracing::info!("Updated artist: id={}", id);

    let mut response = ArtistResponse::from(artist);
    enrich_artist_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Delete an artist
async fn delete_artist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<DeleteArtistQuery>,
) -> Result<(), ApiError> {
    let repo = ArtistRepository::new(state.db.clone());

    let artist = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch artist: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Delete files from disk if requested
    if params.delete_files {
        let artist_path = std::path::Path::new(&artist.path);
        if artist_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(artist_path) {
                tracing::error!("Failed to delete artist folder {}: {}", artist.path, e);
                // Continue with DB deletion even if file deletion fails
            } else {
                tracing::info!("Deleted artist folder: {}", artist.path);
            }
        }
    }

    // Delete track_files for this artist first (no FK cascade)
    let pool = state.db.pool();
    let _ = sqlx::query("DELETE FROM track_files WHERE artist_id = $1")
        .bind(id)
        .execute(pool)
        .await;

    // Delete from database (albums/tracks cascade)
    repo.delete(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete artist: {}", e)))?;

    tracing::info!(
        "Deleted artist: id={}, name={}, delete_files={}",
        id,
        artist.name,
        params.delete_files
    );

    Ok(())
}

/// Refresh artist metadata from MusicBrainz + Fanart.tv, and upsert albums
async fn refresh_artist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ArtistResponse>, ApiError> {
    let repo = ArtistRepository::new(state.db.clone());

    let mut artist = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch artist: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!("Refreshing artist: id={}, name={}", id, artist.name);

    // Refresh metadata from MusicBrainz + Fanart.tv if we have a MBID
    if let Some(mbid) = artist.musicbrainz_id.clone() {
        refresh_artist_metadata(&mut artist, &mbid, &state).await;
        upsert_albums_from_musicbrainz(artist.id, &mbid, &state).await;
    }

    // Populate canonical tracks from MusicBrainz (if not already populated)
    populate_tracks_from_musicbrainz(artist.id, &state).await;

    // Scan audio files on disk and link to existing tracks
    scan_artist_audio_files(artist.id, &artist.path, &state).await;

    artist.last_info_sync = Some(Utc::now());

    repo.update(&artist)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update artist: {}", e)))?;

    let mut response = ArtistResponse::from(artist);
    enrich_artist_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Fetch updated artist metadata from MusicBrainz and images from Fanart.tv
async fn refresh_artist_metadata(
    artist: &mut ArtistDbModel,
    mbid: &str,
    state: &AppState,
) {
    // Update metadata from MusicBrainz service
    if let Some(ref mb_client) = state.musicbrainz_client {
        match mb_client.get_artist(mbid).await {
            Ok(Some(mb_artist)) => {
                if let Some(overview) = mb_artist.overview {
                    artist.overview = Some(overview);
                }
                if let Some(artist_type) = mb_artist.artist_type {
                    artist.artist_type = artist_type;
                }
                if !mb_artist.genres.is_empty() {
                    artist.genres = serde_json::to_string(&mb_artist.genres)
                        .unwrap_or_else(|_| "[]".to_string());
                }
                tracing::info!("Updated artist metadata from MusicBrainz: {}", artist.name);
            }
            Ok(None) => {
                tracing::warn!("Artist not found in MusicBrainz: {}", mbid);
            }
            Err(e) => {
                tracing::warn!("Failed to fetch artist from MusicBrainz: {}", e);
            }
        }
    }

    // Refresh images from Fanart.tv (with local MediaCover URLs)
    let images = fetch_fanart_artist_images(mbid, Some(artist.id)).await;
    if !images.is_empty() {
        artist.images = serde_json::to_string(&images)
            .unwrap_or_else(|_| "[]".to_string());
        tracing::info!("Updated {} Fanart.tv images for artist: {}", images.len(), artist.name);
    }
}

/// Fetch albums from MusicBrainz and upsert into the database
async fn upsert_albums_from_musicbrainz(
    artist_id: i64,
    mbid: &str,
    state: &AppState,
) {
    let mb_client = match state.musicbrainz_client {
        Some(ref c) => c,
        None => return,
    };

    let mb_albums = match mb_client.get_artist_albums(mbid).await {
        Ok(albums) => albums,
        Err(e) => {
            tracing::warn!("Failed to fetch albums from MusicBrainz: {}", e);
            return;
        }
    };

    if mb_albums.is_empty() {
        tracing::info!("No albums found in MusicBrainz for artist MBID: {}", mbid);
        return;
    }

    let album_repo = AlbumRepository::new(state.db.clone());
    let mut inserted = 0;
    let mut updated = 0;

    for mb_album in &mb_albums {
        // Build cover art URL from cover-art-cache service (via nginx proxy)
        let cover_images = build_album_cover_images(&mb_album.mbid);
        let images_json = serde_json::to_string(&cover_images)
            .unwrap_or_else(|_| "[]".to_string());
        let genres_json = serde_json::to_string(&mb_album.genres)
            .unwrap_or_else(|_| "[]".to_string());
        let secondary_types_json = serde_json::to_string(&mb_album.secondary_types)
            .unwrap_or_else(|_| "[]".to_string());
        let release_date = mb_album.release_date.as_deref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        let album_type = mb_album.album_type.clone().unwrap_or_else(|| "Album".to_string());
        let clean = clean_title(&mb_album.title);

        // Auto-monitor: only studio Albums (no secondary types) are monitored by default
        // Compilations, Live, Soundtracks, Remixes etc. start unmonitored
        let should_monitor = album_type.eq_ignore_ascii_case("Album")
            && mb_album.secondary_types.is_empty();

        // Check if album already exists by MBID
        let existing = album_repo.get_by_musicbrainz_id(&mb_album.mbid).await.ok().flatten();

        if let Some(mut existing_album) = existing {
            // Update existing album metadata (preserve user's monitored state)
            existing_album.title = mb_album.title.clone();
            existing_album.clean_title = clean;
            existing_album.album_type = album_type;
            existing_album.secondary_types = secondary_types_json;
            existing_album.release_date = release_date;
            existing_album.genres = genres_json;
            existing_album.images = images_json;
            existing_album.last_info_sync = Some(Utc::now());

            if let Err(e) = album_repo.update(&existing_album).await {
                tracing::warn!("Failed to update album '{}': {}", mb_album.title, e);
            } else {
                updated += 1;
            }
        } else {
            // Insert new album
            let slug = generate_slug(&clean);
            let db_album = AlbumDbModel {
                id: 0,
                artist_id,
                musicbrainz_id: Some(mb_album.mbid.clone()),
                title: mb_album.title.clone(),
                clean_title: clean,
                album_type,
                secondary_types: secondary_types_json,
                release_date,
                genres: genres_json,
                images: images_json,
                monitored: should_monitor,
                added: Utc::now(),
                last_info_sync: Some(Utc::now()),
                title_slug: slug,
            };

            if let Err(e) = album_repo.insert(&db_album).await {
                tracing::warn!("Failed to insert album '{}': {}", mb_album.title, e);
            } else {
                inserted += 1;
            }
        }
    }

    tracing::info!(
        "Album sync complete for artist MBID {}: {} inserted, {} updated (of {} total)",
        mbid, inserted, updated, mb_albums.len()
    );
}

/// Populate canonical tracks for an artist's albums from MusicBrainz.
/// For each album, picks the best release (prefer US/XW, most tracks) and fetches its track listing.
async fn populate_tracks_from_musicbrainz(artist_id: i64, state: &AppState) {
    let mb_client = match state.musicbrainz_client {
        Some(ref c) => c,
        None => return,
    };

    let album_repo = AlbumRepository::new(state.db.clone());
    let track_repo = TrackRepository::new(state.db.clone());

    let albums = match album_repo.get_by_artist_id(artist_id).await {
        Ok(a) => a,
        Err(_) => return,
    };

    for album in &albums {
        let album_mbid = match &album.musicbrainz_id {
            Some(mbid) => mbid.clone(),
            None => continue,
        };

        // Check if this album already has tracks from MusicBrainz
        let existing_tracks = track_repo.get_by_album_id(album.id).await.unwrap_or_default();
        // Skip if we already have tracks with no files (canonical tracks already populated)
        // Only re-populate if there are zero tracks
        if !existing_tracks.is_empty() {
            continue;
        }

        // Get releases for this album, pick the best one
        let releases = match mb_client.get_album_releases(&album_mbid).await {
            Ok(r) => r,
            Err(_) => continue,
        };

        if releases.is_empty() {
            continue;
        }

        // Build a priority order: US first, then XW, then by track count desc
        let mut ordered_releases: Vec<&_> = Vec::with_capacity(releases.len());
        // US releases first
        ordered_releases.extend(releases.iter().filter(|r| r.country.as_deref() == Some("US")));
        // XW (worldwide) next
        ordered_releases.extend(releases.iter().filter(|r| r.country.as_deref() == Some("XW")));
        // Then the rest, sorted by track count descending
        let mut rest: Vec<&_> = releases
            .iter()
            .filter(|r| r.country.as_deref() != Some("US") && r.country.as_deref() != Some("XW"))
            .collect();
        rest.sort_by(|a, b| b.track_count.unwrap_or(0).cmp(&a.track_count.unwrap_or(0)));
        ordered_releases.extend(rest);
        ordered_releases.dedup_by_key(|r| &r.mbid);

        // Try releases in priority order until we find one with actual track data
        let mut mb_tracks = Vec::new();
        for release in &ordered_releases {
            match mb_client.get_release_tracks(&release.mbid).await {
                Ok(t) if !t.is_empty() => {
                    mb_tracks = t;
                    break;
                }
                _ => continue,
            }
        }

        if mb_tracks.is_empty() {
            continue;
        }

        let mut tracks_inserted = 0;
        for mb_track in &mb_tracks {
            let new_track = crate::core::datastore::models::TrackDbModel {
                id: 0,
                album_id: album.id,
                artist_id,
                title: mb_track.title.clone(),
                track_number: mb_track.position,
                disc_number: mb_track.disc_number,
                duration_ms: mb_track.length_ms,
                has_file: false,
                track_file_id: None,
                monitored: true,
                air_date_utc: None,
            };

            if track_repo.insert(&new_track).await.is_ok() {
                tracks_inserted += 1;
            }
        }

        if tracks_inserted > 0 {
            tracing::info!(
                "Populated {} canonical tracks for album '{}'",
                tracks_inserted,
                album.title,
            );
        }
    }
}

/// Scan an artist's album folders for audio files and create track/track_file records.
/// Returns (files_found, files_added).
async fn scan_artist_audio_files(artist_id: i64, artist_path: &str, state: &AppState) -> (usize, usize) {
    let album_repo = AlbumRepository::new(state.db.clone());
    let track_repo = TrackRepository::new(state.db.clone());
    let track_file_repo = TrackFileRepository::new(state.db.clone());

    let albums = match album_repo.get_by_artist_id(artist_id).await {
        Ok(a) => a,
        Err(_) => return (0, 0),
    };

    let artist_dir = std::path::Path::new(artist_path);
    let mut total_found = 0;
    let mut total_added = 0;

    for album in &albums {
        let album_folder = sanitize_filename::sanitize(&album.title);
        let album_path = artist_dir.join(&album_folder);

        if !album_path.is_dir() {
            continue;
        }

        let existing_files = track_file_repo.get_by_album_id(album.id).await.unwrap_or_default();
        let existing_file_paths: std::collections::HashSet<String> =
            existing_files.iter().map(|f| f.path.clone()).collect();
        let existing_tracks = track_repo.get_by_album_id(album.id).await.unwrap_or_default();

        let mut dir = match tokio::fs::read_dir(&album_path).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut audio_files = Vec::new();
        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                let filename = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                let size = entry.metadata().await.map(|m| m.len() as i64).unwrap_or(0);
                audio_files.push((path.to_string_lossy().to_string(), filename, size));
            }
        }
        audio_files.sort_by(|a, b| a.1.cmp(&b.1));
        total_found += audio_files.len();

        for (file_path, filename, size) in &audio_files {
            if existing_file_paths.contains(file_path) {
                continue;
            }
            let (track_num, title) = parse_track_filename(filename);

            // Try to match to an existing canonical track by track number
            // If no canonical track exists (MB didn't have data), create one from filename
            let track_id = if let Some(et) = existing_tracks.iter().find(|t| t.track_number == track_num && !t.has_file) {
                // Found a canonical track without a file — link to it
                et.id
            } else if existing_tracks.iter().any(|t| t.track_number == track_num) {
                // Track number already has a file — skip duplicate
                continue;
            } else {
                // No canonical track — create from filename (fallback)
                let new_track = crate::core::datastore::models::TrackDbModel {
                    id: 0,
                    album_id: album.id,
                    artist_id,
                    title: title.clone(),
                    track_number: track_num,
                    disc_number: 1,
                    duration_ms: None,
                    has_file: true,
                    track_file_id: None,
                    monitored: true,
                    air_date_utc: None,
                };
                match track_repo.insert(&new_track).await {
                    Ok(tid) => tid,
                    Err(e) => {
                        tracing::warn!("Failed to insert track '{}': {}", title, e);
                        continue;
                    }
                }
            };

            let relative_path = format!("{}/{}", album_folder, filename);
            let ext = std::path::Path::new(filename)
                .extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_else(|| "MP3".to_string());

            // Estimate bitrate from file size and MB track duration
            let matched_track = existing_tracks.iter().find(|t| t.track_number == track_num);
            let bitrate = matched_track
                .and_then(|t| t.duration_ms)
                .filter(|&d| d > 0)
                .map(|duration_ms| (*size * 8 / (duration_ms as i64 / 1000)) as i32 / 1000); // kbps

            let media_info = serde_json::json!({
                "audio_format": ext,
                "bitrate": bitrate,
            });

            let new_file = crate::core::datastore::models::TrackFileDbModel {
                id: 0,
                artist_id,
                album_id: album.id,
                relative_path,
                path: file_path.clone(),
                size: *size,
                quality: serde_json::json!({ "codec": ext }).to_string(),
                media_info: Some(media_info.to_string()),
                date_added: Utc::now(),
            };

            if let Ok(file_id) = track_file_repo.insert(&new_file).await {
                let pool = state.db.pool();
                let _ = sqlx::query("UPDATE tracks SET has_file = true, track_file_id = $1 WHERE id = $2")
                    .bind(file_id)
                    .bind(track_id)
                    .execute(pool)
                    .await;
                total_added += 1;
            }
        }
    }

    tracing::info!(
        "Audio scan for artist {}: {} files found, {} added",
        artist_id, total_found, total_added
    );
    (total_found, total_added)
}

/// Audio file extensions to scan
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "m4a", "ogg", "opus", "wav", "aac", "wma", "alac", "ape", "dsf", "dff",
];

/// Parse track number and title from a filename like "01 Courage 1.mp3" or "01 - Courage.flac"
pub fn parse_track_filename(filename: &str) -> (i32, String) {
    let stem = std::path::Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // Try to extract leading track number: "01 Title", "01 - Title", "01. Title"
    let re_result = stem
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();

    if re_result.is_empty() {
        return (0, stem);
    }

    let track_num = re_result.parse::<i32>().unwrap_or(0);
    let rest = stem[re_result.len()..].trim_start_matches([' ', '-', '.', '_']);

    // Strip trailing duplicate markers like " 1", " 2" at the end (e.g., "Courage 1")
    let title = rest
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .trim()
        .to_string();

    let title = if title.is_empty() {
        rest.to_string()
    } else {
        title
    };

    (track_num, title)
}

/// Rescan artist files — scans album folders for audio files and creates track/track_file records
async fn rescan_artist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = ArtistRepository::new(state.db.clone());

    let artist = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch artist: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!("Rescan requested for artist: id={}, name={}, path={}", id, artist.name, artist.path);

    let (total_found, total_added) = scan_artist_audio_files(id, &artist.path, &state).await;

    Ok(Json(serde_json::json!({
        "name": "RescanArtist",
        "commandName": "Rescan Artist",
        "message": format!("Rescan complete: {} files found, {} added", total_found, total_added),
        "body": { "artistId": id },
        "status": "completed"
    })))
}

/// Lookup artists via the MusicBrainz service
async fn lookup_artist(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LookupQuery>,
) -> Result<Json<Vec<ArtistLookupResult>>, ApiError> {
    tracing::info!("Artist lookup requested: term={}", query.term);

    // Query MusicBrainz service if configured
    if let Some(ref mb_client) = state.musicbrainz_client {
        match mb_client.search_artists(&query.term, 25).await {
            Ok(results) => {
                let mut lookup_results: Vec<ArtistLookupResult> = results
                    .into_iter()
                    .map(|mb| ArtistLookupResult {
                        name: mb.name,
                        sort_name: mb.sort_name.unwrap_or_default(),
                        musicbrainz_id: Some(mb.mbid),
                        overview: mb.overview,
                        disambiguation: mb.disambiguation,
                        artist_type: mb.artist_type.unwrap_or_else(|| "group".to_string()),
                        gender: mb.gender,
                        area: mb.area,
                        begin_date: mb.begin_date,
                        end_date: mb.end_date,
                        rating: mb.rating,
                        images: mb
                            .images
                            .into_iter()
                            .map(|img| ArtistImage {
                                cover_type: img.cover_type,
                                url: img.url,
                                remote_url: img.remote_url,
                            })
                            .collect(),
                        genres: mb.genres,
                        tags: mb.tags,
                    })
                    .collect();

                // Enrich top results with Fanart.tv images (limit to 5 to avoid rate limiting)
                let enrich_count = lookup_results.len().min(5);
                for i in 0..enrich_count {
                    if let Some(ref mbid) = lookup_results[i].musicbrainz_id {
                        let images = fetch_fanart_artist_images(mbid, None).await;
                        if !images.is_empty() {
                            lookup_results[i].images = images;
                        }
                    }
                }

                return Ok(Json(lookup_results));
            }
            Err(e) => {
                tracing::warn!("MusicBrainz lookup failed, returning empty results: {}", e);
            }
        }
    }

    // Fallback: return empty results if service is not configured or failed
    Ok(Json(vec![]))
}

// ============================================================================
// Album endpoints
// ============================================================================

/// List all albums (or filter by artistId)
async fn list_albums(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AlbumListQuery>,
) -> Result<Json<Vec<AlbumResponse>>, ApiError> {
    let repo = AlbumRepository::new(state.db.clone());

    let db_albums = if let Some(artist_id) = query.artist_id {
        repo.get_by_artist_id(artist_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch albums: {}", e)))?
    } else {
        repo.get_all()
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch albums: {}", e)))?
    };

    // Bulk-load album track stats
    let stats_map = bulk_load_album_stats(&state.db).await;

    let albums: Vec<AlbumResponse> = db_albums
        .into_iter()
        .map(|a| {
            let id = a.id;
            let mut response = AlbumResponse::from(a);
            if let Some(stats) = stats_map.get(&id) {
                response.statistics = Some(stats.clone());
            }
            response
        })
        .collect();

    Ok(Json(albums))
}

/// Bulk-load track statistics per album
async fn bulk_load_album_stats(
    db: &crate::core::datastore::Database,
) -> std::collections::HashMap<i64, AlbumStatistics> {
    use sqlx::Row;

    let pool = db.pool();
    let rows = sqlx::query(
        "SELECT album_id, COUNT(*)::bigint as cnt, SUM(CASE WHEN has_file THEN 1 ELSE 0 END)::bigint as file_cnt FROM tracks GROUP BY album_id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut result = std::collections::HashMap::new();
    for row in &rows {
        if let (Ok(album_id), Ok(total), Ok(with_files)) = (
            row.try_get::<i64, _>("album_id"),
            row.try_get::<i64, _>("cnt"),
            row.try_get::<i64, _>("file_cnt"),
        ) {
            result.insert(
                album_id,
                AlbumStatistics {
                    total_track_count: total,
                    track_file_count: with_files,
                    percent_of_tracks: if total > 0 {
                        (with_files as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    },
                },
            );
        }
    }

    result
}

/// Get a single album by ID (enriched with MusicBrainz metadata)
async fn get_album(
    State(state): State<Arc<AppState>>,
    Path(id_or_slug): Path<String>,
) -> Result<Json<AlbumResponse>, ApiError> {
    let repo = AlbumRepository::new(state.db.clone());

    // Try numeric id first, then slug lookup (needs artist context from query)
    let album = if let Ok(id) = id_or_slug.parse::<i64>() {
        repo.get_by_id(id).await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch album: {}", e)))?
            .ok_or(ApiError::NotFound)?
    } else {
        // Slug lookup — search all albums for this slug
        let all = sqlx::query_as::<_, AlbumDbModel>(
            "SELECT * FROM albums WHERE title_slug = $1 LIMIT 1",
        )
        .bind(&id_or_slug)
        .fetch_optional(state.db.pool())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch album: {}", e)))?
        .ok_or(ApiError::NotFound)?;
        all
    };

    let mut response = AlbumResponse::from(album);
    enrich_album_response(&mut response, &state.db).await;

    // Enrich with MusicBrainz release group metadata
    if let Some(ref mbid) = response.musicbrainz_id {
        if let Some(ref mb_client) = state.musicbrainz_client {
            if let Ok(Some(rg)) = mb_client.get_album(mbid).await {
                if let Some(album_type) = rg.album_type {
                    if !album_type.is_empty() {
                        response.album_type = album_type;
                    }
                }
                response.rating = rg.rating;
                response.rating_count = rg.rating_count;
                if !rg.tags.is_empty() {
                    response.tags = rg.tags;
                }
                if !rg.genres.is_empty() && response.genres.is_empty() {
                    response.genres = rg.genres;
                }
            }
        }
    }

    Ok(Json(response))
}

/// Update an album (toggle monitored, etc.)
async fn update_album(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(update): Json<UpdateAlbumRequest>,
) -> Result<Json<AlbumResponse>, ApiError> {
    let repo = AlbumRepository::new(state.db.clone());
    let artist_repo = ArtistRepository::new(state.db.clone());

    let mut album = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch album: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let was_unmonitored = !album.monitored;

    if let Some(monitored) = update.monitored {
        album.monitored = monitored;
    }

    repo.update(&album)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update album: {}", e)))?;

    // Create album directory on disk when monitoring is enabled
    if was_unmonitored && album.monitored {
        if let Ok(Some(artist)) = artist_repo.get_by_id(album.artist_id).await {
            let album_folder = sanitize_filename::sanitize(&album.title);
            let album_path = format!("{}/{}", artist.path, album_folder);
            let path = std::path::Path::new(&album_path);
            if !path.exists() {
                match tokio::fs::create_dir_all(path).await {
                    Ok(()) => tracing::info!("Created album folder: {}", album_path),
                    Err(e) => tracing::warn!("Failed to create album folder {}: {}", album_path, e),
                }
            }
        }
    }

    tracing::info!("Updated album: id={}, monitored={}", id, album.monitored);

    let mut response = AlbumResponse::from(album);
    enrich_album_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Query params for rename endpoint
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenameQuery {
    /// If true, return preview without renaming
    #[serde(default)]
    pub preview: bool,
}

/// Rename track files for an album using the pattern: "{track:00} - {title}.{ext}"
/// Pass ?preview=true to get a dry-run preview of changes
/// Rescan audio files for a single album
async fn rescan_album(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<AlbumResponse>, ApiError> {
    let album_repo = AlbumRepository::new(state.db.clone());
    let artist_repo = ArtistRepository::new(state.db.clone());

    let album = album_repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch album: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let artist = artist_repo
        .get_by_id(album.artist_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch artist: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!("Rescanning album: id={}, title='{}' by '{}'", id, album.title, artist.name);

    // Scan just this album's folder
    let artist_dir = std::path::Path::new(&artist.path);
    let album_folder = sanitize_filename::sanitize(&album.title);
    let album_path = artist_dir.join(&album_folder);

    if album_path.is_dir() {
        let track_repo = TrackRepository::new(state.db.clone());
        let track_file_repo = TrackFileRepository::new(state.db.clone());
        let existing_files = track_file_repo.get_by_album_id(album.id).await.unwrap_or_default();
        let existing_file_paths: std::collections::HashSet<String> =
            existing_files.iter().map(|f| f.path.clone()).collect();
        let existing_tracks = track_repo.get_by_album_id(album.id).await.unwrap_or_default();

        if let Ok(mut dir) = tokio::fs::read_dir(&album_path).await {
            let mut audio_files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                let path = entry.path();
                if !path.is_file() { continue; }
                let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                    let filename = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    let size = entry.metadata().await.map(|m| m.len() as i64).unwrap_or(0);
                    audio_files.push((path.to_string_lossy().to_string(), filename, size));
                }
            }
            audio_files.sort_by(|a, b| a.1.cmp(&b.1));

            let mut added = 0;
            for (file_path, filename, size) in &audio_files {
                if existing_file_paths.contains(file_path) { continue; }
                let (track_num, title) = parse_track_filename(filename);

                let track_id = if let Some(et) = existing_tracks.iter().find(|t| t.track_number == track_num && !t.has_file) {
                    et.id
                } else if existing_tracks.iter().any(|t| t.track_number == track_num) {
                    continue;
                } else {
                    let new_track = crate::core::datastore::models::TrackDbModel {
                        id: 0, album_id: album.id, artist_id: album.artist_id,
                        title: title.clone(), track_number: track_num, disc_number: 1,
                        duration_ms: None, has_file: true, track_file_id: None,
                        monitored: true, air_date_utc: None,
                    };
                    match track_repo.insert(&new_track).await {
                        Ok(tid) => tid,
                        Err(_) => continue,
                    }
                };

                let relative_path = format!("{}/{}", album_folder, filename);
                let ext = std::path::Path::new(filename).extension()
                    .map(|e| e.to_string_lossy().to_uppercase())
                    .unwrap_or_else(|| "MP3".to_string());

                let matched_track = existing_tracks.iter().find(|t| t.track_number == track_num);
                let bitrate = matched_track.and_then(|t| t.duration_ms).filter(|&d| d > 0)
                    .map(|duration_ms| (*size * 8 / (duration_ms as i64 / 1000)) as i32 / 1000);

                let media_info = serde_json::json!({ "audio_format": ext, "bitrate": bitrate });

                let new_file = crate::core::datastore::models::TrackFileDbModel {
                    id: 0, artist_id: album.artist_id, album_id: album.id,
                    relative_path, path: file_path.clone(), size: *size,
                    quality: serde_json::json!({ "codec": ext }).to_string(),
                    media_info: Some(media_info.to_string()), date_added: Utc::now(),
                };

                if let Ok(file_id) = track_file_repo.insert(&new_file).await {
                    let pool = state.db.pool();
                    let _ = sqlx::query("UPDATE tracks SET has_file = true, track_file_id = $1 WHERE id = $2")
                        .bind(file_id).bind(track_id).execute(pool).await;
                    added += 1;
                }
            }
            tracing::info!("Album rescan complete: {} files found, {} added for '{}'", audio_files.len(), added, album.title);
        }
    } else {
        tracing::info!("Album folder not found: {}", album_path.display());
    }

    // Return updated album response
    let updated = album_repo.get_by_id(id).await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch album: {}", e)))?
        .ok_or(ApiError::NotFound)?;
    let mut response = AlbumResponse::from(updated);
    enrich_album_response(&mut response, &state.db).await;
    Ok(Json(response))
}

async fn rename_album_files(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(rename_query): Query<RenameQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let album_repo = AlbumRepository::new(state.db.clone());
    let track_repo = TrackRepository::new(state.db.clone());
    let track_file_repo = TrackFileRepository::new(state.db.clone());

    let album = album_repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch album: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let tracks = track_repo
        .get_by_album_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch tracks: {}", e)))?;

    let files = track_file_repo
        .get_by_album_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch track files: {}", e)))?;

    // Build rename plan
    let mut changes: Vec<serde_json::Value> = Vec::new();

    for track in &tracks {
        if !track.has_file {
            continue;
        }

        let file = match files.iter().find(|f| Some(f.id) == track.track_file_id) {
            Some(f) => f,
            None => continue,
        };

        let old_path = std::path::Path::new(&file.path);
        let old_filename = old_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let ext = old_path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "mp3".to_string());

        let clean_title = sanitize_filename::sanitize(&track.title);
        let new_filename = format!("{:02} - {}.{}", track.track_number, clean_title, ext);

        if old_filename == new_filename {
            continue;
        }

        changes.push(serde_json::json!({
            "trackId": track.id,
            "fileId": file.id,
            "existingFilename": old_filename,
            "newFilename": new_filename,
            "existingPath": file.path,
        }));
    }

    // Preview mode — just return the plan
    if rename_query.preview {
        return Ok(Json(serde_json::json!({
            "changes": changes,
            "totalChanges": changes.len(),
        })));
    }

    // Execute renames
    let mut renamed = 0;
    for change in &changes {
        let old_path_str = change["existingPath"].as_str().unwrap_or("");
        let new_filename = change["newFilename"].as_str().unwrap_or("");
        let file_id = change["fileId"].as_i64().unwrap_or(0);

        let old_path = std::path::Path::new(old_path_str);
        let parent = match old_path.parent() {
            Some(p) => p,
            None => continue,
        };
        let new_path = parent.join(new_filename);

        match tokio::fs::rename(&old_path, &new_path).await {
            Ok(()) => {
                let new_path_str = new_path.to_string_lossy().to_string();
                let new_relative = format!(
                    "{}/{}",
                    sanitize_filename::sanitize(&album.title),
                    new_filename
                );
                let pool = state.db.pool();
                let _ = sqlx::query(
                    "UPDATE track_files SET path = $1, relative_path = $2 WHERE id = $3",
                )
                .bind(&new_path_str)
                .bind(&new_relative)
                .bind(file_id)
                .execute(pool)
                .await;
                renamed += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to rename '{}': {}", old_path_str, e);
            }
        }
    }

    Ok(Json(serde_json::json!({
        "message": format!("{} files renamed", renamed),
        "renamed": renamed,
    })))
}

/// Enrich an AlbumResponse with track statistics
async fn enrich_album_response(
    response: &mut AlbumResponse,
    db: &crate::core::datastore::Database,
) {
    use sqlx::Row;

    let pool = db.pool();

    let row = sqlx::query(
        "SELECT COUNT(*)::bigint as cnt, SUM(CASE WHEN has_file THEN 1 ELSE 0 END)::bigint as file_cnt FROM tracks WHERE album_id = $1",
    )
    .bind(response.id)
    .fetch_one(pool)
    .await;

    if let Ok(row) = row {
        let total: i64 = row.try_get("cnt").unwrap_or(0);
        let with_files: i64 = row.try_get("file_cnt").unwrap_or(0);
        response.statistics = Some(AlbumStatistics {
            total_track_count: total,
            track_file_count: with_files,
            percent_of_tracks: if total > 0 {
                (with_files as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        });
    }
}

/// Build cover art image URLs for an album using the cover-art-cache service
fn build_album_cover_images(release_group_mbid: &str) -> Vec<ArtistImage> {
    vec![ArtistImage {
        cover_type: "poster".to_string(),
        url: format!("/coverart/release-group/{}/front-500", release_group_mbid),
        remote_url: Some(format!(
            "https://coverartarchive.org/release-group/{}/front-500",
            release_group_mbid
        )),
    }]
}

// ============================================================================
// Track endpoints
// ============================================================================

/// List tracks (filter by albumId or artistId), enriched with file metadata
async fn list_tracks(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrackListQuery>,
) -> Result<Json<Vec<TrackResponse>>, ApiError> {
    let repo = TrackRepository::new(state.db.clone());
    let file_repo = TrackFileRepository::new(state.db.clone());

    let db_tracks = if let Some(album_id) = query.album_id {
        repo.get_by_album_id(album_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch tracks: {}", e)))?
    } else if let Some(artist_id) = query.artist_id {
        repo.get_by_artist_id(artist_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch tracks: {}", e)))?
    } else {
        return Err(ApiError::Validation(
            "albumId or artistId query parameter required".to_string(),
        ));
    };

    // Load all track files for this album/artist to join with tracks
    let files = if let Some(album_id) = query.album_id {
        file_repo.get_by_album_id(album_id).await.unwrap_or_default()
    } else if let Some(artist_id) = query.artist_id {
        file_repo.get_by_artist_id(artist_id).await.unwrap_or_default()
    } else {
        vec![]
    };

    let file_map: std::collections::HashMap<i64, &TrackFileDbModel> =
        files.iter().map(|f| (f.id, f)).collect();

    let tracks: Vec<TrackResponse> = db_tracks
        .into_iter()
        .map(|t| {
            let file = t.track_file_id.and_then(|fid| file_map.get(&fid).copied());
            TrackResponse::from_with_file(t, file)
        })
        .collect();

    Ok(Json(tracks))
}

/// Get a single track by ID
async fn get_track(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<TrackResponse>, ApiError> {
    let repo = TrackRepository::new(state.db.clone());

    let track = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch track: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(TrackResponse::from(track)))
}

// ============================================================================
// Request/Response types
// ============================================================================

/// Request to create a new artist
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArtistRequest {
    pub name: String,
    pub musicbrainz_id: Option<String>,
    pub quality_profile_id: i64,
    #[serde(default)]
    pub root_folder_path: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub monitored: bool,
    #[serde(default)]
    pub tags: Vec<i64>,
    // Optional fields
    pub overview: Option<String>,
    pub artist_type: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub images: Vec<ArtistImage>,
    #[serde(default)]
    pub add_options: Option<AddArtistOptions>,
}

/// Options for artist creation behavior
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddArtistOptions {
    #[serde(default)]
    pub search_for_albums: bool,
}

impl CreateArtistRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.name.is_empty() {
            return Err(ApiError::Validation("name is required".to_string()));
        }
        if self.quality_profile_id <= 0 {
            return Err(ApiError::Validation(
                "qualityProfileId is required".to_string(),
            ));
        }
        if self.root_folder_path.is_none() && self.path.is_none() {
            return Err(ApiError::Validation(
                "rootFolderPath or path is required".to_string(),
            ));
        }
        Ok(())
    }

    fn get_full_path(&self) -> String {
        if let Some(ref path) = self.path {
            path.clone()
        } else if let Some(ref root) = self.root_folder_path {
            let folder = sanitize_filename::sanitize(&self.name);
            format!(
                "{}/{}",
                root.trim_end_matches('/'),
                folder
            )
        } else {
            String::new()
        }
    }

    fn get_root_folder_path(&self) -> String {
        if let Some(ref root) = self.root_folder_path {
            root.clone()
        } else if let Some(ref path) = self.path {
            // Extract root from full path
            std::path::Path::new(path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        }
    }
}

/// Request to update an existing artist
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArtistRequest {
    pub name: Option<String>,
    pub monitored: Option<bool>,
    pub quality_profile_id: Option<i64>,
    pub path: Option<String>,
    pub tags: Option<Vec<i64>>,
}

/// Query parameters for deleting an artist
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeleteArtistQuery {
    #[serde(default)]
    pub delete_files: bool,
}

/// Artist statistics
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistStatistics {
    pub album_count: i64,
    pub track_count: i64,
    pub track_file_count: i64,
    pub size_on_disk: i64,
    pub percent_of_tracks: f64,
}

/// Image type for artist/album responses
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArtistImage {
    pub cover_type: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
}

/// Full artist response
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArtistResponse {
    pub id: i64,
    pub name: String,
    /// Alias for `name` — frontend Artist interface expects `title` (matching Series shape)
    pub title: String,
    pub sort_name: String,
    /// Alias for `sortName` — frontend uses `sortTitle` for sorting
    pub sort_title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    pub artist_type: String,
    pub images: Vec<ArtistImage>,
    pub path: String,
    pub quality_profile_id: i64,
    pub monitored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicbrainz_id: Option<String>,
    pub clean_name: String,
    pub title_slug: String,
    pub root_folder_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<i64>,
    pub added: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<ArtistStatistics>,
}

impl From<ArtistDbModel> for ArtistResponse {
    fn from(a: ArtistDbModel) -> Self {
        let genres: Vec<String> = serde_json::from_str(&a.genres).unwrap_or_default();
        let tags: Vec<i64> = serde_json::from_str(&a.tags).unwrap_or_default();
        let images: Vec<ArtistImage> = serde_json::from_str(&a.images).unwrap_or_default();

        let folder = a.path.split('/').next_back().map(|f| f.to_string());

        Self {
            id: a.id,
            title: a.name.clone(),
            sort_title: a.sort_name.clone(),
            name: a.name,
            sort_name: a.sort_name,
            status: a.status,
            overview: a.overview,
            artist_type: a.artist_type,
            images,
            path: a.path,
            quality_profile_id: a.quality_profile_id,
            monitored: a.monitored,
            musicbrainz_id: a.musicbrainz_id,
            clean_name: a.clean_name,
            title_slug: a.title_slug,
            root_folder_path: a.root_folder_path,
            folder,
            genres,
            tags,
            added: a.added.to_rfc3339(),
            statistics: None,
        }
    }
}

/// Request to update an album
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAlbumRequest {
    pub monitored: Option<bool>,
}

/// Album statistics
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumStatistics {
    pub total_track_count: i64,
    pub track_file_count: i64,
    pub percent_of_tracks: f64,
}

/// Album response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumResponse {
    pub id: i64,
    pub artist_id: i64,
    pub title: String,
    pub clean_title: String,
    pub title_slug: String,
    pub album_type: String,
    pub secondary_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    pub genres: Vec<String>,
    pub images: Vec<ArtistImage>,
    pub monitored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub musicbrainz_id: Option<String>,
    pub added: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<AlbumStatistics>,
    // MusicBrainz enrichment (populated on single album GET)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
}

impl From<AlbumDbModel> for AlbumResponse {
    fn from(a: AlbumDbModel) -> Self {
        let genres: Vec<String> = serde_json::from_str(&a.genres).unwrap_or_default();
        let images: Vec<ArtistImage> = serde_json::from_str(&a.images).unwrap_or_default();
        let secondary_types: Vec<String> =
            serde_json::from_str(&a.secondary_types).unwrap_or_default();

        Self {
            id: a.id,
            artist_id: a.artist_id,
            title: a.title,
            clean_title: a.clean_title.clone(),
            title_slug: if a.title_slug.is_empty() {
                generate_slug(&a.clean_title)
            } else {
                a.title_slug
            },
            album_type: a.album_type,
            secondary_types,
            release_date: a.release_date.map(|d| d.to_string()),
            genres,
            images,
            monitored: a.monitored,
            musicbrainz_id: a.musicbrainz_id,
            added: a.added.to_rfc3339(),
            statistics: None,
            rating: None,
            rating_count: None,
            tags: vec![],
            overview: None,
        }
    }
}

/// Track file info embedded in track response
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrackFileInfo {
    pub id: i64,
    pub path: String,
    pub relative_path: String,
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<i32>,
    pub date_added: String,
}

/// Track response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackResponse {
    pub id: i64,
    pub album_id: i64,
    pub artist_id: i64,
    pub title: String,
    pub track_number: i32,
    pub disc_number: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i32>,
    pub has_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_file_id: Option<i64>,
    pub monitored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_file: Option<TrackFileInfo>,
}

impl TrackResponse {
    fn from_with_file(t: TrackDbModel, file: Option<&TrackFileDbModel>) -> Self {
        let track_file = file.map(|f| {
            // Parse audio metadata from media_info JSON
            let media: serde_json::Value = f
                .media_info
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({}));

            let audio_format = f.path.rsplit('.').next().map(|e| e.to_uppercase());
            let bitrate = media.get("bitrate").and_then(|v| v.as_i64()).map(|v| v as i32);
            let sample_rate = media.get("sample_rate").and_then(|v| v.as_i64()).map(|v| v as i32);
            let channels = media.get("channels").and_then(|v| v.as_i64()).map(|v| v as i32);

            TrackFileInfo {
                id: f.id,
                path: f.path.clone(),
                relative_path: f.relative_path.clone(),
                size: f.size,
                audio_format,
                bitrate,
                sample_rate,
                channels,
                date_added: f.date_added.to_rfc3339(),
            }
        });

        Self {
            id: t.id,
            album_id: t.album_id,
            artist_id: t.artist_id,
            title: t.title,
            track_number: t.track_number,
            disc_number: t.disc_number,
            duration_ms: t.duration_ms,
            has_file: t.has_file,
            track_file_id: t.track_file_id,
            monitored: t.monitored,
            track_file,
        }
    }
}

impl From<TrackDbModel> for TrackResponse {
    fn from(t: TrackDbModel) -> Self {
        Self {
            id: t.id,
            album_id: t.album_id,
            artist_id: t.artist_id,
            title: t.title,
            track_number: t.track_number,
            disc_number: t.disc_number,
            duration_ms: t.duration_ms,
            has_file: t.has_file,
            track_file_id: t.track_file_id,
            monitored: t.monitored,
            track_file: None,
        }
    }
}

/// Artist lookup result (from external metadata source)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistLookupResult {
    pub name: String,
    pub sort_name: String,
    pub musicbrainz_id: Option<String>,
    pub overview: Option<String>,
    pub disambiguation: Option<String>,
    pub artist_type: String,
    pub gender: Option<String>,
    pub area: Option<String>,
    pub begin_date: Option<String>,
    pub end_date: Option<String>,
    pub rating: Option<f64>,
    pub images: Vec<ArtistImage>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
}

// ── Fanart.tv music artist images ──────────────────────────────────

/// Fanart.tv API key (shared with movies module)
fn fanart_api_key() -> Option<&'static str> {
    static KEY: once_cell::sync::Lazy<Option<String>> =
        once_cell::sync::Lazy::new(|| std::env::var("FANART_API").ok().filter(|s| !s.is_empty()));
    KEY.as_deref()
}

/// Fanart.tv music artist response
#[derive(serde::Deserialize, Default)]
struct FanartMusicResponse {
    #[serde(default)]
    artistthumb: Vec<FanartMusicImage>,
    #[serde(default)]
    artistbackground: Vec<FanartMusicImage>,
    #[serde(default)]
    hdmusiclogo: Vec<FanartMusicImage>,
    #[serde(default)]
    musicbanner: Vec<FanartMusicImage>,
}

#[derive(serde::Deserialize)]
struct FanartMusicImage {
    url: String,
    #[serde(default)]
    likes: String,
}

/// Artist image cache directory
const ARTIST_IMAGE_CACHE_DIR: &str = "cache/artist_images";
/// Max age before re-fetching (30 days)
const ARTIST_IMAGE_CACHE_DAYS: u64 = 30;

/// Fetch artist images from Fanart.tv using MusicBrainz ID.
/// Results are cached to a JSON file per artist for 30 days.
/// `artist_id` is used to generate local `/MediaCover/Artists/{id}/` URLs.
pub async fn fetch_fanart_artist_images(mbid: &str, artist_id: Option<i64>) -> Vec<ArtistImage> {
    // Check disk cache first — but only if we don't need to rewrite URLs.
    // When artist_id is provided, we need local /MediaCover/ URLs, so if the
    // cache has remote URLs we must re-fetch from Fanart.tv.
    let cache_dir = std::path::Path::new(ARTIST_IMAGE_CACHE_DIR);
    let cache_file = cache_dir.join(format!("{}.json", mbid));

    if let Ok(metadata) = tokio::fs::metadata(&cache_file).await {
        if let Ok(modified) = metadata.modified() {
            let age = modified.elapsed().unwrap_or(std::time::Duration::MAX);
            if age < std::time::Duration::from_secs(ARTIST_IMAGE_CACHE_DAYS * 86400) {
                if let Ok(data) = tokio::fs::read_to_string(&cache_file).await {
                    if let Ok(cached) = serde_json::from_str::<Vec<ArtistImage>>(&data) {
                        // If artist_id is provided, check if cached URLs are already local
                        let needs_rewrite = artist_id.is_some()
                            && cached.iter().any(|i| i.url.starts_with("http"));
                        if !needs_rewrite {
                            return cached;
                        }
                        // Fall through to re-fetch and rewrite with local URLs
                    }
                }
            }
        }
    }

    let api_key = match fanart_api_key() {
        Some(k) => k,
        None => return vec![],
    };

    let url = format!(
        "https://webservice.fanart.tv/v3/music/{}?api_key={}",
        mbid, api_key
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let response = match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return vec![],
    };

    let data: FanartMusicResponse = match response.json().await {
        Ok(d) => d,
        Err(_) => return vec![],
    };

    let mut images = Vec::new();

    let types: &[(&str, &[FanartMusicImage])] = &[
        ("poster", &data.artistthumb),
        ("fanart", &data.artistbackground),
        ("logo", &data.hdmusiclogo),
        ("banner", &data.musicbanner),
    ];

    for &(cover_type, source) in types {
        if let Some(best) = source.iter().max_by_key(|i| i.likes.parse::<i32>().unwrap_or(0)) {
            let ext = if best.url.ends_with(".png") { "png" } else { "jpg" };
            let local_url = match artist_id {
                Some(id) => format!("/MediaCover/Artists/{}/{}.{}", id, cover_type, ext),
                None => best.url.clone(),
            };
            images.push(ArtistImage {
                cover_type: cover_type.to_string(),
                url: local_url,
                remote_url: Some(best.url.clone()),
            });
        }
    }

    // Cache to disk
    if !images.is_empty() {
        let _ = tokio::fs::create_dir_all(cache_dir).await;
        if let Ok(json) = serde_json::to_string(&images) {
            let _ = tokio::fs::write(&cache_file, json).await;
        }
    }

    images
}
