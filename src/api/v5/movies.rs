#![allow(dead_code, unused_imports, unused_variables)]
//! Movie API endpoints
//! CRUD operations for movies

use axum::{
    Router,
    routing::{get, post},
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::{MovieDbModel, MovieFileDbModel};
use crate::core::datastore::repositories::{MovieRepository, MovieFileRepository};
use crate::web::AppState;

// Re-use ApiError from series module
use super::series::ApiError;

/// Query parameters for listing movies
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MovieListQuery {
    pub tmdb_id: Option<i64>,
}

/// Create movie router
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_movies).post(create_movie))
        .route("/{id}", get(get_movie).put(update_movie).delete(delete_movie))
        .route("/lookup", get(lookup_movie))
        .route("/import", post(import_movies))
}

/// List all movies (or filter by tmdbId)
async fn list_movies(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MovieListQuery>,
) -> Result<Json<Vec<MovieResponse>>, ApiError> {
    let repo = MovieRepository::new(state.db.clone());

    // If tmdbId filter is provided, return just that movie
    if let Some(tmdb_id) = query.tmdb_id {
        let movie = repo.get_by_tmdb_id(tmdb_id).await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch movie: {}", e)))?;
        return match movie {
            Some(m) => {
                let mut response = MovieResponse::from(m);
                enrich_movie_response(&mut response, &state.db).await;
                Ok(Json(vec![response]))
            }
            None => Ok(Json(vec![])),
        };
    }

    let db_movies = repo.get_all().await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch movies: {}", e)))?;

    let mut movies = Vec::with_capacity(db_movies.len());
    for m in db_movies {
        let mut response = MovieResponse::from(m);
        enrich_movie_response(&mut response, &state.db).await;
        movies.push(response);
    }

    Ok(Json(movies))
}

/// Get a single movie by ID
async fn get_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<MovieResponse>, ApiError> {
    let repo = MovieRepository::new(state.db.clone());

    let movie = repo.get_by_id(id).await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch movie: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let mut response = MovieResponse::from(movie);
    enrich_movie_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Enrich a MovieResponse with file statistics from the database
async fn enrich_movie_response(response: &mut MovieResponse, db: &crate::core::datastore::Database) {
    use sqlx::Row;

    let pool = db.pool();
    if let Ok(row) = sqlx::query(
        "SELECT COALESCE(SUM(size), 0) as size_on_disk FROM movie_files WHERE movie_id = $1"
    )
    .bind(response.id)
    .fetch_one(pool)
    .await {
        let size_on_disk: i64 = row.try_get("size_on_disk").unwrap_or(0);
        response.statistics = Some(MovieStatistics {
            size_on_disk,
            has_file: response.has_file,
        });
    }
}

/// Create a new movie
async fn create_movie(
    State(state): State<Arc<AppState>>,
    Json(options): Json<CreateMovieRequest>,
) -> Result<Json<MovieResponse>, ApiError> {
    options.validate()?;

    let repo = MovieRepository::new(state.db.clone());

    // Check if movie already exists by tmdbId
    if let Some(_existing) = repo.get_by_tmdb_id(options.tmdb_id).await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    {
        return Err(ApiError::Validation("Movie already exists".to_string()));
    }

    let clean = clean_title(&options.title);
    let sort = clean.clone();
    let slug = generate_slug(&options.title);

    let full_path = options.get_full_path();
    let root_folder_path = options.get_root_folder_path();

    let release_date = options.release_date.as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let status = match options.status.as_deref() {
        Some("announced") => 1,
        Some("inCinemas") => 2,
        Some("released") => 3,
        _ => 0, // TBA
    };

    let genres_json = serde_json::to_string(&options.genres).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(&options.tags).unwrap_or_else(|_| "[]".to_string());

    // Use images from request; if empty, try to fetch from TMDB
    let images = if options.images.is_empty() {
        if let Some(ref imdb_id) = options.imdb_id {
            fetch_tmdb_images(&state.tmdb_client, imdb_id).await
        } else {
            vec![]
        }
    } else {
        options.images.clone()
    };
    let images_json = serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());

    let db_movie = MovieDbModel {
        id: 0,
        tmdb_id: options.tmdb_id,
        imdb_id: options.imdb_id.clone(),
        title: options.title.clone(),
        clean_title: clean,
        sort_title: sort,
        status,
        overview: options.overview.clone(),
        monitored: options.monitored,
        quality_profile_id: options.quality_profile_id,
        title_slug: slug,
        path: full_path,
        root_folder_path,
        year: options.year.unwrap_or(0),
        release_date,
        physical_release_date: None,
        digital_release_date: None,
        runtime: options.runtime.unwrap_or(0),
        studio: options.studio.clone(),
        certification: options.certification.clone(),
        genres: genres_json,
        tags: tags_json,
        images: images_json,
        has_file: false,
        movie_file_id: None,
        added: Utc::now(),
        last_info_sync: None,
        imdb_rating: None,
        imdb_votes: None,
    };

    let id = repo.insert(&db_movie).await
        .map_err(|e| ApiError::Internal(format!("Failed to create movie: {}", e)))?;

    tracing::info!("Created movie: id={}, title={}", id, options.title);

    let created = repo.get_by_id(id).await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch created movie: {}", e)))?
        .ok_or(ApiError::Internal("Movie not found after creation".to_string()))?;

    let mut response = MovieResponse::from(created);
    enrich_movie_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Update an existing movie
async fn update_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(update): Json<UpdateMovieRequest>,
) -> Result<Json<MovieResponse>, ApiError> {
    let repo = MovieRepository::new(state.db.clone());

    let mut movie = repo.get_by_id(id).await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch movie: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Apply updates
    if let Some(title) = update.title {
        movie.title = title.clone();
        movie.clean_title = clean_title(&title);
        movie.sort_title = movie.clean_title.clone();
    }
    if let Some(monitored) = update.monitored {
        movie.monitored = monitored;
    }
    if let Some(quality_profile_id) = update.quality_profile_id {
        movie.quality_profile_id = quality_profile_id;
    }
    if let Some(path) = update.path {
        movie.path = path;
    }
    if let Some(tags) = update.tags {
        movie.tags = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    }

    repo.update(&movie).await
        .map_err(|e| ApiError::Internal(format!("Failed to update movie: {}", e)))?;

    tracing::info!("Updated movie: id={}", id);

    let mut response = MovieResponse::from(movie);
    enrich_movie_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Delete a movie
async fn delete_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<DeleteMovieQuery>,
) -> Result<(), ApiError> {
    let repo = MovieRepository::new(state.db.clone());

    let movie = repo.get_by_id(id).await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch movie: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Delete files from disk if requested
    if params.delete_files {
        let movie_path = std::path::Path::new(&movie.path);
        if movie_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(movie_path) {
                tracing::error!("Failed to delete movie folder {}: {}", movie.path, e);
                // Continue with DB deletion even if file deletion fails
            } else {
                tracing::info!("Deleted movie folder: {}", movie.path);
            }
        }
    }

    // Delete from database (movie_files cascade)
    repo.delete(id).await
        .map_err(|e| ApiError::Internal(format!("Failed to delete movie: {}", e)))?;

    tracing::info!("Deleted movie: id={}, title={}, delete_files={}", id, movie.title, params.delete_files);

    Ok(())
}

/// Lookup movies from pir9-imdb service, enriched with TMDB images
async fn lookup_movie(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LookupQuery>,
) -> Result<Json<Vec<MovieLookupResult>>, ApiError> {
    let results = state.imdb_client.search_movies(&query.term, 25).await
        .map_err(|e| ApiError::Internal(format!("Failed to search movies: {}", e)))?;

    // Fire TMDB lookups in parallel for all results that have IMDB IDs
    let tmdb_futures: Vec<_> = results.iter().map(|m| {
        let tmdb = state.tmdb_client.clone();
        let imdb_id = m.imdb_id.clone();
        async move { tmdb.find_movie_by_imdb_id(&imdb_id).await.ok().flatten() }
    }).collect();

    let tmdb_results = futures::future::join_all(tmdb_futures).await;

    let lookup_results: Vec<MovieLookupResult> = results.into_iter()
        .zip(tmdb_results)
        .map(|(m, tmdb)| {
            let (tmdb_id, images) = if let Some(ref t) = tmdb {
                let mut imgs = Vec::new();
                if let Some(ref poster) = t.poster_url {
                    imgs.push(MovieImage {
                        cover_type: "poster".to_string(),
                        url: poster.clone(),
                        remote_url: Some(poster.clone()),
                    });
                }
                if let Some(ref fanart) = t.fanart_url {
                    imgs.push(MovieImage {
                        cover_type: "fanart".to_string(),
                        url: fanart.clone(),
                        remote_url: Some(fanart.clone()),
                    });
                }
                (t.tmdb_id, imgs)
            } else {
                (0, vec![])
            };

            MovieLookupResult {
                tmdb_id,
                imdb_id: Some(m.imdb_id),
                title: m.title.clone(),
                sort_title: m.title.to_lowercase(),
                overview: None,
                year: m.year.unwrap_or(0),
                studio: None,
                images,
                ratings: Ratings {
                    votes: m.votes.unwrap_or(0),
                    value: m.rating.unwrap_or(0.0),
                },
                genres: m.genres,
                runtime: m.runtime_minutes.unwrap_or(0),
                certification: None,
            }
        }).collect();

    Ok(Json(lookup_results))
}

// Helper functions

/// Fetch poster and fanart images from TMDB for a given IMDB ID
async fn fetch_tmdb_images(
    tmdb_client: &crate::core::tmdb::TmdbClient,
    imdb_id: &str,
) -> Vec<MovieImage> {
    match tmdb_client.find_movie_by_imdb_id(imdb_id).await {
        Ok(Some(tmdb)) => {
            let mut images = Vec::new();
            if let Some(ref poster) = tmdb.poster_url {
                images.push(MovieImage {
                    cover_type: "poster".to_string(),
                    url: poster.clone(),
                    remote_url: Some(poster.clone()),
                });
            }
            if let Some(ref fanart) = tmdb.fanart_url {
                images.push(MovieImage {
                    cover_type: "fanart".to_string(),
                    url: fanart.clone(),
                    remote_url: Some(fanart.clone()),
                });
            }
            images
        }
        _ => vec![],
    }
}

fn clean_title(title: &str) -> String {
    title.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_slug(title: &str) -> String {
    title.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

// Request/Response types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMovieRequest {
    pub tmdb_id: i64,
    pub title: String,
    pub quality_profile_id: i64,
    #[serde(default)]
    pub root_folder_path: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub monitored: bool,
    #[serde(default)]
    pub tags: Vec<i64>,
    // Optional fields from lookup
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub studio: Option<String>,
    pub runtime: Option<i32>,
    pub imdb_id: Option<String>,
    pub certification: Option<String>,
    pub release_date: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub images: Vec<MovieImage>,
    #[serde(default)]
    pub add_options: AddMovieOptionsRequest,
}

impl CreateMovieRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.tmdb_id <= 0 {
            return Err(ApiError::Validation("tmdbId must be positive".to_string()));
        }
        if self.title.is_empty() {
            return Err(ApiError::Validation("title is required".to_string()));
        }
        if self.path.is_none() && self.root_folder_path.as_ref().map_or(true, |s| s.is_empty()) {
            return Err(ApiError::Validation("path or rootFolderPath is required".to_string()));
        }
        Ok(())
    }

    fn get_full_path(&self) -> String {
        if let Some(path) = &self.path {
            if !path.is_empty() {
                return path.clone();
            }
        }
        let root = self.root_folder_path.as_deref().unwrap_or("");
        let folder_name = format!("{} ({})", self.title, self.year.unwrap_or(0));
        format!("{}/{}", root.trim_end_matches('/'), folder_name)
    }

    fn get_root_folder_path(&self) -> String {
        if let Some(root) = &self.root_folder_path {
            if !root.is_empty() {
                return root.clone();
            }
        }
        if let Some(path) = &self.path {
            if let Some(idx) = path.rfind('/') {
                return path[..idx].to_string();
            }
        }
        String::new()
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddMovieOptionsRequest {
    #[serde(default)]
    pub search_for_movie: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMovieRequest {
    pub title: Option<String>,
    pub monitored: Option<bool>,
    pub quality_profile_id: Option<i64>,
    pub path: Option<String>,
    pub tags: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMovieQuery {
    #[serde(default)]
    pub delete_files: bool,
    #[serde(default)]
    pub add_import_list_exclusion: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupQuery {
    pub term: String,
}

/// Full movie response
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MovieResponse {
    pub id: i64,
    pub title: String,
    pub sort_title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    pub images: Vec<MovieImage>,
    pub year: i32,
    pub path: String,
    pub quality_profile_id: i64,
    pub monitored: bool,
    pub runtime: i32,
    pub tmdb_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_id: Option<String>,
    pub clean_title: String,
    pub title_slug: String,
    pub root_folder_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub studio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certification: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<i64>,
    pub added: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_release_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digital_release_date: Option<String>,
    pub has_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movie_file_id: Option<i64>,
    pub ratings: Ratings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_rating: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_votes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<MovieStatistics>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MovieStatistics {
    pub size_on_disk: i64,
    pub has_file: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MovieImage {
    pub cover_type: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Ratings {
    pub votes: i64,
    pub value: f64,
}

impl From<MovieDbModel> for MovieResponse {
    fn from(m: MovieDbModel) -> Self {
        let status = match m.status {
            0 => "tba",
            1 => "announced",
            2 => "inCinemas",
            3 => "released",
            _ => "deleted",
        };

        let genres: Vec<String> = serde_json::from_str(&m.genres).unwrap_or_default();
        let tags: Vec<i64> = serde_json::from_str(&m.tags).unwrap_or_default();
        let images: Vec<MovieImage> = serde_json::from_str(&m.images).unwrap_or_default();

        // Construct default images if none stored.
        // The local /MediaCover proxy route will resolve remote images on demand.
        let images = if images.is_empty() {
            vec![
                MovieImage {
                    cover_type: "poster".to_string(),
                    url: format!("/MediaCover/Movies/{}/poster.jpg", m.id),
                    remote_url: None,
                },
                MovieImage {
                    cover_type: "fanart".to_string(),
                    url: format!("/MediaCover/Movies/{}/fanart.jpg", m.id),
                    remote_url: None,
                },
            ]
        } else {
            // Rewrite urls to local proxy paths, preserving remote_url for CDN access
            images.into_iter().map(|img| {
                let ext = if img.cover_type == "fanart" { "jpg" } else { "jpg" };
                MovieImage {
                    url: format!("/MediaCover/Movies/{}/{}.{}", m.id, img.cover_type, ext),
                    remote_url: if img.remote_url.is_some() { img.remote_url } else { Some(img.url).filter(|u| u.starts_with("http")) },
                    cover_type: img.cover_type,
                }
            }).collect()
        };

        let folder = m.path.split('/').last().map(|f| f.to_string());

        Self {
            id: m.id,
            title: m.title,
            sort_title: m.sort_title,
            status: status.to_string(),
            overview: m.overview,
            images,
            year: m.year,
            path: m.path,
            quality_profile_id: m.quality_profile_id,
            monitored: m.monitored,
            runtime: m.runtime,
            tmdb_id: m.tmdb_id,
            imdb_id: m.imdb_id,
            clean_title: m.clean_title,
            title_slug: m.title_slug,
            root_folder_path: m.root_folder_path,
            folder,
            studio: m.studio,
            certification: m.certification,
            genres,
            tags,
            added: m.added.to_rfc3339(),
            release_date: m.release_date.map(|d| d.to_string()),
            physical_release_date: m.physical_release_date.map(|d| d.to_string()),
            digital_release_date: m.digital_release_date.map(|d| d.to_string()),
            has_file: m.has_file,
            movie_file_id: m.movie_file_id,
            ratings: Ratings {
                votes: m.imdb_votes.map(|v| v as i64).unwrap_or(0),
                value: m.imdb_rating.map(|r| r as f64).unwrap_or(0.0),
            },
            imdb_rating: m.imdb_rating,
            imdb_votes: m.imdb_votes,
            statistics: Some(MovieStatistics::default()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieLookupResult {
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub year: i32,
    pub studio: Option<String>,
    pub images: Vec<MovieImage>,
    pub ratings: Ratings,
    pub genres: Vec<String>,
    pub runtime: i32,
    pub certification: Option<String>,
}

/// Request for importing movies from disk
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMovieRequest {
    pub imdb_id: Option<String>,
    pub title: Option<String>,
    pub path: Option<String>,
    pub root_folder_path: Option<String>,
    pub quality_profile_id: Option<i64>,
    #[serde(default)]
    pub monitored: Option<bool>,
    #[serde(default)]
    pub tags: Option<Vec<i64>>,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub studio: Option<String>,
    pub runtime: Option<i32>,
    #[serde(default)]
    pub genres: Option<Vec<String>>,
    #[serde(default)]
    pub images: Option<Vec<MovieImage>>,
}

/// Import multiple movies at once (bulk create from disk)
async fn import_movies(
    State(state): State<Arc<AppState>>,
    Json(movie_list): Json<Vec<ImportMovieRequest>>,
) -> Result<Json<Vec<MovieResponse>>, ApiError> {
    let repo = MovieRepository::new(state.db.clone());
    let file_repo = MovieFileRepository::new(state.db.clone());
    let mut results = Vec::with_capacity(movie_list.len());

    for import_req in movie_list {
        // Extract folder name and year from path
        let folder_name = import_req.path.as_ref()
            .and_then(|p| p.rsplit('/').next())
            .unwrap_or("")
            .to_string();

        let folder_year = extract_year_from_folder(&folder_name);

        // Get the title to use (from request or extract from path)
        let lookup_title = import_req.title.clone().unwrap_or_else(|| {
            // Strip year suffix like " (2020)" from folder name
            if let Some(idx) = folder_name.rfind(" (") {
                folder_name[..idx].to_string()
            } else {
                folder_name.clone()
            }
        });

        if lookup_title.is_empty() {
            tracing::warn!("Skipping movie import: no title or path provided");
            continue;
        }

        // If imdb_id is provided, check for duplicates
        let imdb_id = import_req.imdb_id.clone();
        if let Some(ref id) = imdb_id {
            if !id.is_empty() {
                if let Ok(Some(_existing)) = repo.get_by_imdb_id(id).await {
                    tracing::info!("Movie already exists, skipping: imdbId={}", id);
                    continue;
                }
            }
        }

        // If imdb_id not provided, try to look up via IMDB service
        let (resolved_imdb_id, resolved_title, resolved_year, resolved_overview, resolved_runtime, resolved_genres, resolved_rating, resolved_votes) =
            if imdb_id.as_ref().map_or(true, |id| id.is_empty()) && state.imdb_client.is_enabled() {
                tracing::info!("Looking up movie for import: {} (year={:?})", lookup_title, folder_year);
                match state.imdb_client.search_movies(&lookup_title, 10).await {
                    Ok(results) if !results.is_empty() => {
                        // Pick best match by year proximity
                        let best = if let Some(yr) = folder_year {
                            results.iter()
                                .filter(|m| m.year.is_some())
                                .min_by_key(|m| (m.year.unwrap_or(0) - yr).unsigned_abs())
                                .or(results.first())
                        } else {
                            results.first()
                        };

                        if let Some(m) = best {
                            tracing::info!("IMDB match: {} ({}) [{}]", m.title, m.year.unwrap_or(0), m.imdb_id);
                            (
                                Some(m.imdb_id.clone()),
                                m.title.clone(),
                                m.year,
                                None::<String>, // IMDB search doesn't return overview
                                m.runtime_minutes,
                                m.genres.clone(),
                                m.rating.map(|r| r as f32),
                                m.votes.map(|v| v as i32),
                            )
                        } else {
                            (None, lookup_title.clone(), folder_year, None, None, vec![], None, None)
                        }
                    }
                    _ => (None, lookup_title.clone(), folder_year, None, None, vec![], None, None),
                }
            } else {
                (imdb_id, lookup_title.clone(), import_req.year.or(folder_year), import_req.overview.clone(), import_req.runtime, import_req.genres.clone().unwrap_or_default(), None, None)
            };

        // Double-check resolved imdb_id for duplicates
        if let Some(ref id) = resolved_imdb_id {
            if !id.is_empty() {
                if let Ok(Some(_existing)) = repo.get_by_imdb_id(id).await {
                    tracing::info!("Movie already exists after lookup, skipping: imdbId={}", id);
                    continue;
                }
            }
        }

        let title = import_req.title.unwrap_or(resolved_title);
        let clean = clean_title(&title);
        let sort = clean.clone();
        let slug = generate_slug(&title);

        let full_path = import_req.path.clone().unwrap_or_default();
        let root_folder_path = import_req.root_folder_path.clone().unwrap_or_else(|| {
            if let Some(idx) = full_path.rfind('/') {
                full_path[..idx].to_string()
            } else {
                String::new()
            }
        });

        let year = resolved_year.unwrap_or(0);
        let genres_json = serde_json::to_string(&resolved_genres).unwrap_or_else(|_| "[]".to_string());
        let tags_json = serde_json::to_string(&import_req.tags.unwrap_or_default()).unwrap_or_else(|_| "[]".to_string());

        // Use images from request; if empty, try to fetch from TMDB
        let images = match import_req.images {
            Some(ref imgs) if !imgs.is_empty() => imgs.clone(),
            _ => {
                if let Some(ref id) = resolved_imdb_id {
                    fetch_tmdb_images(&state.tmdb_client, id).await
                } else {
                    vec![]
                }
            }
        };
        let images_json = serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());

        let db_movie = MovieDbModel {
            id: 0,
            tmdb_id: 0,
            imdb_id: resolved_imdb_id,
            title: title.clone(),
            clean_title: clean,
            sort_title: sort,
            status: 3, // released (imported from disk)
            overview: resolved_overview,
            monitored: import_req.monitored.unwrap_or(true),
            quality_profile_id: import_req.quality_profile_id.unwrap_or(1),
            title_slug: slug,
            path: full_path.clone(),
            root_folder_path,
            year,
            release_date: None,
            physical_release_date: None,
            digital_release_date: None,
            runtime: resolved_runtime.unwrap_or(0),
            studio: import_req.studio,
            certification: None,
            genres: genres_json,
            tags: tags_json,
            images: images_json,
            has_file: false,
            movie_file_id: None,
            added: Utc::now(),
            last_info_sync: None,
            imdb_rating: resolved_rating,
            imdb_votes: resolved_votes,
        };

        match repo.insert(&db_movie).await {
            Ok(id) => {
                tracing::info!("Imported movie: id={}, title={}", id, title);

                // Scan folder for video file
                if let Some(movie_file) = scan_movie_folder(&full_path, id) {
                    match file_repo.insert(&movie_file).await {
                        Ok(file_id) => {
                            // Update movie with file info
                            let pool = state.db.pool();
                            let _ = sqlx::query(
                                "UPDATE movies SET has_file = true, movie_file_id = $1 WHERE id = $2"
                            )
                            .bind(file_id)
                            .bind(id)
                            .execute(pool)
                            .await;
                            tracing::info!("Found video file for movie {}: {}", id, movie_file.path);
                        }
                        Err(e) => {
                            tracing::error!("Failed to insert movie file for {}: {}", id, e);
                        }
                    }
                }

                if let Ok(Some(created)) = repo.get_by_id(id).await {
                    let mut response = MovieResponse::from(created);
                    enrich_movie_response(&mut response, &state.db).await;
                    results.push(response);
                }
            }
            Err(e) => {
                tracing::error!("Failed to import movie {}: {}", title, e);
            }
        }
    }

    tracing::info!("Bulk movie import complete: {} movies imported", results.len());
    Ok(Json(results))
}

/// Extract a year (1900-2099) from a folder name like "The Matrix (1999)" or "The.Matrix.1999"
fn extract_year_from_folder(folder: &str) -> Option<i32> {
    static PAREN_YEAR_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\((\d{4})\)").expect("valid regex")
    });
    static FOLDER_YEAR_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"[\s.\(_-]((?:19|20)\d{2})[\s.\)_-]?$").expect("valid regex")
    });

    if let Some(caps) = PAREN_YEAR_RE.captures(folder) {
        if let Ok(y) = caps[1].parse::<i32>() {
            if (1900..2100).contains(&y) {
                return Some(y);
            }
        }
    }
    if let Some(caps) = FOLDER_YEAR_RE.captures(folder) {
        if let Ok(y) = caps[1].parse::<i32>() {
            if (1900..2100).contains(&y) {
                return Some(y);
            }
        }
    }
    None
}

/// Scan a movie folder for the largest video file and return a MovieFileDbModel
fn scan_movie_folder(folder_path: &str, movie_id: i64) -> Option<MovieFileDbModel> {
    use std::path::Path;

    let root = Path::new(folder_path);
    if !root.exists() || !root.is_dir() {
        return None;
    }

    let video_extensions = ["mkv", "mp4", "avi", "wmv", "m4v", "ts", "webm", "mov"];

    let mut best_file: Option<(std::path::PathBuf, u64)> = None;

    fn walk_dir(dir: &Path, extensions: &[&str], best: &mut Option<(std::path::PathBuf, u64)>, depth: usize) {
        if depth > 2 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_dir(&path, extensions, best, depth + 1);
                } else {
                    let is_video = path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| extensions.contains(&ext.to_lowercase().as_str()))
                        .unwrap_or(false);

                    if is_video {
                        let size = std::fs::metadata(&path)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        if best.as_ref().map_or(true, |(_, s)| size > *s) {
                            *best = Some((path, size));
                        }
                    }
                }
            }
        }
    }

    walk_dir(root, &video_extensions, &mut best_file, 0);

    best_file.map(|(file_path, size)| {
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let relative_path = file_path.strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file_name.to_string());

        let quality = super::series::parse_quality_from_filename(file_name);
        let release_group = super::series::parse_release_group(file_name);

        MovieFileDbModel {
            id: 0,
            movie_id,
            relative_path,
            path: file_path.to_string_lossy().to_string(),
            size: size as i64,
            date_added: Utc::now(),
            scene_name: Some(file_name.to_string()),
            release_group,
            quality: serde_json::to_string(&quality).unwrap_or_else(|_| r#"{"quality":{"id":1,"name":"HDTV-720p"}}"#.to_string()),
            languages: r#"[{"id":1,"name":"English"}]"#.to_string(),
            media_info: None,
            original_file_path: Some(file_path.to_string_lossy().to_string()),
            edition: None,
        }
    })
}
