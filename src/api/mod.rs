//! API module
//! REST API endpoints for pir9

use axum::Router;
use std::sync::Arc;

pub mod models;
pub mod v3;
pub mod v5;

use crate::web::AppState;

/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}

/// Create API router with all routes
#[allow(dead_code)]
pub fn create_api_router() -> Router<Arc<AppState>> {
    Router::new()
        .nest("/v3", v3::routes())
        .nest("/v5", v5::routes())
}
