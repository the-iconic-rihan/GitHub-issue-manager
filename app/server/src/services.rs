//! Business logic shared by the HTTP handlers.

use chrono::{DateTime, Utc};
use middleware::{
    github::{
        models::{Issue, Repository},
        IssueSnapshot,
    },
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
    /// True when a conditional sync found nothing changed since the last complete sync.
    pub not_modified: bool,
    pub synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Always download every page (the "Sync" button).
    Full,
    /// Skip the download when GitHub reports the first page unchanged (automatic syncs).
    Conditional,
}

/// Refreshes the cached snapshot of a tracked repository's issues from GitHub.
pub async fn sync(state: &AppState, user: &AuthUser, path: &RepoPath, mode: SyncMode) -> Result<SyncResult, ApiError> {
    let tracked = tracked_by_name(state, user, path)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("{} is not tracked. Track it before syncing.", path.full_name())))?;
    sync_tracked(state, user, &tracked, mode).await
}

async fn sync_tracked(
    state: &AppState,
    user: &AuthUser,
    tracked: &TrackedRepository,
    mode: SyncMode,
) -> Result<SyncResult, ApiError> {
    let (owner, name) = tracked
        .full_name
        .split_once('/')
        .ok_or_else(|| ApiError::Internal(format!("malformed tracked repository name {:?}", tracked.full_name)))?;

    // Confirms access and follows renames (GitHub redirects old names to the same id).
    let repo: Repository = state.github.get_repo(user.token(), owner, name).await?;
    if repo.id != tracked.repo_id {
        return Err(ApiError::Conflict(format!(
            "{} now refers to a different repository. Untrack it and track it again.",
            tracked.full_name
        )));
    }

    let previous_etag = match mode {
        SyncMode::Conditional => tracked::issues_etag(&state.pool, user.id(), repo.id).await?,
        SyncMode::Full => None,
    };
    let untracked = || ApiError::Conflict(format!("{} was untracked during the sync.", tracked.full_name));

    let snapshot = state
        .github
        .issue_snapshot(
            user.token(),
            &repo.owner.login,
            &repo.name,
            state.sync_max_pages,
            previous_etag.as_deref(),
        )
        .await?;

    let (repository, issues_fetched, truncated, not_modified) = match snapshot {
        IssueSnapshot::NotModified => {
            let repository = tracked::mark_synced(
                &state.pool,
                user.id(),
                repo.id,
                &repo.full_name,
                previous_etag.as_deref(),
            )
            .await?
            .ok_or_else(untracked)?;
            (repository, 0, false, true)
        }
        IssueSnapshot::Fetched {
            issues,
            truncated,
            etag,
        } => {
            let repository = repo_issues::sync(
                &state.pool,
                user.id(),
                repo.id,
                &repo.full_name,
                &issues,
                !truncated,
                etag.as_deref(),
            )
            .await?
            .ok_or_else(untracked)?;
            (repository, issues.len(), truncated, false)
        }
    };

    tracing::info!(
        user_id = user.id(),
        repo = %repo.full_name,
        issues = issues_fetched,
        truncated,
        not_modified,
        ?mode,
        "repository synced"
    );

    Ok(SyncResult {
        synced_at: repository.last_synced_at.unwrap_or_else(Utc::now),
        repository,
        issues_fetched,
        truncated,
        not_modified,
    })
}

#[derive(Debug, Serialize)]
pub struct SyncFailure {
    pub full_name: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct StaleSyncReport {
    /// Repositories whose snapshot changed.
    pub updated: Vec<String>,
    /// Repositories GitHub reported unchanged (no rate limit spent on the listing).
    pub unchanged: Vec<String>,
    pub failed: Vec<SyncFailure>,
    /// True if more stale repositories remain (limit reached or stopped by rate limiting).
    pub more_pending: bool,
}

/// Conditionally re-syncs the user's tracked repositories whose snapshot is older than
/// `AUTO_SYNC_STALE_SECS`, at most `AUTO_SYNC_MAX_REPOS` per call, least recently synced
/// first. One repository failing does not stop the others; rate limiting does.
pub async fn sync_stale(state: &AppState, user: &AuthUser) -> Result<StaleSyncReport, ApiError> {
    let limit = i64::from(state.auto_sync.max_repos);
    // Fetch one extra row to know whether more are pending.
    let mut stale = tracked::stale(&state.pool, user.id(), state.auto_sync.stale_after_secs, limit + 1).await?;
    let mut report = StaleSyncReport {
        more_pending: stale.len() as i64 > limit,
        ..Default::default()
    };
    stale.truncate(limit as usize);

    for repo in &stale {
        match sync_tracked(state, user, repo, SyncMode::Conditional).await {
            Ok(result) if result.not_modified => report.unchanged.push(repo.full_name.clone()),
            Ok(_) => report.updated.push(repo.full_name.clone()),
            // The token itself is invalid: nothing else can succeed either.
            Err(e @ ApiError::Unauthorized(_)) => return Err(e),
            Err(e) => {
                let rate_limited = matches!(e, ApiError::RateLimited { .. });
                report.failed.push(SyncFailure {
                    full_name: repo.full_name.clone(),
                    code: e.code(),
                    message: e.public_message(),
                });
                if rate_limited {
                    report.more_pending = true;
                    break;
                }
            }
        }
    }
    Ok(report)
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
