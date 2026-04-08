//! Radarr v3 compatible Movie API endpoints
//!
//! Provides movie lookup (TMDB/IMDB/text), list, and add endpoints
//! matching the Radarr v3 API shape so external scripts can add movies.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::core::datastore::repositories::MovieRepository;
use crate::web::AppState;

// Reuse types from v5 movies module
use crate::api::v5::movies::{
    self, CreateMovieRequest, MovieImage, MovieLookupResult, MovieResponse, Ratings,
};
// Reuse ApiError from v5 series
use crate::api::v5::series::ApiError;

/// Routes mounted at `/api/v3/movie`
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_movies).post(add_movie))
        .route("/{id}", get(get_movie))
}

/// Routes mounted at `/api/v3/movie/lookup`
pub fn lookup_routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(lookup_movie))
}

// ─── Query types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MovieLookupQuery {
    term: Option<String>,
    tmdb_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MovieListQuery {
    tmdb_id: Option<i64>,
}

// ─── Lookup handler ──────────────────────────────────────────────────────────

/// GET /api/v3/movie/lookup?term=tmdb:152742
///
/// Supports three lookup modes:
/// - `term=tmdb:{id}` — fetch from TMDB, enrich from local IMDB service
/// - `term=imdb:{id}` — fetch from local IMDB service, enrich with TMDB images
/// - `term={text}` — search local IMDB service, enrich with TMDB images
/// - `tmdbId={id}` query param — same as tmdb: prefix
async fn lookup_movie(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MovieLookupQuery>,
) -> Result<Json<Vec<MovieLookupResult>>, ApiError> {
    let term = query.term.as_deref().unwrap_or("");

    // Parse the term to determine lookup mode
    if let Some(tmdb_id) = query.tmdb_id.or_else(|| {
        term.strip_prefix("tmdb:")
            .and_then(|s| s.trim().parse::<i64>().ok())
    }) {
        return lookup_by_tmdb_id(&state, tmdb_id).await;
    }

    if let Some(imdb_id) = term.strip_prefix("imdb:").map(|s| s.trim()) {
        return lookup_by_imdb_id(&state, imdb_id).await;
    }

    // Fall through to text search
    if term.is_empty() {
        return Ok(Json(vec![]));
    }
    lookup_by_text(&state, term).await
}

/// Lookup by TMDB ID: TMDB API → optional IMDB enrichment
async fn lookup_by_tmdb_id(
    state: &AppState,
    tmdb_id: i64,
) -> Result<Json<Vec<MovieLookupResult>>, ApiError> {
    // 1. Fetch from TMDB direct API
    let tmdb = match movies::fetch_tmdb_movie_by_id(tmdb_id).await {
        Some(detail) => detail,
        None => {
            tracing::warn!("TMDB lookup failed for tmdb_id={}", tmdb_id);
            return Ok(Json(vec![]));
        }
    };

    let title = tmdb.title.clone().unwrap_or_default();
    let overview = tmdb.overview.clone();
    let runtime = tmdb.runtime.unwrap_or(0);
    let year = tmdb
        .release_date
        .as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(0);
    let genres: Vec<String> = tmdb
        .genres
        .as_ref()
        .map(|gs| gs.iter().map(|g| g.name.clone()).collect())
        .unwrap_or_default();
    // Build images from TMDB paths
    let mut images = Vec::new();
    if let Some(ref poster) = tmdb.poster_path {
        let url = format!("https://image.tmdb.org/t/p/w500{}", poster);
        images.push(MovieImage {
            cover_type: "poster".to_string(),
            url: url.clone(),
            remote_url: Some(url),
        });
    }
    if let Some(ref backdrop) = tmdb.backdrop_path {
        let url = format!("https://image.tmdb.org/t/p/w1280{}", backdrop);
        images.push(MovieImage {
            cover_type: "fanart".to_string(),
            url: url.clone(),
            remote_url: Some(url),
        });
    }

    // 2. If TMDB returned an imdb_id, enrich from local IMDB service
    let (imdb_id, rating, votes, imdb_genres) = if let Some(ref imdb_id) = tmdb.imdb_id {
        if state.imdb_client.is_enabled() {
            match state.imdb_client.get_movie(imdb_id).await {
                Ok(Some(m)) => (
                    Some(imdb_id.clone()),
                    m.rating.unwrap_or(0.0),
                    m.votes.unwrap_or(0),
                    if m.genres.is_empty() {
                        None
                    } else {
                        Some(m.genres)
                    },
                ),
                _ => (Some(imdb_id.clone()), 0.0, 0, None),
            }
        } else {
            (
                Some(imdb_id.clone()),
                tmdb.vote_average.unwrap_or(0.0),
                tmdb.vote_count.unwrap_or(0),
                None,
            )
        }
    } else {
        (
            None,
            tmdb.vote_average.unwrap_or(0.0),
            tmdb.vote_count.unwrap_or(0),
            None,
        )
    };

    let result = MovieLookupResult {
        tmdb_id,
        imdb_id,
        title: title.clone(),
        sort_title: title.to_lowercase(),
        overview,
        year,
        studio: None,
        images,
        ratings: Ratings {
            votes,
            value: rating,
        },
        genres: imdb_genres.unwrap_or(genres),
        runtime,
        certification: None,
    };

    Ok(Json(vec![result]))
}

/// Lookup by IMDB ID: local IMDB service → TMDB image enrichment
async fn lookup_by_imdb_id(
    state: &AppState,
    imdb_id: &str,
) -> Result<Json<Vec<MovieLookupResult>>, ApiError> {
    // 1. Try local IMDB service first
    let imdb_movie = if state.imdb_client.is_enabled() {
        state
            .imdb_client
            .get_movie(imdb_id)
            .await
            .map_err(|e| ApiError::Internal(format!("IMDB service error: {}", e)))?
    } else {
        None
    };

    // 2. Fetch enrichment (images + metadata) from cascade
    let enrichment = movies::fetch_movie_images_and_tmdb_id(imdb_id).await;

    let result = if let Some(m) = imdb_movie {
        let (tmdb_id, images, overview, studio, certification) = match &enrichment {
            Some(e) => (
                e.tmdb_id,
                e.images.clone(),
                e.overview.clone(),
                e.studio.clone(),
                e.certification.clone(),
            ),
            None => (0, vec![], None, None, None),
        };
        MovieLookupResult {
            tmdb_id,
            imdb_id: Some(m.imdb_id),
            title: m.title.clone(),
            sort_title: m.title.to_lowercase(),
            overview,
            year: m.year.unwrap_or(0),
            studio,
            images,
            ratings: Ratings {
                votes: m.votes.unwrap_or(0),
                value: m.rating.unwrap_or(0.0),
            },
            genres: m.genres,
            runtime: m.runtime_minutes.unwrap_or(0),
            certification,
        }
    } else {
        // No IMDB data — try Radarr metadata as fallback
        match movies::fetch_radarr_full_metadata(imdb_id).await {
            Some(meta) => MovieLookupResult {
                tmdb_id: meta.tmdb_id,
                imdb_id: Some(imdb_id.to_string()),
                title: meta.title.clone().unwrap_or_default(),
                sort_title: meta.title.unwrap_or_default().to_lowercase(),
                overview: meta.overview,
                year: meta.year.unwrap_or(0),
                studio: meta.studio,
                images: meta.images,
                ratings: Ratings {
                    votes: 0,
                    value: 0.0,
                },
                genres: vec![],
                runtime: meta.runtime.unwrap_or(0),
                certification: meta.certification,
            },
            None => return Ok(Json(vec![])),
        }
    };

    Ok(Json(vec![result]))
}

/// Lookup by text search: local IMDB service → parallel TMDB image enrichment
async fn lookup_by_text(
    state: &AppState,
    term: &str,
) -> Result<Json<Vec<MovieLookupResult>>, ApiError> {
    let results = state
        .imdb_client
        .search_movies(term, 25)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search movies: {}", e)))?;

    // Parallel enrichment lookups
    let enrichment_futures: Vec<_> = results
        .iter()
        .map(|m| {
            let imdb_id = m.imdb_id.clone();
            async move { movies::fetch_movie_images_and_tmdb_id(&imdb_id).await }
        })
        .collect();

    let enrichment_results = futures::future::join_all(enrichment_futures).await;

    let lookup_results: Vec<MovieLookupResult> = results
        .into_iter()
        .zip(enrichment_results)
        .map(|(m, enrichment)| {
            let (tmdb_id, images, overview, studio, certification) = match enrichment {
                Some(e) => (e.tmdb_id, e.images, e.overview, e.studio, e.certification),
                None => (0, vec![], None, None, None),
            };
            MovieLookupResult {
                tmdb_id,
                imdb_id: Some(m.imdb_id),
                title: m.title.clone(),
                sort_title: m.title.to_lowercase(),
                overview,
                year: m.year.unwrap_or(0),
                studio,
                images,
                ratings: Ratings {
                    votes: m.votes.unwrap_or(0),
                    value: m.rating.unwrap_or(0.0),
                },
                genres: m.genres,
                runtime: m.runtime_minutes.unwrap_or(0),
                certification,
            }
        })
        .collect();

    Ok(Json(lookup_results))
}

// ─── List handler ────────────────────────────────────────────────────────────

/// GET /api/v3/movie — list all movies (or filter by tmdbId)
async fn list_movies(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MovieListQuery>,
) -> Result<Json<Vec<MovieResponse>>, ApiError> {
    let repo = MovieRepository::new(state.db.clone());

    if let Some(tmdb_id) = query.tmdb_id {
        let movie = repo
            .get_by_tmdb_id(tmdb_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch movie: {}", e)))?;
        return match movie {
            Some(m) => Ok(Json(vec![MovieResponse::from(m)])),
            None => Ok(Json(vec![])),
        };
    }

    let db_movies = repo
        .get_all()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch movies: {}", e)))?;

    let movies: Vec<MovieResponse> = db_movies.into_iter().map(MovieResponse::from).collect();
    Ok(Json(movies))
}

// ─── Get single handler ──────────────────────────────────────────────────────

/// GET /api/v3/movie/{id} — get a single movie by database ID
async fn get_movie(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<MovieResponse>, ApiError> {
    let repo = MovieRepository::new(state.db.clone());

    let movie = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch movie: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(MovieResponse::from(movie)))
}

// ─── Add handler ─────────────────────────────────────────────────────────────

/// POST /api/v3/movie — add a new movie (Radarr v3 compatible)
///
/// Accepts the same body as v5 `CreateMovieRequest` (camelCase).
/// Returns the created movie or the existing one if already present (idempotent).
async fn add_movie(
    State(state): State<Arc<AppState>>,
    Json(options): Json<CreateMovieRequest>,
) -> Result<Json<MovieResponse>, ApiError> {
    options.validate()?;

    let repo = MovieRepository::new(state.db.clone());

    // Check for existing movie by tmdbId (return it instead of erroring — idempotent for scripts)
    if options.tmdb_id > 0 {
        if let Some(existing) = repo
            .get_by_tmdb_id(options.tmdb_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        {
            tracing::info!(
                "Movie already exists (tmdb_id={}), returning existing id={}",
                options.tmdb_id,
                existing.id
            );
            return Ok(Json(MovieResponse::from(existing)));
        }
    }
    if let Some(ref imdb_id) = options.imdb_id {
        if let Some(existing) = repo
            .get_by_imdb_id(imdb_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        {
            tracing::info!(
                "Movie already exists (imdb_id={}), returning existing id={}",
                imdb_id,
                existing.id
            );
            return Ok(Json(MovieResponse::from(existing)));
        }
    }

    // Delegate to v5 create logic
    let clean = movies::clean_title(&options.title);
    let sort = clean.clone();
    let slug = movies::generate_slug(&options.title);

    let full_path = options.get_full_path();
    let root_folder_path = options.get_root_folder_path();

    let release_date = options
        .release_date
        .as_ref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let status = match options.status.as_deref() {
        Some("announced") => 1,
        Some("inCinemas") => 2,
        Some("released") => 3,
        _ => 0,
    };

    let genres_json = serde_json::to_string(&options.genres).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(&options.tags).unwrap_or_else(|_| "[]".to_string());

    // Fetch enrichment from cascade (Radarr metadata + images)
    let enrichment = if let Some(ref imdb_id) = options.imdb_id {
        movies::fetch_movie_images_and_tmdb_id(imdb_id).await
    } else {
        None
    };

    let images = if options.images.is_empty() {
        enrichment
            .as_ref()
            .map(|e| e.images.clone())
            .unwrap_or_default()
    } else {
        options.images.clone()
    };
    let images_json = serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());

    let tmdb_id = if options.tmdb_id > 0 {
        options.tmdb_id
    } else {
        enrichment.as_ref().map(|e| e.tmdb_id).unwrap_or(0)
    };

    // Merge enrichment metadata — request fields take priority
    let overview = options
        .overview
        .clone()
        .or_else(|| enrichment.as_ref().and_then(|e| e.overview.clone()));
    let studio = options
        .studio
        .clone()
        .or_else(|| enrichment.as_ref().and_then(|e| e.studio.clone()));
    let certification = options
        .certification
        .clone()
        .or_else(|| enrichment.as_ref().and_then(|e| e.certification.clone()));
    let physical_release_date = enrichment
        .as_ref()
        .and_then(|e| e.physical_release.as_deref())
        .and_then(|s| movies::parse_date_prefix(s));
    let digital_release_date = enrichment
        .as_ref()
        .and_then(|e| e.digital_release.as_deref())
        .and_then(|s| movies::parse_date_prefix(s));
    let release_date = release_date.or_else(|| {
        enrichment
            .as_ref()
            .and_then(|e| e.in_cinemas.as_deref())
            .and_then(|s| movies::parse_date_prefix(s))
    });

    use crate::core::datastore::models::MovieDbModel;

    let db_movie = MovieDbModel {
        id: 0,
        tmdb_id,
        imdb_id: options.imdb_id.clone(),
        title: options.title.clone(),
        clean_title: clean,
        sort_title: sort,
        status,
        overview,
        monitored: options.monitored,
        quality_profile_id: options.quality_profile_id,
        title_slug: slug,
        path: full_path,
        root_folder_path,
        year: options.year.unwrap_or(0),
        release_date,
        physical_release_date,
        digital_release_date,
        runtime: options.runtime.unwrap_or(0),
        studio,
        certification,
        genres: genres_json,
        tags: tags_json,
        images: images_json,
        has_file: false,
        movie_file_id: None,
        added: chrono::Utc::now(),
        last_info_sync: None,
        imdb_rating: None,
        imdb_votes: None,
    };

    let id = repo
        .insert(&db_movie)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create movie: {}", e)))?;

    tracing::info!(
        "Created movie via v3 API: id={}, title={}",
        id,
        options.title
    );

    // Create the movie folder on disk
    let movie_path = &db_movie.path;
    if !movie_path.is_empty() {
        let path = std::path::Path::new(movie_path);
        if !path.exists() {
            match tokio::fs::create_dir_all(path).await {
                Ok(()) => tracing::info!("Created movie folder: {}", movie_path),
                Err(e) => tracing::warn!("Failed to create movie folder {}: {}", movie_path, e),
            }
        }
    }

    let created = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch created movie: {}", e)))?
        .ok_or(ApiError::Internal(
            "Movie not found after creation".to_string(),
        ))?;

    Ok(Json(MovieResponse::from(created)))
}
