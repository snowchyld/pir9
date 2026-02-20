//! Indexer Flag API endpoints

use axum::{response::Json, routing::get, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerFlagResource {
    pub id: i32,
    pub name: String,
}

/// GET /api/v3/indexerflag
/// Returns list of indexer flags (used for bitwise flag matching in episode files)
pub async fn get_indexer_flags() -> Json<Vec<IndexerFlagResource>> {
    // Standard indexer flags as defined in Sonarr
    // These are bitwise flags: 1, 2, 4, 8, 16, etc.
    Json(vec![
        IndexerFlagResource {
            id: 1,
            name: "G_Freeleech".to_string(),
        },
        IndexerFlagResource {
            id: 2,
            name: "G_Halfleech".to_string(),
        },
        IndexerFlagResource {
            id: 4,
            name: "G_DoubleUpload".to_string(),
        },
        IndexerFlagResource {
            id: 8,
            name: "PTP_Golden".to_string(),
        },
        IndexerFlagResource {
            id: 16,
            name: "PTP_Approved".to_string(),
        },
        IndexerFlagResource {
            id: 32,
            name: "HDB_Internal".to_string(),
        },
        IndexerFlagResource {
            id: 64,
            name: "AHD_Internal".to_string(),
        },
        IndexerFlagResource {
            id: 128,
            name: "G_Scene".to_string(),
        },
        IndexerFlagResource {
            id: 256,
            name: "G_Freeleech75".to_string(),
        },
        IndexerFlagResource {
            id: 512,
            name: "G_Freeleech25".to_string(),
        },
    ])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_indexer_flags))
}
