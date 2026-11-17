use actix_web::{web, HttpResponse};
use middleware::{github::models::User, store::users};
use serde::Serialize;

use crate::{auth::AuthUser, error::ApiError, response, state::AppState};

#[derive(Serialize)]
struct Me<'a> {
    user: &'a User,
    discover_enabled: bool,
}

/// The signed-in user. Also records/refreshes the user's profile ("connects" the account).
pub async fn me(state: web::Data<AppState>, user: AuthUser) -> Result<HttpResponse, ApiError> {
    users::upsert(&state.pool, &user.user).await?;
    Ok(response::ok(Me {
        user: &user.user,
        discover_enabled: state.discover.is_some(),
    }))
}

/// Disconnects the account: deletes everything stored for the user (tracked repositories and
/// cached issues cascade). Revoking the OAuth grant on GitHub is done by the web app, which
/// holds the OAuth client secret.
pub async fn delete_me(state: web::Data<AppState>, user: AuthUser) -> Result<HttpResponse, ApiError> {
    users::delete(&state.pool, user.id()).await?;
    state.auth_cache.invalidate(user.token());
    tracing::info!(user_id = user.id(), "user data deleted");
    Ok(response::no_content())
}
