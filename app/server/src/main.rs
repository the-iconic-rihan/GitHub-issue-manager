use std::{process::ExitCode, time::Duration};

use actix_web::{web, HttpServer};
use middleware::{github::GitHubClient, routine::Routine};
use server::{
    auth::AuthCache,
    build_app,
    config::{Config, LogFormat},
    state::{AppState, Discover},
};
use tracing_subscriber::EnvFilter;

const DB_CONNECT_ATTEMPTS: u32 = 30;

fn init_logging(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter).with_target(true);
    match format {
        LogFormat::Json => builder.json().flatten_event(true).init(),
        LogFormat::Text => builder.init(),
    }
}

/// `server --healthcheck`: exits 0 if the local server answers its liveness endpoint.
/// Used by the Docker HEALTHCHECK, as the runtime image has no curl.
async fn healthcheck() -> ExitCode {
    let bind = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let port = bind.rsplit(':').next().unwrap_or("8080");
    let url = format!("http://127.0.0.1:{port}/api/v1/health/live");
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(3)).build() {
        Ok(c) => c,
        Err(_) => return ExitCode::FAILURE,
    };
    match client.get(url).send().await {
        Ok(r) if r.status().is_success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

async fn connect_with_retry(config: &Config) -> Result<sqlx::PgPool, sqlx::Error> {
    let mut attempt = 1;
    loop {
        match middleware::db::connect(config.database.clone(), config.db_max_connections).await {
            Ok(pool) => return Ok(pool),
            Err(e) if attempt < DB_CONNECT_ATTEMPTS => {
                tracing::warn!(attempt, error = %e, "database not reachable yet; retrying in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Periodically collects new Discover issues and prunes closed ones.
fn spawn_discover_job(lock: std::sync::Arc<tokio::sync::Mutex<()>>, routine: Routine, interval: Duration, batch: u16) {
    tokio::spawn(async move {
        // Let the server finish starting before spending API quota.
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            {
                let _guard = lock.lock().await;
                if let Err(e) = routine.insert_issues(batch).await {
                    tracing::error!(error = %e, "discover: collecting issues failed");
                }
                if let Err(e) = routine.update_issues(batch).await {
                    tracing::error!(error = %e, "discover: refreshing issue states failed");
                }
            }
            tokio::time::sleep(interval).await;
        }
    });
}

#[actix_web::main]
async fn main() -> ExitCode {
    if std::env::args().any(|a| a == "--healthcheck") {
        return healthcheck().await;
    }

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    init_logging(config.log_format);
    tracing::info!(?config, "starting server");

    let pool = match connect_with_retry(&config).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "could not connect to the database");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = middleware::db::migrate(&pool).await {
        tracing::error!(error = %e, "database migration failed");
        return ExitCode::FAILURE;
    }
    tracing::info!("database migrations applied");

    let github = match GitHubClient::new(&config.github_api_url, config.github_timeout) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "invalid GitHub configuration");
            return ExitCode::FAILURE;
        }
    };

    let discover = match &config.discover {
        None => {
            tracing::info!("Discover disabled (GITHUB_ACCESS_TOKEN not set)");
            None
        }
        Some(d) => match Routine::new(pool.clone(), &d.graphql_url, d.token.clone(), config.github_timeout) {
            Ok(routine) => {
                let discover = Discover::new(routine.clone());
                if let Some(interval) = d.interval {
                    tracing::info!(
                        interval_secs = interval.as_secs(),
                        batch = d.batch,
                        "Discover background job enabled"
                    );
                    spawn_discover_job(discover.lock.clone(), routine, interval, d.batch);
                }
                Some(discover)
            }
            Err(e) => {
                tracing::error!(error = %e, "invalid Discover configuration");
                return ExitCode::FAILURE;
            }
        },
    };

    let state = web::Data::new(AppState {
        pool,
        github,
        auth_cache: AuthCache::new(config.auth_cache_ttl),
        sync_max_pages: config.sync_max_pages,
        discover,
        commander_token: config.commander_token.clone(),
        admin_token: config.admin_token.clone(),
    });

    let server = match HttpServer::new(move || build_app(state.clone())).bind(&config.bind_address) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, address = %config.bind_address, "could not bind");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(address = %config.bind_address, "listening");

    match server.run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "server error");
            ExitCode::FAILURE
        }
    }
}
