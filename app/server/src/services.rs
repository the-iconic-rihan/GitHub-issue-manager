//! Business logic shared by the HTTP handlers.

use chrono::{DateTime, Utc};
use middleware::{
    github::models::{Issue, Repository},
    store::{repo_issues, tracked, tracked::TrackedRepository, users},
};
use serde::Serialize;

use crate::{auth::AuthUser, error::ApiError, state::AppState, validate::RepoPath};

/// Looks up a tracked repository by `owner/repo` for this user.
pub async fn tracked_by_name(
    state: &AppState,
    user: &AuthUser,
    repo: &RepoPath,
) -> Result<Option<TrackedRepository>, ApiError> {
    Ok(tracked::find_by_name(&state.pool, user.id(), &repo.full_name()).await?)
}

/// Starts tracking a repository after confirming with GitHub that the user can see it and
/// that it has issues enabled.
pub async fn track(state: &AppState, user: &AuthUser, path: &RepoPath) -> Result<TrackedRepository, ApiError> {
    let repo = state.github.get_repo(user.token(), &path.owner, &path.repo).await?;
    if !repo.has_issues {
        return Err(ApiError::Conflict(format!(
            "Issues are disabled for {}.",
            repo.full_name
        )));
    }
    // Ensure the user row exists for the foreign key, and refresh their profile.
    users::upsert(&state.pool, &user.user).await?;
    Ok(tracked::track(&state.pool, user.id(), repo.id, &repo.full_name, repo.private).await?)
}

pub async fn untrack(state: &AppState, user: &AuthUser, path: &RepoPath) -> Result<(), ApiError> {
    let found = tracked_by_name(state, user, path)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("{} is not tracked.", path.full_name())))?;
    tracked::untrack(&state.pool, user.id(), found.repo_id).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub repository: TrackedRepository,
    pub issues_fetched: usize,
    /// True when the repository has more issues than `ISSUE_SYNC_MAX_PAGES` allows; the
    /// snapshot then contains the most recently updated ones.
    pub truncated: bool,
    pub synced_at: DateTime<Utc>,
}

/// Refreshes the cached snapshot of a tracked repository's issues from GitHub.
pub async fn sync(state: &AppState, user: &AuthUser, path: &RepoPath) -> Result<SyncResult, ApiError> {
    let tracked = tracked_by_name(state, user, path)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("{} is not tracked. Track it before syncing.", path.full_name())))?;

    let repo: Repository = state.github.get_repo(user.token(), &path.owner, &path.repo).await?;
    if repo.id != tracked.repo_id {
        return Err(ApiError::Conflict(format!(
            "{} now refers to a different repository. Untrack it and track it again.",
            path.full_name()
        )));
    }

    let (issues, truncated) = state
        .github
        .list_all_issues(user.token(), &repo.owner.login, &repo.name, state.sync_max_pages)
        .await?;

    let repository = repo_issues::sync(&state.pool, user.id(), repo.id, &repo.full_name, &issues, !truncated)
        .await?
        .ok_or_else(|| ApiError::Conflict(format!("{} was untracked during the sync.", path.full_name())))?;

    tracing::info!(
        user_id = user.id(),
        repo = %repo.full_name,
        issues = issues.len(),
        truncated,
        "repository synced"
    );

    Ok(SyncResult {
        synced_at: repository.last_synced_at.unwrap_or_else(Utc::now),
        repository,
        issues_fetched: issues.len(),
        truncated,
    })
}

/// Cache maintenance after a successful GitHub write. GitHub is the source of truth and the
/// write already succeeded, so a cache failure is logged (not returned) and corrected by the
/// next sync.
pub enum CacheUpdate<'a> {
    Issue(&'a Issue),
    Labels { number: i32, labels: Vec<String> },
    CommentAdded { number: i32 },
}

pub async fn update_cache(state: &AppState, user: &AuthUser, path: &RepoPath, update: CacheUpdate<'_>) {
    let result = async {
        let Some(t) = tracked_by_name(state, user, path).await? else {
            return Ok::<_, ApiError>(());
        };
        match update {
            CacheUpdate::Issue(issue) => repo_issues::record(&state.pool, user.id(), t.repo_id, issue).await?,
            CacheUpdate::Labels { number, labels } => {
                repo_issues::set_labels(&state.pool, user.id(), t.repo_id, number, &labels).await?
            }
            CacheUpdate::CommentAdded { number } => {
                repo_issues::increment_comments(&state.pool, user.id(), t.repo_id, number).await?
            }
        };
        Ok(())
    }
    .await;

    if let Err(e) = result {
        tracing::warn!(
            user_id = user.id(),
            repo = %path.full_name(),
            error = %e,
            "issue cache update failed; it will be corrected by the next sync"
        );
    }
}
