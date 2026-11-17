//! Database pool creation and embedded migrations.

use std::time::Duration;

use sqlx::{
    migrate::{MigrateError, Migrator},
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};

/// All migrations in `app/middleware/migrations`, embedded at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Creates a connection pool. Connections are established lazily-but-verified:
/// the first connection is opened before returning so configuration errors surface early.
pub async fn connect(options: PgConnectOptions, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
}

/// Applies pending migrations. Migrations are additive; nothing here drops user data.
pub async fn migrate(pool: &PgPool) -> Result<(), MigrateError> {
    MIGRATOR.run(pool).await
}

/// Cheap connectivity check used by the readiness probe.
pub async fn ping(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await.map(|_| ())
}
