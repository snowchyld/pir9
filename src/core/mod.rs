//! Core business logic module
//! Contains all domain-specific implementations

pub mod configuration;
pub mod datastore;
pub mod download;
pub mod imdb;
pub mod indexers;
pub mod logging;
pub mod mediafiles;
pub mod messaging;
pub mod notifications;
pub mod parser;
pub mod profiles;
pub mod queue;
pub mod scanner;
pub mod scheduler;
pub mod tv;
pub mod worker;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state container
#[derive(Debug)]
pub struct AppStateInner {
    pub config: configuration::AppConfig,
    pub db: datastore::Database,
    pub scheduler: scheduler::JobScheduler,
}

impl AppStateInner {
    pub fn new(
        config: configuration::AppConfig,
        db: datastore::Database,
        scheduler: scheduler::JobScheduler,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            config,
            db,
            scheduler,
        })
    }
}

pub type AppState = Arc<RwLock<AppStateInner>>;
