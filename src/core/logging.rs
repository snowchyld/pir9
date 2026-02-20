#![allow(dead_code)]
//! Application event logging service
//! Logs significant application events to the database for the Events page

use tokio::sync::RwLock;

use crate::core::datastore::Database;
use crate::core::datastore::repositories::LogRepository;

/// Log levels for database logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Fatal => "fatal",
        }
    }
}

/// Application event logger
///
/// This struct provides a way to log significant application events
/// to the database for display in the Events page.
#[derive(Clone)]
pub struct AppLogger {
    db: Database,
}

impl AppLogger {
    /// Create a new application logger
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Log an event to the database
    pub async fn log(&self, level: LogLevel, logger: &str, message: &str) -> anyhow::Result<i64> {
        let repo = LogRepository::new(self.db.clone());
        repo.insert(level.as_str(), logger, message, None, None).await
    }

    /// Log an event with exception details
    pub async fn log_exception(
        &self,
        level: LogLevel,
        logger: &str,
        message: &str,
        exception: &str,
        exception_type: Option<&str>,
    ) -> anyhow::Result<i64> {
        let repo = LogRepository::new(self.db.clone());
        repo.insert(level.as_str(), logger, message, Some(exception), exception_type).await
    }

    /// Log an info event
    pub async fn info(&self, logger: &str, message: &str) -> anyhow::Result<i64> {
        self.log(LogLevel::Info, logger, message).await
    }

    /// Log a warning event
    pub async fn warn(&self, logger: &str, message: &str) -> anyhow::Result<i64> {
        self.log(LogLevel::Warn, logger, message).await
    }

    /// Log an error event
    pub async fn error(&self, logger: &str, message: &str) -> anyhow::Result<i64> {
        self.log(LogLevel::Error, logger, message).await
    }

    /// Log an error with exception details
    pub async fn error_with_exception(
        &self,
        logger: &str,
        message: &str,
        exception: &str,
    ) -> anyhow::Result<i64> {
        self.log_exception(LogLevel::Error, logger, message, exception, None).await
    }
}

/// Global app logger holder (initialized after database connection)
static APP_LOGGER: RwLock<Option<AppLogger>> = RwLock::const_new(None);

/// Initialize the global app logger
pub async fn init_app_logger(db: Database) {
    let logger = AppLogger::new(db);
    *APP_LOGGER.write().await = Some(logger);
}

/// Get the global app logger
pub async fn get_app_logger() -> Option<AppLogger> {
    APP_LOGGER.read().await.clone()
}

/// Log an info event using the global logger
pub async fn log_info(logger: &str, message: &str) {
    if let Some(app_logger) = get_app_logger().await {
        if let Err(e) = app_logger.info(logger, message).await {
            tracing::warn!("Failed to write to event log: {}", e);
        }
    }
}

/// Log a warning event using the global logger
pub async fn log_warn(logger: &str, message: &str) {
    if let Some(app_logger) = get_app_logger().await {
        if let Err(e) = app_logger.warn(logger, message).await {
            tracing::warn!("Failed to write to event log: {}", e);
        }
    }
}

/// Log an error event using the global logger
pub async fn log_error(logger: &str, message: &str) {
    if let Some(app_logger) = get_app_logger().await {
        if let Err(e) = app_logger.error(logger, message).await {
            tracing::warn!("Failed to write to event log: {}", e);
        }
    }
}

/// Log an error with exception details using the global logger
pub async fn log_error_exception(logger: &str, message: &str, exception: &str) {
    if let Some(app_logger) = get_app_logger().await {
        if let Err(e) = app_logger.error_with_exception(logger, message, exception).await {
            tracing::warn!("Failed to write to event log: {}", e);
        }
    }
}
