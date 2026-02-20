//! Data persistence layer
//! Handles database connections, migrations, and query execution
//! PostgreSQL-only implementation

use anyhow::{Context, Result};
use sqlx::{Pool, Postgres};

pub mod models;
pub mod repositories;

use crate::core::configuration::DatabaseConfig;

/// Database connection wrapper (PostgreSQL-only)
#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<Postgres>,
}

impl Database {
    /// Connect to the PostgreSQL database
    pub async fn connect(config: &DatabaseConfig) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(std::time::Duration::from_secs(config.connection_timeout_secs))
            .connect(&config.connection_string)
            .await
            .context("Failed to connect to PostgreSQL database")?;

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        // PostgreSQL uses migrations from migrations/postgres/
        // These are applied via sqlx migrate or manually
        sqlx::migrate!("./migrations/postgres")
            .run(&self.pool)
            .await
            .context("Failed to run PostgreSQL migrations")?;
        Ok(())
    }

    /// Get the PostgreSQL pool
    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

    /// Legacy method for compatibility during refactor
    /// TODO: Remove once all repositories are updated
    pub fn postgres(&self) -> Option<&Pool<Postgres>> {
        Some(&self.pool)
    }

    /// Check database connectivity
    pub async fn health_check(&self) -> Result<bool> {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await?;
        Ok(true)
    }
}

/// Unit of work pattern for transactions
pub struct UnitOfWork {
    database: Database,
}

impl UnitOfWork {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn commit(&mut self) -> Result<()> {
        // Transaction commit logic
        Ok(())
    }

    pub async fn rollback(&mut self) -> Result<()> {
        // Transaction rollback logic
        Ok(())
    }
}
