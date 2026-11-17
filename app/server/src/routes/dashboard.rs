use actix_web::{web, HttpResponse};
use middleware::store::repo_issues;

use crate::{auth::AuthUser, error::ApiError, response, state::AppState};

/// Statistics over the user's tracked repositories, computed from the synced snapshot.
pub async fn dashboard(state: web::Data<AppState>, user: AuthUser) -> Result<HttpResponse, ApiError> {
    let dashboard = repo_issues::dashboard(&state.pool, user.id(), user.login()).await?;
    Ok(response::ok(dashboard))
}
