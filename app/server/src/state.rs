use std::sync::Arc;

use middleware::{github::GitHubClient, routine::Routine};
use sqlx::PgPool;

use crate::auth::AuthCache;

/// Shared application state (wrapped in `web::Data`, i.e. an `Arc`).
pub struct AppState {
    pub pool: PgPool,
    pub github: GitHubClient,
    pub auth_cache: AuthCache,
    pub sync_max_pages: u32,
    pub discover: Option<Discover>,
    pub commander_token: Option<String>,
    pub admin_token: Option<String>,
}

/// The Discover aggregator plus a lock so the background job and manual commands never
/// run concurrently (they would double-spend the rate limit and race on the same rows).
pub struct Discover {
    pub routine: Routine,
    pub lock: Arc<tokio::sync::Mutex<()>>,
}

impl Discover {
    pub fn new(routine: Routine) -> Self {
        Self {
            routine,
            lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}
