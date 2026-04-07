#![allow(dead_code)]
//! Audiobook domain events
//! Events related to audiobooks and audiobook chapters

use serde::{Deserialize, Serialize};

/// Audiobook added event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookAddedEvent {
    pub audiobook_id: i64,
    pub title: String,
    pub isbn: Option<String>,
    pub asin: Option<String>,
}

/// Audiobook updated event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookUpdatedEvent {
    pub audiobook_id: i64,
    pub title: String,
    pub changes: Vec<String>,
}

/// Audiobook deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookDeletedEvent {
    pub audiobook_id: i64,
    pub title: String,
    pub deleted_files: bool,
}

/// Audiobook refreshed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookRefreshedEvent {
    pub audiobook_id: i64,
    pub title: String,
}

/// Audiobook file imported event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookFileImportedEvent {
    pub audiobook_id: i64,
    pub audiobook_file_id: i64,
}

/// Audiobook file deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookFileDeletedEvent {
    pub audiobook_id: i64,
    pub audiobook_file_id: i64,
    pub reason: DeleteReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeleteReason {
    MissingFromDisk,
    Manual,
    Upgrade,
}
