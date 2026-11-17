use actix_web::{web, HttpResponse};
use serde_json::json;

use crate::{error::ApiError, response, state::AppState};

/// Liveness: the process is serving requests.
pub async fn live() -> HttpResponse {
    response::ok(json!({ "status": "ok" }))
}

/// Readiness: the database is reachable.
pub async fn ready(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    middleware::db::ping(&state.pool).await.map_err(|e| {
        tracing::warn!(error = %e, "readiness check failed");
        ApiError::Unavailable("Database is not reachable.".into())
    })?;
    Ok(response::ok(json!({ "status": "ok", "database": "ok" })))
}
