//! Runtime configuration from environment variables. See `.env.example` for documentation.

use std::{env, fmt, str::FromStr, time::Duration};

use sqlx::postgres::PgConnectOptions;

#[derive(Debug, thiserror::Error)]
#[error("configuration error: {0}")]
pub struct ConfigError(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Clone)]
pub struct Config {
    pub bind_address: String,
    pub database: PgConnectOptions,
    pub db_max_connections: u32,
    pub github_api_url: String,
    pub github_timeout: Duration,
    /// How long a validated user token is trusted before re-checking it with GitHub.
    pub auth_cache_ttl: Duration,
    /// Upper bound on pages (of 100) fetched by a repository sync.
    pub sync_max_pages: u32,
    pub discover: Option<DiscoverConfig>,
    pub commander_token: Option<String>,
    pub admin_token: Option<String>,
    pub log_format: LogFormat,
}

#[derive(Clone)]
pub struct DiscoverConfig {
    pub graphql_url: String,
    pub token: String,
    /// `None` disables the background job; the command endpoint still works.
    pub interval: Option<Duration>,
    pub batch: u16,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bind_address", &self.bind_address)
            .field("database_host", &self.database.get_host())
            .field("database_name", &self.database.get_database())
            .field("db_max_connections", &self.db_max_connections)
            .field("github_api_url", &self.github_api_url)
            .field("sync_max_pages", &self.sync_max_pages)
            .field("discover_enabled", &self.discover.is_some())
            .field("commander_token_set", &self.commander_token.is_some())
            .field("admin_token_set", &self.admin_token.is_some())
            .finish()
    }
}

fn var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn parse<T: FromStr>(name: &str, default: T) -> Result<T, ConfigError>
where
    T::Err: fmt::Display,
{
    match var(name) {
        None => Ok(default),
        Some(v) => v
            .parse()
            .map_err(|e| ConfigError(format!("{name}={v:?} is invalid: {e}"))),
    }
}

/// Secrets shorter than this are rejected so a placeholder can't become a real credential.
const MIN_TOKEN_LEN: usize = 16;

fn secret(name: &str) -> Result<Option<String>, ConfigError> {
    match var(name) {
        Some(v) if v.len() < MIN_TOKEN_LEN => Err(ConfigError(format!(
            "{name} must be at least {MIN_TOKEN_LEN} characters"
        ))),
        other => Ok(other),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database = match var("DATABASE_URL") {
            Some(url) => url
                .parse::<PgConnectOptions>()
                .map_err(|e| ConfigError(format!("DATABASE_URL is invalid: {e}")))?,
            None => {
                // Individual settings; POSTGRESDB_SERVICE_* are injected by Kubernetes for the
                // `postgresdb` Service.
                let host = var("POSTGRES_HOST")
                    .or_else(|| var("POSTGRESDB_SERVICE_HOST"))
                    .unwrap_or_else(|| "localhost".into());
                let port = match var("POSTGRES_PORT").or_else(|| var("POSTGRESDB_SERVICE_PORT")) {
                    Some(p) => p
                        .parse::<u16>()
                        .map_err(|e| ConfigError(format!("invalid Postgres port {p:?}: {e}")))?,
                    None => 5432,
                };
                let user = var("POSTGRES_USER").ok_or_else(|| {
                    ConfigError("set DATABASE_URL or POSTGRES_USER/POSTGRES_PASSWORD/POSTGRES_DB".into())
                })?;
                let password =
                    var("POSTGRES_PASSWORD").ok_or_else(|| ConfigError("POSTGRES_PASSWORD is not set".into()))?;
                let db = var("POSTGRES_DB").ok_or_else(|| ConfigError("POSTGRES_DB is not set".into()))?;
                PgConnectOptions::new()
                    .host(&host)
                    .port(port)
                    .username(&user)
                    .password(&password)
                    .database(&db)
            }
        };

        let discover = match secret("GITHUB_ACCESS_TOKEN")? {
            None => None,
            Some(token) => {
                let interval_secs: u64 = parse("DISCOVER_SYNC_INTERVAL_SECS", 3600)?;
                Some(DiscoverConfig {
                    graphql_url: var("GRAPHQL_URL").unwrap_or_else(|| "https://api.github.com/graphql".into()),
                    token,
                    interval: (interval_secs > 0).then(|| Duration::from_secs(interval_secs.max(60))),
                    batch: parse("DISCOVER_SYNC_BATCH", 100)?,
                })
            }
        };

        let log_format = match var("LOG_FORMAT").as_deref() {
            None | Some("text") => LogFormat::Text,
            Some("json") => LogFormat::Json,
            Some(other) => {
                return Err(ConfigError(format!(
                    "LOG_FORMAT must be 'text' or 'json', got {other:?}"
                )))
            }
        };

        let sync_max_pages: u32 = parse("ISSUE_SYNC_MAX_PAGES", 10)?;
        if !(1..=100).contains(&sync_max_pages) {
            return Err(ConfigError("ISSUE_SYNC_MAX_PAGES must be between 1 and 100".into()));
        }

        Ok(Config {
            bind_address: var("BIND_ADDRESS").unwrap_or_else(|| "0.0.0.0:8080".into()),
            database,
            db_max_connections: parse("DATABASE_MAX_CONNECTIONS", 10)?,
            github_api_url: var("GITHUB_API_URL").unwrap_or_else(|| middleware::github::DEFAULT_API_URL.into()),
            github_timeout: Duration::from_secs(parse("GITHUB_TIMEOUT_SECS", 15)?),
            auth_cache_ttl: Duration::from_secs(parse("AUTH_CACHE_TTL_SECS", 120)?),
            sync_max_pages,
            discover,
            commander_token: secret("COMMANDER_ACCESS_TOKEN")?,
            admin_token: secret("ADMIN_ACCESS_TOKEN")?,
            log_format,
        })
    }
}
