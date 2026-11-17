use actix_web::{web, HttpResponse};

use crate::{
    auth::AuthUser,
    error::ApiError,
    response,
    services::{self, CacheUpdate},
    state::AppState,
    validate::{
        CommentBody, CreateIssueBody, IssueListQuery, IssuePath, LabelNameQuery, LabelsBody, PageQuery, RepoPath,
        UpdateIssueBody,
    },
};

/// Lists issues with filtering, search, sorting and pagination (performed by GitHub).
pub async fn list(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<RepoPath>,
    query: web::Query<IssueListQuery>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    let query = query.into_inner().into_query()?;
    let page = state
        .github
        .list_issues(user.token(), &path.owner, &path.repo, &query)
        .await?;
    Ok(response::from_page(page))
}

pub async fn create(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<RepoPath>,
    body: web::Json<CreateIssueBody>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    let new_issue = body.into_inner().into_new_issue()?;
    let issue = state
        .github
        .create_issue(user.token(), &path.owner, &path.repo, &new_issue)
        .await?;
    tracing::info!(user_id = user.id(), repo = %path.full_name(), number = issue.number, "issue created");
    services::update_cache(&state, &user, &path, CacheUpdate::Issue(&issue)).await;
    Ok(response::created(issue))
}

pub async fn get(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<IssuePath>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    let issue = state
        .github
        .get_issue(user.token(), &path.owner, &path.repo, path.number)
        .await?;
    Ok(response::ok(issue))
}

/// Edits title/body, closes/reopens (with a reason), and replaces assignees or labels.
pub async fn update(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<IssuePath>,
    body: web::Json<UpdateIssueBody>,
) -> Result<HttpResponse, ApiError> {
    let repo = path.validate()?;
    let patch = body.into_inner().into_patch()?;
    let issue = state
        .github
        .update_issue(user.token(), &path.owner, &path.repo, path.number, &patch)
        .await?;
    if issue.is_pull_request() {
        return Err(ApiError::NotFound(format!(
            "#{} is a pull request, not an issue",
            path.number
        )));
    }
    tracing::info!(user_id = user.id(), repo = %repo.full_name(), number = issue.number, "issue updated");
    services::update_cache(&state, &user, &repo, CacheUpdate::Issue(&issue)).await;
    Ok(response::ok(issue))
}

pub async fn comments(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<IssuePath>,
    query: web::Query<PageQuery>,
) -> Result<HttpResponse, ApiError> {
    path.validate()?;
    let (page, per_page) = query.resolve()?;
    let comments = state
        .github
        .list_comments(user.token(), &path.owner, &path.repo, path.number, page, per_page)
        .await?;
    Ok(response::from_page(comments))
}

pub async fn add_comment(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<IssuePath>,
    body: web::Json<CommentBody>,
) -> Result<HttpResponse, ApiError> {
    let repo = path.validate()?;
    body.validate()?;
    let comment = state
        .github
        .create_comment(user.token(), &path.owner, &path.repo, path.number, &body.body)
        .await?;
    let number = path.number as i32;
    services::update_cache(&state, &user, &repo, CacheUpdate::CommentAdded { number }).await;
    Ok(response::created(comment))
}

pub async fn add_labels(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<IssuePath>,
    body: web::Json<LabelsBody>,
) -> Result<HttpResponse, ApiError> {
    let repo = path.validate()?;
    let labels = body.into_inner().into_labels()?;
    let result = state
        .github
        .add_labels(user.token(), &path.owner, &path.repo, path.number, &labels)
        .await?;
    let names = result.iter().map(|l| l.name.clone()).collect();
    services::update_cache(
        &state,
        &user,
        &repo,
        CacheUpdate::Labels {
            number: path.number as i32,
            labels: names,
        },
    )
    .await;
    Ok(response::ok(result))
}

/// `DELETE .../labels?name=<label>` (the name is a query parameter so it may contain `/`).
pub async fn remove_label(
    state: web::Data<AppState>,
    user: AuthUser,
    path: web::Path<IssuePath>,
    query: web::Query<LabelNameQuery>,
) -> Result<HttpResponse, ApiError> {
    let repo = path.validate()?;
    query.validate()?;
    let result = state
        .github
        .remove_label(user.token(), &path.owner, &path.repo, path.number, &query.name)
        .await?;
    let names = result.iter().map(|l| l.name.clone()).collect();
    services::update_cache(
        &state,
        &user,
        &repo,
        CacheUpdate::Labels {
            number: path.number as i32,
            labels: names,
        },
    )
    .await;
    Ok(response::ok(result))
}
