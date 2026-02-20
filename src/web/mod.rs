#![allow(dead_code, unused_imports)]
//! Web module
//! WebSocket handlers and static file serving

use axum::{
    body::Body,
    extract::{
        ws::{Message as WsMessage, WebSocket},
        State, WebSocketUpgrade,
    },
    http::{Request, Uri},
    response::{Json, Response},
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tracing::{error, info};

use crate::core::{
    configuration::AppConfig,
    datastore::Database,
    imdb::ImdbClient,
    messaging::{EventBus, HybridEventBus},
    metadata::MetadataService,
    scanner::ScanResultConsumer,
    scheduler::JobScheduler,
};

/// Application state shared across handlers
#[derive(Debug)]
pub struct AppState {
    pub config: parking_lot::RwLock<AppConfig>,
    pub db: Database,
    pub scheduler: JobScheduler,
    /// IMDB microservice client
    pub imdb_client: ImdbClient,
    /// Unified metadata service (IMDB + Skyhook)
    pub metadata_service: MetadataService,
    /// Event bus for real-time updates (local or Redis-backed)
    pub event_bus: EventBus,
    /// Hybrid event bus for distributed scanning (if Redis enabled)
    pub hybrid_event_bus: Option<HybridEventBus>,
    /// Cancellation tokens for running commands (keyed by command ID)
    pub command_tokens: dashmap::DashMap<i64, tokio_util::sync::CancellationToken>,
    /// Scan result consumer for registering download imports (set in server mode)
    pub scan_result_consumer: tokio::sync::OnceCell<Arc<ScanResultConsumer>>,
}

impl AppState {
    /// Create application state with local event bus only
    pub fn new(
        config: AppConfig,
        db: Database,
        scheduler: JobScheduler,
    ) -> anyhow::Result<Arc<Self>> {
        let imdb_client = ImdbClient::from_env();
        let tvmaze_client = crate::core::tvmaze::TvMazeClient::new();
        let metadata_service = MetadataService::new(imdb_client.clone(), tvmaze_client);
        Ok(Arc::new(Self {
            config: parking_lot::RwLock::new(config),
            db,
            scheduler,
            imdb_client,
            metadata_service,
            event_bus: EventBus::new(),
            hybrid_event_bus: None,
            command_tokens: dashmap::DashMap::new(),
            scan_result_consumer: tokio::sync::OnceCell::new(),
        }))
    }

    /// Create application state with Redis-backed event bus for distributed mode
    #[cfg(feature = "redis-events")]
    pub async fn new_with_redis(
        config: AppConfig,
        db: Database,
        scheduler: JobScheduler,
        redis_url: &str,
    ) -> anyhow::Result<Arc<Self>> {
        use tracing::info;

        info!("Initializing Redis event bus for distributed mode");
        let hybrid_bus = HybridEventBus::new_redis(redis_url).await?;

        // Start the Redis subscriber in the background
        let bus_clone = hybrid_bus.clone();
        tokio::spawn(async move {
            if let Err(e) = bus_clone.start_redis_subscriber().await {
                tracing::error!("Redis subscriber error: {}", e);
            }
        });

        info!("Redis event bus initialized");

        let imdb_client = ImdbClient::from_env();
        let tvmaze_client = crate::core::tvmaze::TvMazeClient::new();
        let metadata_service = MetadataService::new(imdb_client.clone(), tvmaze_client);
        Ok(Arc::new(Self {
            config: parking_lot::RwLock::new(config),
            db,
            scheduler,
            imdb_client,
            metadata_service,
            event_bus: EventBus::new(),
            hybrid_event_bus: Some(hybrid_bus),
            command_tokens: dashmap::DashMap::new(),
            scan_result_consumer: tokio::sync::OnceCell::new(),
        }))
    }

    /// Fallback when redis-events feature is not enabled
    #[cfg(not(feature = "redis-events"))]
    pub async fn new_with_redis(
        _config: AppConfig,
        _db: Database,
        _scheduler: JobScheduler,
        _redis_url: &str,
    ) -> anyhow::Result<Arc<Self>> {
        anyhow::bail!("Redis support requires the 'redis-events' feature (enabled by default). Was this built with --no-default-features?")
    }
}

/// WebSocket handler for real-time updates
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    info!("New WebSocket connection established");

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to events
    let mut event_rx = state.event_bus.subscribe();

    // Spawn task to send events to client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let msg = serde_json::to_string(&event).unwrap_or_default();
            if sender.send(WsMessage::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from client
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            WsMessage::Text(text) => {
                // Handle client commands
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        handle_client_message(client_msg, &state).await;
                    }
                    Err(e) => {
                        error!("Failed to parse client message: {}", e);
                    }
                }
            }
            WsMessage::Close(_) => {
                info!("WebSocket connection closed by client");
                break;
            }
            _ => {}
        }
    }

    // Clean up
    send_task.abort();
    info!("WebSocket connection closed");
}

async fn handle_client_message(msg: ClientMessage, _state: &AppState) {
    match msg {
        ClientMessage::Ping => {
            // Respond with pong
        }
        ClientMessage::Subscribe { channel } => {
            info!("Client subscribed to channel: {}", channel);
        }
        ClientMessage::Unsubscribe { channel } => {
            info!("Client unsubscribed from channel: {}", channel);
        }
        ClientMessage::Command { name, args } => {
            info!("Received command: {} with args: {:?}", name, args);
            // Execute command
        }
    }
}

/// Messages sent by WebSocket clients
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Ping,
    Subscribe {
        channel: String,
    },
    Unsubscribe {
        channel: String,
    },
    Command {
        name: String,
        args: Option<serde_json::Value>,
    },
}

/// Initialize.json response for frontend bootstrap
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub api_root: String,
    pub api_key: String,
    pub release: String,
    pub version: String,
    pub instance_name: String,
    pub theme: String,
    pub branch: String,
    pub analytics: bool,
    pub user_hash: String,
    pub url_base: String,
    pub is_production: bool,
    pub is_admin: bool,
    pub authentication: String,
    #[serde(rename = "isAuthenticated")]
    pub is_authenticated: bool,
}

/// Handler for /initialize.json endpoint
pub async fn initialize_json(State(state): State<Arc<AppState>>) -> Json<InitializeResponse> {
    Json(InitializeResponse {
        api_root: "/api/v3".to_string(),
        api_key: state
            .config
            .read()
            .security
            .secret_key
            .chars()
            .take(32)
            .collect(),
        release: "develop".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        instance_name: "pir9".to_string(),
        theme: "auto".to_string(),
        branch: "develop".to_string(),
        analytics: false,
        user_hash: "anonymous".to_string(),
        url_base: "".to_string(),
        is_production: cfg!(not(debug_assertions)),
        is_admin: true,
        authentication: "none".to_string(),
        is_authenticated: true,
    })
}

/// Middleware layer for case-insensitive API routing
#[derive(Clone)]
pub struct NormalizeApiPathLayer;

impl<S> Layer<S> for NormalizeApiPathLayer {
    type Service = NormalizeApiPathService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        NormalizeApiPathService { inner }
    }
}

/// Service that normalizes API paths to lowercase for case-insensitive routing
#[derive(Clone)]
pub struct NormalizeApiPathService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for NormalizeApiPathService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let path = req.uri().path();

        // Only normalize paths that start with /api/
        if path.starts_with("/api/") {
            let normalized = normalize_api_path(path);
            if normalized != path {
                // Rebuild the URI with the normalized path
                let mut parts = req.uri().clone().into_parts();
                let path_and_query = if let Some(query) = req.uri().query() {
                    format!("{}?{}", normalized, query)
                } else {
                    normalized
                };
                parts.path_and_query = Some(path_and_query.parse().unwrap());
                if let Ok(new_uri) = Uri::from_parts(parts) {
                    *req.uri_mut() = new_uri;
                }
            }
        }

        self.inner.call(req)
    }
}

/// Normalize API path segments to lowercase for case-insensitive routing
fn normalize_api_path(path: &str) -> String {
    // For API paths, normalize segments to lowercase to match registered routes
    let parts: Vec<&str> = path.split('/').collect();
    let normalized: Vec<String> = parts
        .iter()
        .map(|segment| {
            // API segments (after /api/v3 or /api/v5) should be lowercase
            segment.to_lowercase()
        })
        .collect();

    normalized.join("/")
}
