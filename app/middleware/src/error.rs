//! Errors raised by the Discover aggregator ([`crate::routine`]).

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing: {0}")]
    Missing(String),
    #[error("HTTP request to GitHub failed: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("GitHub GraphQL API returned errors: {0}")]
    GraphQL(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
