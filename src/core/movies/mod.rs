//! Movies domain module
//! Contains models and services for Movies and MovieFiles
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod models;
pub mod services;
pub mod repositories;
pub mod events;

pub use models::*;

use serde::{Deserialize, Serialize};

/// Movie status types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, Default)]
#[repr(i32)]
pub enum MovieStatusType {
    #[default]
    TBA = 0,
    Announced = 1,
    InCinemas = 2,
    Released = 3,
    Deleted = 4,
}

/// Movie statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieStatistics {
    pub size_on_disk: i64,
    pub has_file: bool,
}
