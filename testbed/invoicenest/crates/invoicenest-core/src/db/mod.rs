//! Database connection pool and migration management.
//!
//! Wraps a [`sqlx::PgPool`] and provides helpers for creating the pool
//! from environment configuration, running migrations, and managing
//! the connection lifecycle.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub mod schema;

/// Application database configuration.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_seconds: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost:5432/invoicenest".into()),
            max_connections: 10,
            min_connections: 2,
            acquire_timeout_seconds: 30,
        }
    }
}

/// Creates a new connection pool from the given configuration.
///
/// # Errors
///
/// Returns a [`sqlx::Error`] if the pool cannot be created or
/// a basic connectivity check fails.
pub async fn create_pool(config: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(std::time::Duration::from_secs(config.acquire_timeout_seconds))
        .connect(&config.url)
        .await?;

    Ok(pool)
}

/// Runs all pending SQLx migrations against the database.
///
/// Migrations are located in the `migrations/` directory at the crate root.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../../migrations").run(pool).await
}

/// Health-check: verifies the pool can acquire a connection.
pub async fn health_check(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .is_ok()
}
