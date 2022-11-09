use std::{error, fmt::Display};

#[derive(Debug)]
pub enum Error {
    Missing(String),
    Reqwest(reqwest::Error),
    Sqlx(sqlx::Error),
    Migrate(sqlx::migrate::MigrateError),
    EnvVar(std::env::VarError),
    Other(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::Reqwest(e)
    }
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Self::Sqlx(e)
    }
}

impl From<sqlx::migrate::MigrateError> for Error {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        Self::Migrate(e)
    }
}

impl From<std::env::VarError> for Error {
    fn from(e: std::env::VarError) -> Self {
        Self::EnvVar(e)
    }
}

impl error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
