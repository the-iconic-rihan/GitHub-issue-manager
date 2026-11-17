use std::collections::HashSet;

use actix_web::{web, HttpResponse};
use middleware::{github::models::Repository, store::tracked};
use serde::Serialize;

use crate::{
    auth::AuthUser,
    error::ApiError,
    response, services,
    state::AppState,
    validate::{PageQuery, RepoPath},
};

#[derive(Serialize)]
struct RepoView {
    #[serde(flatten)]
    repo: Repository,
    tracked: bool,
}

/// Repositories the user can access on GitHub, flagged with whether they are tracked here.
pub async fn list(
    state: web::Data<AppState>,
    user: AuthUser,
    query: web::Query<PageQuery>,
) -> Result<HttpResponse, ApiError> {
    let (page, per_page) = query.resolve()?;
    let repos = state.github.list_user_repos(user.token(), page, per_page).await?;
    let tracked_ids: HashSet<i64> = tracked::list(&state.pool, user.id())
        .await?
        .into_iter()
        .map(|t| t.repo_id)
        .collect();
    let meta = response::PageMeta {
        page: repos.page,
        per_page: repos.per_page,
        has_next: repos.has_next,
        total_count: None,
    };
    let items = repos
        .items
        .into_iter()
        .map(|repo| RepoView {
            tracked: tracked_ids.contains(&repo.id),
            repo,
        })
        .collect();
    Ok(response::paged(items, meta))
}

#[derive(Serialize)]
struct RepoDetail {
    #[serde(flatten)]
    repo: Repository,
    tracked: Option<tracked::TrackedRepository>,
}

pub async fn get(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<RepoPath>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    let repo = state.github.get_repo(user.token(), &path.owner, &path.repo).await?;
    let tracked = tracked::find_by_id(&state.pool, user.id(), repo.id).await?;
    Ok(response::ok(RepoDetail { repo, tracked }))
}

pub async fn labels(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<RepoPath>,
    query: web::Query<PageQuery>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    let (page, per_page) = query.resolve()?;
    let labels = state
        .github
        .list_repo_labels(user.token(), &path.owner, &path.repo, page, per_page)
        .await?;
    Ok(response::from_page(labels))
}

pub async fn assignees(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<RepoPath>,
    query: web::Query<PageQuery>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    let (page, per_page) = query.resolve()?;
    let users = state
        .github
        .list_assignees(user.token(), &path.owner, &path.repo, page, per_page)
        .await?;
    Ok(response::from_page(users))
}

pub async fn tracked(state: web::Data<AppState>, user: AuthUser) -> Result<HttpResponse, ApiError> {
    Ok(response::ok(tracked::list(&state.pool, user.id()).await?))
}

pub async fn track(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<RepoPath>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    Ok(response::ok(services::track(&state, &user, &path).await?))
}

pub async fn untrack(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<RepoPath>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    services::untrack(&state, &user, &path).await?;
    Ok(response::no_content())
}

pub async fn sync(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<RepoPath>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    Ok(response::ok(services::sync(&state, &user, &path).await?))
}
