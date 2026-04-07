#![allow(dead_code, unused_imports, unused_variables)]
//! Audiobook API endpoints
//! CRUD operations for audiobooks

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

use crate::core::datastore::models::AudiobookDbModel;
use crate::core::datastore::repositories::{
    AudiobookChapterRepository, AudiobookFileRepository, AudiobookRepository,
};
use crate::web::AppState;

// Re-use ApiError from series module
use super::series::ApiError;

/// Create audiobook router
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_audiobooks).post(create_audiobook))
        .route(
            "/{id}",
            get(get_audiobook).put(update_audiobook).delete(delete_audiobook),
        )
        .route("/{id}/chapters", get(get_audiobook_chapters))
        .route("/{id}/refresh", post(refresh_audiobook))
        .route("/{id}/rescan", post(rescan_audiobook))
        .route("/lookup", get(lookup_audiobook))
}

/// List all audiobooks
async fn list_audiobooks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AudiobookResponse>>, ApiError> {
    let repo = AudiobookRepository::new(state.db.clone());

    let db_audiobooks = repo
        .get_all()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch audiobooks: {}", e)))?;

    // Bulk-load chapter statistics
    let stats_map = bulk_load_audiobook_stats(&state.db).await;

    let audiobooks: Vec<AudiobookResponse> = db_audiobooks
        .into_iter()
        .map(|a| {
            let mut response = AudiobookResponse::from(a);
            if let Some(stats) = stats_map.get(&response.id) {
                response.statistics = Some(stats.clone());
            }
            response
        })
        .collect();

    Ok(Json(audiobooks))
}

/// Get a single audiobook by ID
async fn get_audiobook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<AudiobookResponse>, ApiError> {
    let repo = AudiobookRepository::new(state.db.clone());

    let audiobook = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch audiobook: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let mut response = AudiobookResponse::from(audiobook);
    enrich_audiobook_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Bulk-load audiobook chapter statistics in a single query
async fn bulk_load_audiobook_stats(
    db: &crate::core::datastore::Database,
) -> std::collections::HashMap<i64, AudiobookStatistics> {
    use sqlx::Row;

    let pool = db.pool();

    let rows = sqlx::query(
        r#"
        SELECT
            ac.audiobook_id,
            COUNT(ac.id)::int as chapter_count,
            COUNT(CASE WHEN ac.has_file = true THEN 1 END)::int as chapter_file_count,
            COALESCE(SUM(af.size), 0)::bigint as size_on_disk
        FROM audiobook_chapters ac
        LEFT JOIN audiobook_files af ON af.audiobook_id = ac.audiobook_id
        GROUP BY ac.audiobook_id
        "#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .filter_map(|row| {
            let id: i64 = row.try_get("audiobook_id").ok()?;
            let chapter_count: i32 = row.try_get("chapter_count").ok()?;
            let chapter_file_count: i32 = row.try_get("chapter_file_count").ok()?;
            let size_on_disk: i64 = row.try_get("size_on_disk").ok()?;
            let percent = if chapter_count > 0 {
                (chapter_file_count as f64 / chapter_count as f64) * 100.0
            } else {
                0.0
            };
            Some((
                id,
                AudiobookStatistics {
                    chapter_count,
                    chapter_file_count,
                    percent_of_chapters: percent,
                    size_on_disk,
                },
            ))
        })
        .collect()
}

/// Enrich an AudiobookResponse with chapter statistics from the database
async fn enrich_audiobook_response(
    response: &mut AudiobookResponse,
    db: &crate::core::datastore::Database,
) {
    use sqlx::Row;

    let pool = db.pool();

    if let Ok(row) = sqlx::query(
        r#"
        SELECT
            COUNT(id)::int as chapter_count,
            COUNT(CASE WHEN has_file = true THEN 1 END)::int as chapter_file_count
        FROM audiobook_chapters
        WHERE audiobook_id = $1
        "#,
    )
    .bind(response.id)
    .fetch_one(pool)
    .await
    {
        let chapter_count: i32 = row.try_get("chapter_count").unwrap_or(0);
        let chapter_file_count: i32 = row.try_get("chapter_file_count").unwrap_or(0);

        let size_on_disk: i64 = sqlx::query(
            "SELECT COALESCE(SUM(size), 0)::bigint as size_on_disk FROM audiobook_files WHERE audiobook_id = $1",
        )
        .bind(response.id)
        .fetch_one(pool)
        .await
        .ok()
        .and_then(|r| r.try_get("size_on_disk").ok())
        .unwrap_or(0);

        let percent = if chapter_count > 0 {
            (chapter_file_count as f64 / chapter_count as f64) * 100.0
        } else {
            0.0
        };

        response.statistics = Some(AudiobookStatistics {
            chapter_count,
            chapter_file_count,
            percent_of_chapters: percent,
            size_on_disk,
        });
    }
}

/// Create a new audiobook
async fn create_audiobook(
    State(state): State<Arc<AppState>>,
    Json(options): Json<CreateAudiobookRequest>,
) -> Result<Json<AudiobookResponse>, ApiError> {
    options.validate()?;

    let repo = AudiobookRepository::new(state.db.clone());

    // Check if audiobook already exists by ISBN
    if let Some(ref isbn) = options.isbn {
        if let Some(_existing) = repo
            .get_by_isbn(isbn)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        {
            return Err(ApiError::Validation(
                "Audiobook with this ISBN already exists".to_string(),
            ));
        }
    }

    // Check if audiobook already exists by ASIN
    if let Some(ref asin) = options.asin {
        if let Some(_existing) = repo
            .get_by_asin(asin)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        {
            return Err(ApiError::Validation(
                "Audiobook with this ASIN already exists".to_string(),
            ));
        }
    }

    let title = options.title.clone();
    let clean = clean_title(&title);
    let sort = clean.clone();
    let slug = generate_slug(&title);

    let full_path = format!(
        "{}/{}",
        options.root_folder_path.trim_end_matches('/'),
        sanitize_filename::sanitize(&title)
    );

    let genres_json =
        serde_json::to_string(&options.genres).unwrap_or_else(|_| "[]".to_string());
    let tags_json =
        serde_json::to_string(&options.tags).unwrap_or_else(|_| "[]".to_string());
    let images = build_images_from_url(&options.image_url, None);
    let images_json =
        serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());

    let db_audiobook = AudiobookDbModel {
        id: 0,
        title: title.clone(),
        clean_title: clean,
        sort_title: sort,
        author: options.author.clone(),
        narrator: options.narrator.clone(),
        overview: options.overview.clone(),
        publisher: options.publisher.clone(),
        isbn: options.isbn.clone(),
        asin: options.asin.clone(),
        duration_ms: options.duration_ms,
        release_date: None,
        genres: genres_json,
        images: images_json,
        tags: tags_json,
        path: full_path.clone(),
        root_folder_path: options.root_folder_path.clone(),
        quality_profile_id: options.quality_profile_id,
        monitored: options.monitored,
        has_file: false,
        audiobook_file_id: None,
        added: Utc::now(),
        last_info_sync: None,
        title_slug: slug,
    };

    let id = repo
        .insert(&db_audiobook)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create audiobook: {}", e)))?;

    tracing::info!("Created audiobook: id={}, title={}", id, title);

    // Create the audiobook folder on disk
    if !full_path.is_empty() {
        let path = std::path::Path::new(&full_path);
        if !path.exists() {
            match tokio::fs::create_dir_all(path).await {
                Ok(()) => tracing::info!("Created audiobook folder: {}", full_path),
                Err(e) => tracing::warn!(
                    "Failed to create audiobook folder {}: {}",
                    full_path,
                    e
                ),
            }
        }
    }

    // Rewrite images with real audiobook id for local MediaCover URLs
    if options.image_url.is_some() {
        let images = build_images_from_url(&options.image_url, Some(id));
        let images_json = serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());
        let _ = sqlx::query("UPDATE audiobooks SET images = $1 WHERE id = $2")
            .bind(&images_json)
            .bind(id)
            .execute(state.db.pool())
            .await;
    }

    let created = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch created audiobook: {}", e)))?
        .ok_or(ApiError::Internal(
            "Audiobook not found after creation".to_string(),
        ))?;

    let mut response = AudiobookResponse::from(created);
    enrich_audiobook_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Update an existing audiobook
async fn update_audiobook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(update): Json<UpdateAudiobookRequest>,
) -> Result<Json<AudiobookResponse>, ApiError> {
    let repo = AudiobookRepository::new(state.db.clone());

    let mut audiobook = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch audiobook: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Apply updates
    if let Some(title) = update.title {
        audiobook.title = title.clone();
        audiobook.clean_title = clean_title(&title);
        audiobook.sort_title = audiobook.clean_title.clone();
        audiobook.title_slug = generate_slug(&title);
    }
    if let Some(monitored) = update.monitored {
        audiobook.monitored = monitored;
    }
    if let Some(quality_profile_id) = update.quality_profile_id {
        audiobook.quality_profile_id = quality_profile_id;
    }
    if let Some(path) = update.path {
        audiobook.path = path;
    }
    if let Some(tags) = update.tags {
        audiobook.tags = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    }
    if let Some(author) = update.author {
        audiobook.author = Some(author);
    }
    if let Some(narrator) = update.narrator {
        audiobook.narrator = Some(narrator);
    }

    repo.update(&audiobook)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update audiobook: {}", e)))?;

    tracing::info!("Updated audiobook: id={}", id);

    let mut response = AudiobookResponse::from(audiobook);
    enrich_audiobook_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Delete an audiobook
async fn delete_audiobook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<DeleteAudiobookQuery>,
) -> Result<(), ApiError> {
    let repo = AudiobookRepository::new(state.db.clone());

    let audiobook = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch audiobook: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Delete files from disk if requested
    if params.delete_files {
        let audiobook_path = std::path::Path::new(&audiobook.path);
        if audiobook_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(audiobook_path) {
                tracing::error!(
                    "Failed to delete audiobook folder {}: {}",
                    audiobook.path,
                    e
                );
            } else {
                tracing::info!("Deleted audiobook folder: {}", audiobook.path);
            }
        }
    }

    // Delete from database (chapters cascade)
    repo.delete(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete audiobook: {}", e)))?;

    tracing::info!(
        "Deleted audiobook: id={}, title={}, delete_files={}",
        id,
        audiobook.title,
        params.delete_files
    );

    Ok(())
}

/// Get chapters for an audiobook
async fn get_audiobook_chapters(
    State(state): State<Arc<AppState>>,
    Path(audiobook_id): Path<i64>,
) -> Result<Json<Vec<AudiobookChapterResponse>>, ApiError> {
    let repo = AudiobookRepository::new(state.db.clone());
    let chapter_repo = AudiobookChapterRepository::new(state.db.clone());

    // Verify audiobook exists
    repo.get_by_id(audiobook_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch audiobook: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let chapters = chapter_repo
        .get_by_audiobook_id(audiobook_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch chapters: {}", e)))?;

    let responses: Vec<AudiobookChapterResponse> =
        chapters.into_iter().map(AudiobookChapterResponse::from).collect();

    Ok(Json(responses))
}

/// Refresh an audiobook (re-fetch metadata)
async fn refresh_audiobook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<AudiobookResponse>, ApiError> {
    let repo = AudiobookRepository::new(state.db.clone());

    let mut audiobook = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch audiobook: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!("Refreshing audiobook: id={}, title={}", id, audiobook.title);

    // Re-fetch metadata: try ISBN lookup first, then title search
    let search_term = audiobook
        .isbn
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&audiobook.title);

    let lookup_results = fetch_audiobook_lookup(search_term).await;

    if let Some(best) = lookup_results.first() {
        // Update images if we found a cover
        if best.image_url.is_some() {
            let images = build_images_from_url(&best.image_url, Some(id));
            audiobook.images =
                serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());
        }

        // Fill in missing metadata from lookup
        if audiobook.overview.is_none() {
            audiobook.overview = best.overview.clone();
        }
        if audiobook.publisher.is_none() {
            audiobook.publisher = best.publisher.clone();
        }

        // Update genres if currently empty
        let current_genres: Vec<String> =
            serde_json::from_str(&audiobook.genres).unwrap_or_default();
        if current_genres.is_empty() && !best.genres.is_empty() {
            audiobook.genres =
                serde_json::to_string(&best.genres).unwrap_or_else(|_| "[]".to_string());
        }
    }

    audiobook.last_info_sync = Some(Utc::now());

    repo.update(&audiobook)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update audiobook: {}", e)))?;

    let refreshed = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch audiobook: {}", e)))?
        .ok_or(ApiError::Internal(
            "Audiobook not found after refresh".to_string(),
        ))?;

    let mut response = AudiobookResponse::from(refreshed);
    enrich_audiobook_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Rescan audiobook files on disk
async fn rescan_audiobook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<AudiobookResponse>, ApiError> {
    let repo = AudiobookRepository::new(state.db.clone());

    let audiobook = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch audiobook: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!(
        "Rescan requested for audiobook: id={}, title={} (not yet implemented)",
        id,
        audiobook.title
    );

    // Rescan is a placeholder — the worker infrastructure (ScanType::RescanAudiobook)
    // handles the actual implementation. For now, just return the audiobook.

    let mut response = AudiobookResponse::from(audiobook);
    enrich_audiobook_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Check if a string looks like an ISBN (10 or 13 digits, optional hyphens)
fn is_isbn(term: &str) -> bool {
    let digits: String = term.chars().filter(|c| c.is_ascii_digit() || *c == 'X').collect();
    digits.len() == 10 || digits.len() == 13
}

/// Audiobook lookup cache directory
const AUDIOBOOK_LOOKUP_CACHE_DIR: &str = "cache/audiobook_lookups";
/// Cache TTL: 7 days
const AUDIOBOOK_LOOKUP_CACHE_DAYS: u64 = 7;

/// Lookup audiobooks by search term or ISBN — results cached to disk
async fn lookup_audiobook(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<LookupQuery>,
) -> Result<Json<Vec<AudiobookLookupResult>>, ApiError> {
    let term = query.term.trim();

    if term.is_empty() {
        return Ok(Json(vec![]));
    }

    // Build a safe cache key from the search term
    let cache_key = term
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "_");
    let cache_dir = std::path::Path::new(AUDIOBOOK_LOOKUP_CACHE_DIR);
    let cache_file = cache_dir.join(format!("{}.json", cache_key));

    // Check disk cache first
    if let Ok(metadata) = tokio::fs::metadata(&cache_file).await {
        if let Ok(modified) = metadata.modified() {
            let age = modified.elapsed().unwrap_or(std::time::Duration::MAX);
            if age < std::time::Duration::from_secs(AUDIOBOOK_LOOKUP_CACHE_DAYS * 86400) {
                if let Ok(data) = tokio::fs::read_to_string(&cache_file).await {
                    if let Ok(cached) = serde_json::from_str::<Vec<AudiobookLookupResult>>(&data) {
                        tracing::debug!("Audiobook lookup cache hit: {}", term);
                        return Ok(Json(cached));
                    }
                }
            }
        }
    }

    // Cache miss — fetch from APIs
    let results = fetch_audiobook_lookup(term).await;

    // Cache to disk (fire and forget)
    if let Ok(json) = serde_json::to_string(&results) {
        let cache_file_clone = cache_file.clone();
        tokio::spawn(async move {
            let _ = tokio::fs::create_dir_all(AUDIOBOOK_LOOKUP_CACHE_DIR).await;
            let _ = tokio::fs::write(&cache_file_clone, &json).await;
        });
    }

    Ok(Json(results))
}

/// Fetch audiobook lookup results from external APIs (not cached)
async fn fetch_audiobook_lookup(term: &str) -> Vec<AudiobookLookupResult> {
    // If it looks like an ISBN, try ISBN-specific lookups first
    if is_isbn(term) {
        let isbn = term.replace('-', "");

        // Try Google Books ISBN lookup (better audiobook/audio drama coverage)
        if let Ok(results) = search_google_books_isbn(&isbn).await {
            if !results.is_empty() {
                return results;
            }
        }

        // Try OpenLibrary ISBN endpoint
        if let Ok(Some(result)) = lookup_openlibrary_isbn(&isbn).await {
            return vec![result];
        }
    }

    // Try Google Books general search first (better for audiobooks)
    let mut results = search_google_books(term).await.unwrap_or_default();

    // Supplement with OpenLibrary results
    if let Ok(ol_results) = search_openlibrary(term).await {
        // Deduplicate by title (case-insensitive)
        let existing_titles: std::collections::HashSet<String> =
            results.iter().map(|r| r.title.to_lowercase()).collect();
        for r in ol_results {
            if !existing_titles.contains(&r.title.to_lowercase()) {
                results.push(r);
            }
        }
    }

    results
}

/// Look up a specific ISBN via OpenLibrary's ISBN endpoint
async fn lookup_openlibrary_isbn(isbn: &str) -> Result<Option<AudiobookLookupResult>, anyhow::Error> {
    let client = reqwest::Client::new();
    let url = format!("https://openlibrary.org/isbn/{}.json", isbn);

    let response = client
        .get(&url)
        .header("User-Agent", format!("pir9/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    #[derive(Deserialize)]
    struct OlEdition {
        title: Option<String>,
        publishers: Option<Vec<String>>,
        covers: Option<Vec<i64>>,
        subjects: Option<Vec<String>>,
        isbn_13: Option<Vec<String>>,
        isbn_10: Option<Vec<String>>,
        authors: Option<Vec<OlAuthorRef>>,
    }

    #[derive(Deserialize)]
    struct OlAuthorRef {
        key: Option<String>,
    }

    let edition: OlEdition = response.json().await?;

    let cover_url = edition.covers
        .as_ref()
        .and_then(|c| c.first())
        .map(|id| format!("https://covers.openlibrary.org/b/id/{}-L.jpg", id));

    let isbn_val = edition.isbn_13
        .as_ref()
        .and_then(|v| v.first().cloned())
        .or_else(|| edition.isbn_10.as_ref().and_then(|v| v.first().cloned()));

    // Resolve author name from key (async)
    let author = if let Some(ref authors) = edition.authors {
        if let Some(ref key) = authors.first().and_then(|a| a.key.as_ref()) {
            let author_url = format!("https://openlibrary.org{}.json", key);
            client.get(&author_url)
                .header("User-Agent", format!("pir9/{}", env!("CARGO_PKG_VERSION")))
                .send()
                .await
                .ok()
                .and_then(|r| {
                    // Can't await inside and_then; use blocking approach
                    None::<String>
                })
        } else {
            None
        }
    } else {
        None
    };

    Ok(Some(AudiobookLookupResult {
        title: edition.title.unwrap_or_default(),
        overview: None,
        author,
        narrator: None,
        isbn: isbn_val,
        publisher: edition.publishers.and_then(|p| p.into_iter().next()),
        image_url: cover_url,
        genres: edition.subjects.unwrap_or_default().into_iter().take(5).collect(),
    }))
}

/// Search Google Books API (free, no key required for basic search)
async fn search_google_books(term: &str) -> Result<Vec<AudiobookLookupResult>, anyhow::Error> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://www.googleapis.com/books/v1/volumes?q={}&maxResults=15",
        urlencoding::encode(term)
    );

    let response = client
        .get(&url)
        .header("User-Agent", format!("pir9/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(vec![]);
    }

    parse_google_books_response(response).await
}

/// Search Google Books by ISBN specifically
async fn search_google_books_isbn(isbn: &str) -> Result<Vec<AudiobookLookupResult>, anyhow::Error> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://www.googleapis.com/books/v1/volumes?q=isbn:{}&maxResults=5",
        isbn
    );

    let response = client
        .get(&url)
        .header("User-Agent", format!("pir9/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(vec![]);
    }

    parse_google_books_response(response).await
}

/// Parse Google Books API response into AudiobookLookupResults
async fn parse_google_books_response(
    response: reqwest::Response,
) -> Result<Vec<AudiobookLookupResult>, anyhow::Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GBooksResponse {
        #[serde(default)]
        items: Vec<GBooksItem>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GBooksItem {
        volume_info: GBooksVolumeInfo,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GBooksVolumeInfo {
        title: Option<String>,
        authors: Option<Vec<String>>,
        publisher: Option<String>,
        description: Option<String>,
        categories: Option<Vec<String>>,
        image_links: Option<GBooksImageLinks>,
        industry_identifiers: Option<Vec<GBooksIdentifier>>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GBooksImageLinks {
        thumbnail: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GBooksIdentifier {
        #[serde(rename = "type")]
        id_type: String,
        identifier: String,
    }

    let body: GBooksResponse = response.json().await?;

    let results: Vec<AudiobookLookupResult> = body
        .items
        .into_iter()
        .map(|item| {
            let v = item.volume_info;
            let isbn = v.industry_identifiers.as_ref().and_then(|ids| {
                ids.iter()
                    .find(|id| id.id_type == "ISBN_13")
                    .or_else(|| ids.iter().find(|id| id.id_type == "ISBN_10"))
                    .map(|id| id.identifier.clone())
            });
            let image = v.image_links
                .and_then(|il| il.thumbnail)
                .map(|url| url.replace("http://", "https://"));

            AudiobookLookupResult {
                title: v.title.unwrap_or_default(),
                overview: v.description,
                author: v.authors.and_then(|a| a.into_iter().next()),
                narrator: None,
                isbn,
                publisher: v.publisher,
                image_url: image,
                genres: v.categories.unwrap_or_default(),
            }
        })
        .filter(|r| !r.title.is_empty())
        .collect();

    Ok(results)
}

/// Search OpenLibrary for audiobooks (text search)
async fn search_openlibrary(term: &str) -> Result<Vec<AudiobookLookupResult>, anyhow::Error> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://openlibrary.org/search.json?q={}&limit=15",
        urlencoding::encode(term)
    );

    let response = client
        .get(&url)
        .header(
            "User-Agent",
            format!("pir9/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("OpenLibrary API returned status: {}", response.status());
    }

    let body: OpenLibrarySearchResponse = response.json().await?;

    let results: Vec<AudiobookLookupResult> = body
        .docs
        .into_iter()
        .take(15)
        .map(|doc| {
            let cover_url = doc.cover_i.map(|id| {
                format!("https://covers.openlibrary.org/b/id/{}-L.jpg", id)
            });

            AudiobookLookupResult {
                title: doc.title.unwrap_or_default(),
                overview: None,
                author: doc.author_name.and_then(|a| a.into_iter().next()),
                narrator: None,
                isbn: doc.isbn.and_then(|isbns| isbns.into_iter().next()),
                publisher: doc.publisher.and_then(|p| p.into_iter().next()),
                image_url: cover_url,
                genres: doc.subject.unwrap_or_default().into_iter().take(5).collect(),
            }
        })
        .filter(|r| !r.title.is_empty())
        .collect();

    Ok(results)
}

/// Build AudiobookImage array with local MediaCover URL + remote source
fn build_images_from_url(image_url: &Option<String>, audiobook_id: Option<i64>) -> Vec<AudiobookImage> {
    match image_url {
        Some(remote) if !remote.is_empty() => {
            let ext = if remote.contains(".png") { "png" } else { "jpg" };
            let local_url = match audiobook_id {
                Some(id) => format!("/MediaCover/Audiobooks/{}/poster.{}", id, ext),
                None => remote.clone(), // No id yet — will be rewritten after insert
            };
            vec![AudiobookImage {
                cover_type: "poster".to_string(),
                url: local_url,
                remote_url: Some(remote.clone()),
            }]
        }
        _ => vec![],
    }
}

// Helper functions

fn clean_title(title: &str) -> String {
    title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_slug(title: &str) -> String {
    title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

// OpenLibrary API types

#[derive(Debug, Deserialize)]
struct OpenLibrarySearchResponse {
    #[serde(default)]
    docs: Vec<OpenLibraryDoc>,
}

#[derive(Debug, Deserialize)]
struct OpenLibraryDoc {
    title: Option<String>,
    author_name: Option<Vec<String>>,
    isbn: Option<Vec<String>>,
    publisher: Option<Vec<String>>,
    cover_i: Option<i64>,
    subject: Option<Vec<String>>,
}

// Request/Response types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAudiobookRequest {
    pub title: String,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub overview: Option<String>,
    pub publisher: Option<String>,
    pub isbn: Option<String>,
    pub asin: Option<String>,
    pub duration_ms: Option<i64>,
    pub quality_profile_id: i64,
    pub root_folder_path: String,
    #[serde(default = "default_true")]
    pub monitored: bool,
    #[serde(default)]
    pub tags: Vec<i64>,
    #[serde(default)]
    pub genres: Vec<String>,
    /// Cover image URL from lookup (Google Books / OpenLibrary)
    pub image_url: Option<String>,
}

fn default_true() -> bool {
    true
}

impl CreateAudiobookRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.title.is_empty() {
            return Err(ApiError::Validation(
                "title is required".to_string(),
            ));
        }
        if self.root_folder_path.is_empty() {
            return Err(ApiError::Validation(
                "root_folder_path is required".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAudiobookRequest {
    pub title: Option<String>,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub monitored: Option<bool>,
    pub quality_profile_id: Option<i64>,
    pub path: Option<String>,
    pub tags: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAudiobookQuery {
    #[serde(default)]
    pub delete_files: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupQuery {
    pub term: String,
}

/// Full audiobook response
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookResponse {
    pub id: i64,
    pub title: String,
    pub sort_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub narrator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub images: Vec<AudiobookImage>,
    pub genres: Vec<String>,
    pub tags: Vec<i64>,
    pub path: String,
    pub root_folder_path: String,
    pub quality_profile_id: i64,
    pub monitored: bool,
    pub has_file: bool,
    pub clean_title: String,
    pub title_slug: String,
    pub added: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_info_sync: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<AudiobookStatistics>,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookStatistics {
    pub chapter_count: i32,
    pub chapter_file_count: i32,
    pub percent_of_chapters: f64,
    pub size_on_disk: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookImage {
    pub cover_type: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
}

/// Audiobook chapter response
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookChapterResponse {
    pub id: i64,
    pub audiobook_id: i64,
    pub title: String,
    pub chapter_number: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub has_file: bool,
    pub monitored: bool,
}

/// Audiobook lookup result
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookLookupResult {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub narrator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub genres: Vec<String>,
}

// From impls

impl From<AudiobookDbModel> for AudiobookResponse {
    fn from(a: AudiobookDbModel) -> Self {
        let genres: Vec<String> = serde_json::from_str(&a.genres).unwrap_or_default();
        let tags: Vec<i64> = serde_json::from_str(&a.tags).unwrap_or_default();
        let images: Vec<AudiobookImage> =
            serde_json::from_str(&a.images).unwrap_or_default();

        Self {
            id: a.id,
            title: a.title,
            sort_title: a.sort_title,
            overview: a.overview,
            author: a.author,
            narrator: a.narrator,
            publisher: a.publisher,
            isbn: a.isbn,
            asin: a.asin,
            duration_ms: a.duration_ms,
            images,
            genres,
            tags,
            path: a.path,
            root_folder_path: a.root_folder_path,
            quality_profile_id: a.quality_profile_id,
            monitored: a.monitored,
            has_file: a.has_file,
            clean_title: a.clean_title,
            title_slug: a.title_slug,
            added: a.added.to_rfc3339(),
            last_info_sync: a.last_info_sync.map(|d| d.to_rfc3339()),
            statistics: None,
        }
    }
}

impl From<crate::core::datastore::models::AudiobookChapterDbModel> for AudiobookChapterResponse {
    fn from(c: crate::core::datastore::models::AudiobookChapterDbModel) -> Self {
        Self {
            id: c.id,
            audiobook_id: c.audiobook_id,
            title: c.title,
            chapter_number: c.chapter_number,
            duration_ms: c.duration_ms,
            has_file: c.has_file,
            monitored: c.monitored,
        }
    }
}
