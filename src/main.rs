//! pir9 - Smart PVR for TV and anime
//! A modern Rust media management application

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::Parser;
use tokio::signal;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, normalize_path::NormalizePathLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use tracing::{info, warn};

mod api;
mod cli_args;
mod core;
mod web;

use crate::cli_args::{Args, RunMode};
use crate::core::{
    configuration::{AppConfig, DatabaseConfig},
    datastore::Database,
    scheduler::JobScheduler,
};
use crate::web::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing/logging
    init_tracing();

    // Parse command-line arguments
    let args = Args::parse();

    // Validate arguments
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    info!("=== pir9 v{} ===", env!("CARGO_PKG_VERSION"));
    info!("Mode: {:?}", args.mode);
    if let Some(ref redis_url) = args.redis_url {
        info!("Redis URL: {}", redis_url);
    }
    if !args.worker_paths.is_empty() {
        info!("Worker paths: {:?}", args.worker_paths);
    }

    // Worker mode runs a completely different code path
    if args.should_run_worker() {
        return run_worker_mode(&args).await;
    }

    // Server and All modes continue with the normal startup
    run_server_mode(&args).await
}

/// Run in worker mode - scan files and publish results to Redis
async fn run_worker_mode(args: &Args) -> Result<()> {
    use crate::core::worker::WorkerRunner;

    let redis_url = args
        .redis_url
        .as_ref()
        .expect("Redis URL validated in args.validate()");

    let worker_id = args
        .worker_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    info!("Worker ID: {}", worker_id);
    info!("Worker paths: {:?}", args.worker_paths);

    let worker = WorkerRunner::new(redis_url, &worker_id, args.worker_paths.clone())
        .await
        .context("Failed to initialize worker")?;

    // Run the worker (blocks until shutdown signal)
    worker.run().await
}

/// Run in server or all mode - web server + scheduler
async fn run_server_mode(args: &Args) -> Result<()> {
    // Load configuration
    let mut config = AppConfig::load().context("Failed to load application configuration")?;

    // Override port if specified on command line
    if let Some(port) = args.port {
        config.server.port = port;
    }

    info!("Configuration loaded successfully");

    // Initialize database connection
    let db_config = DatabaseConfig::from(&config);
    let database = Database::connect(&db_config)
        .await
        .context("Failed to connect to database")?;

    info!("Database connection established");

    // Run database migrations
    database
        .migrate()
        .await
        .context("Failed to run database migrations")?;

    // Initialize the application event logger
    crate::core::logging::init_app_logger(database.clone()).await;

    // Log application startup
    let mode_str = match args.mode {
        RunMode::All => "standalone",
        RunMode::Server => "server (distributed)",
        RunMode::Worker => unreachable!(),
    };
    crate::core::logging::log_info(
        "ApplicationStartup",
        &format!(
            "pir9 v{} started in {} mode",
            env!("CARGO_PKG_VERSION"),
            mode_str
        ),
    )
    .await;

    // Clean up stale commands from previous server sessions
    {
        use crate::core::datastore::repositories::CommandRepository;
        let repo = CommandRepository::new(database.clone());
        match repo.cleanup_stale().await {
            Ok(count) if count > 0 => {
                info!("Cleaned up {} stale commands from previous session", count);
            }
            Ok(_) => {}
            Err(e) => {
                warn!("Failed to clean up stale commands: {}", e);
            }
        }
    }

    // Log tracked download count — don't process yet.
    // Download clients (qBittorrent etc.) may still be initializing and return
    // empty results, which would cause process_queue() to delete all tracked
    // records. The first scheduled ProcessDownloadQueue (1 min) handles it.
    {
        use crate::core::datastore::repositories::TrackedDownloadRepository;
        let repo = TrackedDownloadRepository::new(database.clone());
        match repo.get_all_active().await {
            Ok(active) => info!(
                "Startup: {} active tracked downloads in database",
                active.len()
            ),
            Err(e) => warn!("Startup: failed to check tracked downloads: {}", e),
        }
    }

    // Initialize job scheduler with metadata service for IMDB-enriched refreshes
    let mut scheduler =
        JobScheduler::new(database.clone()).context("Failed to initialize job scheduler")?;
    {
        let imdb_client = crate::core::imdb::ImdbClient::from_env();
        let tvmaze_client = crate::core::tvmaze::TvMazeClient::new();
        let tvdb_client = crate::core::tvdb::TvdbClient::from_env();
        let metadata_service =
            crate::core::metadata::MetadataService::new(imdb_client, tvmaze_client, tvdb_client);
        scheduler.set_metadata_service(metadata_service);
        scheduler.set_media_config(config.media.clone());
    }

    // Create application state (with Redis event bus if in server mode)
    let state = if args.mode == RunMode::Server {
        let redis_url = args
            .redis_url
            .as_ref()
            .expect("Redis URL validated in args.validate()");
        info!("Initializing distributed scanning mode with Redis");
        let state =
            AppState::new_with_redis(config.clone(), database.clone(), scheduler, redis_url)
                .await?;
        info!("Distributed scanning enabled - file scans will be delegated to workers");
        state
    } else {
        info!("Running in standalone mode - file scans will be performed locally");
        AppState::new(config.clone(), database.clone(), scheduler)?
    };

    // Start notification service event listener
    {
        use crate::core::notifications::NotificationService;
        let notification_service = std::sync::Arc::new(NotificationService::new(
            database.clone(),
            state.event_bus.clone(),
        ));
        tokio::spawn(notification_service.start_event_listener());
        info!("Notification service started");
    }

    // Start distributed scanning services if in server mode
    if args.mode == RunMode::Server {
        if let Some(ref hybrid_bus) = state.hybrid_event_bus {
            use crate::core::scanner::{
                JobTrackerService, ScanResultConsumer, WorkerRegistryService,
            };

            // Initialize Redis streams and consumer groups
            hybrid_bus
                .ensure_streams()
                .await
                .context("Failed to initialize Redis streams")?;

            // Start result stream reader — reads durable results from workers
            // and feeds them into the local broadcast for the consumer to process
            let bus_for_results = hybrid_bus.clone();
            tokio::spawn(async move {
                if let Err(e) = bus_for_results.start_result_stream_reader().await {
                    tracing::error!("Result stream reader error: {}", e);
                }
            });
            info!("Result stream reader started");

            // Start scan result consumer (with media config for download import naming)
            let media_config = state.config.read().media.clone();
            let mut consumer_inner = ScanResultConsumer::new(
                database.clone(),
                hybrid_bus.clone(),
            );
            consumer_inner.set_media_config(media_config);
            // Bridge scan progress to WebSocket event bus so frontend gets real-time updates
            consumer_inner.set_ws_event_bus(state.event_bus.clone());
            let consumer = std::sync::Arc::new(consumer_inner);

            // Store in AppState for command.rs to register download imports
            let _ = state.scan_result_consumer.set(consumer.clone());

            // Wire scheduler with Redis handles so scheduled jobs (e.g.
            // DownloadedEpisodesScan) also dispatch to workers
            state.scheduler.set_hybrid_event_bus(hybrid_bus.clone());
            state.scheduler.set_scan_result_consumer(consumer.clone());

            tokio::spawn(consumer.run());
            info!("Scan result consumer started");

            // Start job tracker (handles timeouts and retries)
            let job_tracker =
                std::sync::Arc::new(JobTrackerService::new(database.clone(), hybrid_bus.clone()));
            tokio::spawn(job_tracker.run());
            info!("Job tracker service started");

            // Start worker registry (tracks online workers)
            let worker_registry =
                std::sync::Arc::new(WorkerRegistryService::new(hybrid_bus.clone()));
            tokio::spawn(worker_registry.run());
            info!("Worker registry service started");

            info!("Distributed scanning mode active");
        }
    }

    // Build the router
    let app = create_router(state);

    // Start the server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    info!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("Failed to bind to address")?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server error")?;

    info!("pir9 shutdown complete");
    Ok(())
}

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,pir9=debug,tower_http=debug"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health check
        .route("/health", get(api::health_check))
        // Initialize.json for frontend bootstrap
        .route("/initialize.json", get(web::initialize_json))
        // API v5 routes
        .nest("/api/v5", api::v5::routes())
        // Legacy API v3 routes (for compatibility)
        .nest("/api/v3", api::v3::routes())
        // WebSocket endpoint for real-time updates
        .route("/ws", get(web::websocket_handler))
        // MediaCover routes - serve/proxy artwork (must be before static files)
        .route(
            "/MediaCover/Series/{series_id}/{filename}",
            get(media_cover_handler),
        )
        .route(
            "/MediaCover/Movies/{movie_id}/{filename}",
            get(movie_media_cover_handler),
        )
        // Static files (frontend) with SPA fallback
        // Using fallback_service so explicit routes above take precedence
        .fallback_service(
            tower_http::services::ServeDir::new("frontend/dist").fallback(
                tower_http::services::ServeFile::new("frontend/dist/index.html"),
            ),
        )
        // Add middleware — log all API requests at INFO level with method, path, status
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    let path = request.uri().path();
                    let method = request.method().as_str();
                    // Log API requests at INFO, everything else at DEBUG
                    if path.starts_with("/api/") {
                        tracing::info_span!("request", %method, %path,
                            query = request.uri().query().unwrap_or(""))
                    } else {
                        tracing::debug_span!("request", %method, %path)
                    }
                })
                .on_response(DefaultOnResponse::new().level(Level::INFO))
        )
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(NormalizePathLayer::trim_trailing_slash())
        .with_state(state)
}

/// Handler for /MediaCover/Series/:series_id/:filename
/// Fetches images from Skyhook and caches them locally
async fn media_cover_handler(
    State(state): State<Arc<AppState>>,
    Path((series_id, filename)): Path<(i64, String)>,
) -> impl IntoResponse {
    use crate::core::datastore::repositories::SeriesRepository;

    // Parse the cover type from filename (e.g., "poster-250.jpg" -> "poster")
    let cover_type = filename
        .split('-')
        .next()
        .unwrap_or(&filename)
        .split('.')
        .next()
        .unwrap_or("poster");

    tracing::debug!(
        "MediaCover request: series_id={}, filename={}, cover_type={}",
        series_id,
        filename,
        cover_type
    );

    let content_type = if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
        "image/jpeg"
    } else if filename.ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };

    // First, check if we have a cached local file
    let cache_dir = format!("cache/MediaCover/Series/{}", series_id);
    let cache_path = format!("{}/{}", cache_dir, filename);

    if let Ok(data) = tokio::fs::read(&cache_path).await {
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "max-age=86400"),
            ],
            data,
        )
            .into_response();
    }

    // Try fallback sizes if exact size not found (e.g., poster-1000.jpg -> poster-500.jpg -> poster-250.jpg)
    let fallback_sizes = ["500", "250", "1000"];
    let extension = if filename.ends_with(".png") {
        "png"
    } else {
        "jpg"
    };

    for size in fallback_sizes {
        let fallback_path = format!("{}/{}-{}.{}", cache_dir, cover_type, size, extension);
        if let Ok(data) = tokio::fs::read(&fallback_path).await {
            tracing::debug!(
                "Using fallback image: {} for requested {}",
                fallback_path,
                filename
            );
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CACHE_CONTROL, "max-age=86400"),
                ],
                data,
            )
                .into_response();
        }
    }

    // No cached file - look up series to get TVDB ID
    let repo = SeriesRepository::new(state.db.clone());
    let series = match repo.get_by_id(series_id).await {
        Ok(Some(s)) => s,
        _ => {
            return (StatusCode::NOT_FOUND, "Series not found").into_response();
        }
    };

    // Fetch image URLs from Skyhook
    let skyhook_url = format!(
        "http://skyhook.sonarr.tv/v1/tvdb/shows/en/{}",
        series.tvdb_id
    );

    let client = reqwest::Client::new();
    let response = match client
        .get(&skyhook_url)
        .header("User-Agent", "pir9/0.1.0")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => {
            return (StatusCode::NOT_FOUND, "Failed to fetch from Skyhook").into_response();
        }
    };

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SkyhookResponse {
        images: Option<Vec<SkyhookImage>>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SkyhookImage {
        cover_type: String,
        url: String,
    }

    let skyhook: SkyhookResponse = match response.json().await {
        Ok(s) => s,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Failed to parse Skyhook response").into_response();
        }
    };

    // Find the matching image URL
    let images = skyhook.images.unwrap_or_default();

    // Log available image types for debugging
    let available_types: Vec<String> = images.iter().map(|i| i.cover_type.clone()).collect();
    tracing::debug!(
        "Skyhook returned {} images: {:?}, looking for: {}",
        images.len(),
        available_types,
        cover_type
    );

    let image_url = images
        .iter()
        .find(|img| img.cover_type.to_lowercase() == cover_type.to_lowercase())
        .or_else(|| {
            images
                .iter()
                .find(|img| img.cover_type.eq_ignore_ascii_case("fanart"))
        })
        .map(|img| img.url.clone());

    let image_url = match image_url {
        Some(url) => {
            tracing::debug!("Found image URL for {}: {}", cover_type, url);
            url
        }
        None => {
            tracing::warn!(
                "Image type '{}' not found for series {}. Available: {:?}",
                cover_type,
                series_id,
                available_types
            );
            return (StatusCode::NOT_FOUND, "Image type not found").into_response();
        }
    };

    // Fetch the actual image
    let image_response = match client.get(&image_url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => {
            return (StatusCode::NOT_FOUND, "Failed to fetch image").into_response();
        }
    };

    let image_data = match image_response.bytes().await {
        Ok(b) => b.to_vec(),
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Failed to read image data").into_response();
        }
    };

    // Cache the image for future requests (fire and forget)
    let cache_dir_clone = cache_dir.clone();
    let cache_path_clone = cache_path.clone();
    let image_data_clone = image_data.clone();
    tokio::spawn(async move {
        if tokio::fs::create_dir_all(&cache_dir_clone).await.is_ok() {
            let _ = tokio::fs::write(&cache_path_clone, &image_data_clone).await;
        }
    });

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "max-age=86400"),
        ],
        image_data,
    )
        .into_response()
}

/// Handler for /MediaCover/Movies/:movie_id/:filename
/// Fetches movie images via Radarr metadata API and caches them locally
async fn movie_media_cover_handler(
    State(state): State<Arc<AppState>>,
    Path((movie_id, filename)): Path<(i64, String)>,
) -> impl IntoResponse {
    use crate::core::datastore::repositories::MovieRepository;

    let cover_type = filename
        .split('-')
        .next()
        .unwrap_or(&filename)
        .split('.')
        .next()
        .unwrap_or("poster");

    tracing::debug!(
        "Movie MediaCover request: movie_id={}, filename={}, cover_type={}",
        movie_id,
        filename,
        cover_type
    );

    let content_type = if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
        "image/jpeg"
    } else if filename.ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };

    // Check local cache first
    let cache_dir = format!("cache/MediaCover/Movies/{}", movie_id);
    let cache_path = format!("{}/{}", cache_dir, filename);

    if let Ok(data) = tokio::fs::read(&cache_path).await {
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "max-age=86400"),
            ],
            data,
        )
            .into_response();
    }

    // Try fallback sizes
    let fallback_sizes = ["500", "250", "1000"];
    let extension = if filename.ends_with(".png") {
        "png"
    } else {
        "jpg"
    };

    for size in fallback_sizes {
        let fallback_path = format!("{}/{}-{}.{}", cache_dir, cover_type, size, extension);
        if let Ok(data) = tokio::fs::read(&fallback_path).await {
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CACHE_CONTROL, "max-age=86400"),
                ],
                data,
            )
                .into_response();
        }
    }

    // No cache hit — look up movie in DB to get imdb_id and stored images
    let repo = MovieRepository::new(state.db.clone());
    let movie = match repo.get_by_id(movie_id).await {
        Ok(Some(m)) => m,
        _ => {
            return (StatusCode::NOT_FOUND, "Movie not found").into_response();
        }
    };

    // Check if images JSON has a remote_url for this cover type
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct StoredImage {
        cover_type: String,
        #[serde(default)]
        remote_url: Option<String>,
    }

    let stored_images: Vec<StoredImage> = serde_json::from_str(&movie.images).unwrap_or_default();
    let stored_url = stored_images
        .into_iter()
        .find(|img| img.cover_type.eq_ignore_ascii_case(cover_type))
        .and_then(|img| img.remote_url);

    // Resolve the image URL: use stored remote_url, or fall back to external APIs
    let image_url = if let Some(url) = stored_url {
        Some(url)
    } else if let Some(ref imdb_id) = movie.imdb_id {
        // Try Radarr metadata proxy first (has poster + fanart)
        let radarr_url = format!("https://api.radarr.video/v1/movie/imdb/{}", imdb_id);
        let client = reqwest::Client::new();
        let radarr_result = match client
            .get(&radarr_url)
            .header("User-Agent", format!("pir9/{}", env!("CARGO_PKG_VERSION")))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                #[derive(serde::Deserialize)]
                #[allow(non_snake_case)]
                struct RadarrImage {
                    CoverType: String,
                    Url: String,
                }
                #[derive(serde::Deserialize)]
                #[allow(non_snake_case)]
                struct RadarrMovie {
                    Images: Vec<RadarrImage>,
                }

                match resp.json::<Vec<RadarrMovie>>().await {
                    Ok(movies) => movies.into_iter().next().and_then(|radarr| {
                        let radarr_cover = match cover_type {
                            "poster" => "Poster",
                            "fanart" | "backdrop" => "Fanart",
                            _ => return None,
                        };
                        radarr
                            .Images
                            .into_iter()
                            .find(|img| img.CoverType == radarr_cover)
                            .map(|img| img.Url)
                    }),
                    Err(e) => {
                        tracing::warn!("Failed to parse Radarr response for {}: {}", imdb_id, e);
                        None
                    }
                }
            }
            _ => None,
        };

        // If Radarr didn't have it, try Fanart.tv (has logo, clearart, banner, thumb, disc)
        if radarr_result.is_some() {
            radarr_result
        } else {
            crate::api::v5::movies::fetch_fanart_image_url(imdb_id, cover_type).await
        }
    } else {
        None
    };

    let image_url = match image_url {
        Some(url) => url,
        None => {
            return (StatusCode::NOT_FOUND, "No image available for this movie").into_response();
        }
    };

    // Fetch the image from CDN
    let client = reqwest::Client::new();
    let image_response = match client.get(&image_url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => {
            return (StatusCode::NOT_FOUND, "Failed to fetch image").into_response();
        }
    };

    let image_data = match image_response.bytes().await {
        Ok(b) => b.to_vec(),
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Failed to read image data").into_response();
        }
    };

    // Cache locally (fire and forget)
    let cache_dir_clone = cache_dir.clone();
    let cache_path_clone = cache_path.clone();
    let image_data_clone = image_data.clone();
    tokio::spawn(async move {
        if tokio::fs::create_dir_all(&cache_dir_clone).await.is_ok() {
            let _ = tokio::fs::write(&cache_path_clone, &image_data_clone).await;
        }
    });

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "max-age=86400"),
        ],
        image_data,
    )
        .into_response()
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, starting graceful shutdown...");
        }
        _ = terminate => {
            info!("Received SIGTERM, starting graceful shutdown...");
        }
    }
}
