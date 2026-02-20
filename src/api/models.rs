#![allow(dead_code)]
//! Shared API models and DTOs

use serde::{Deserialize, Serialize};

/// Standard API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

/// API error details
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Pagination parameters
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i32,
    #[serde(default = "default_page_size")]
    pub page_size: i32,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
}

fn default_page() -> i32 {
    1
}

fn default_page_size() -> i32 {
    20
}

/// Paginated response
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub page: i32,
    pub page_size: i32,
    pub total_records: i64,
    pub total_pages: i32,
    pub records: Vec<T>,
}

impl<T> PaginatedResponse<T> {
    pub fn new(page: i32, page_size: i32, total_records: i64, records: Vec<T>) -> Self {
        let total_pages = ((total_records as f64) / (page_size as f64)).ceil() as i32;
        Self {
            page,
            page_size,
            total_records,
            total_pages,
            records,
        }
    }
}

/// Command request
#[derive(Debug, Deserialize)]
pub struct CommandRequest {
    pub name: String,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
}

/// Command response
#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub id: i64,
    pub name: String,
    pub command_name: String,
    pub message: Option<String>,
    pub body: Option<serde_json::Value>,
    pub priority: String,
    pub status: String,
    pub result: Option<String>,
    pub queued: chrono::DateTime<chrono::Utc>,
    pub started: Option<chrono::DateTime<chrono::Utc>>,
    pub ended: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<String>,
    pub exception: Option<String>,
    pub trigger: String,
    pub state_change_time: chrono::DateTime<chrono::Utc>,
    pub send_updates_to_client: bool,
    pub update_scheduled_task: bool,
    pub last_execution_time: Option<chrono::DateTime<chrono::Utc>>,
}
