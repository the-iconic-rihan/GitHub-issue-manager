//! Cached snapshot of tracked repositories' issues, and dashboard statistics over it.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgExecutor, PgPool};

use super::tracked::{self, TrackedRepository};
use crate::github::models::Issue;

/// Upserts one issue. Never overwrites a row with older data (`updated_at` guard), which
/// keeps concurrent sync and write-through updates consistent. Does nothing if the
/// repository is not tracked by the user.
async fn upsert<'e>(db: impl PgExecutor<'e>, user_id: i64, repo_id: i64, issue: &Issue) -> sqlx::Result<u64> {
    let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
    let assignees: Vec<&str> = issue.assignees.iter().map(|a| a.login.as_str()).collect();
    let number = i32::try_from(issue.number).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
    let comments = i32::try_from(issue.comments.max(0)).unwrap_or(i32::MAX);

    sqlx::query(
        r#"
INSERT INTO repo_issues (user_id, repo_id, number, github_id, title, state, author_login,
                         assignees, labels, comments, html_url, created_at, updated_at, closed_at)
SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
WHERE EXISTS (SELECT 1 FROM tracked_repositories WHERE user_id = $1 AND repo_id = $2)
ON CONFLICT (user_id, repo_id, number) DO UPDATE SET
    github_id = EXCLUDED.github_id,
    title = EXCLUDED.title,
    state = EXCLUDED.state,
    author_login = EXCLUDED.author_login,
    assignees = EXCLUDED.assignees,
    labels = EXCLUDED.labels,
    comments = EXCLUDED.comments,
    html_url = EXCLUDED.html_url,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at,
    closed_at = EXCLUDED.closed_at
WHERE repo_issues.updated_at <= EXCLUDED.updated_at
"#,
    )
    .bind(user_id)
    .bind(repo_id)
    .bind(number)
    .bind(issue.id)
    .bind(&issue.title)
    .bind(if issue.state == "closed" { "closed" } else { "open" })
    .bind(issue.user.as_ref().map(|u| u.login.as_str()))
    .bind(&assignees)
    .bind(&labels)
    .bind(comments)
    .bind(&issue.html_url)
    .bind(issue.created_at)
    .bind(issue.updated_at)
    .bind(issue.closed_at)
    .execute(db)
    .await
    .map(|r| r.rows_affected())
}

/// Write-through after the user changes an issue in the app. Returns whether a row changed.
pub async fn record(pool: &PgPool, user_id: i64, repo_id: i64, issue: &Issue) -> sqlx::Result<bool> {
    upsert(pool, user_id, repo_id, issue).await.map(|n| n > 0)
}

/// Write-through for label changes (GitHub's label endpoints return labels, not the issue).
pub async fn set_labels(
    pool: &PgPool,
    user_id: i64,
    repo_id: i64,
    number: i32,
    labels: &[String],
) -> sqlx::Result<bool> {
    sqlx::query("UPDATE repo_issues SET labels = $4 WHERE user_id = $1 AND repo_id = $2 AND number = $3")
        .bind(user_id)
        .bind(repo_id)
        .bind(number)
        .bind(labels)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}

/// Write-through after a comment is added.
pub async fn increment_comments(pool: &PgPool, user_id: i64, repo_id: i64, number: i32) -> sqlx::Result<bool> {
    sqlx::query("UPDATE repo_issues SET comments = comments + 1 WHERE user_id = $1 AND repo_id = $2 AND number = $3")
        .bind(user_id)
        .bind(repo_id)
        .bind(number)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}

/// Replaces the snapshot of a tracked repository's issues in one transaction.
///
/// When `complete` is false (the fetch was truncated) issues missing from `issues` are kept,
/// because their absence does not mean they were deleted. Returns `None` if the repository
/// stopped being tracked meanwhile.
pub async fn sync(
    pool: &PgPool,
    user_id: i64,
    repo_id: i64,
    full_name: &str,
    issues: &[Issue],
    complete: bool,
    etag: Option<&str>,
) -> sqlx::Result<Option<TrackedRepository>> {
    let mut tx = pool.begin().await?;

    // Serialises concurrent syncs of the same repository for this user.
    let locked: Option<i64> =
        sqlx::query_scalar("SELECT repo_id FROM tracked_repositories WHERE user_id = $1 AND repo_id = $2 FOR UPDATE")
            .bind(user_id)
            .bind(repo_id)
            .fetch_optional(&mut *tx)
            .await?;
    if locked.is_none() {
        return Ok(None);
    }

    for issue in issues {
        upsert(&mut *tx, user_id, repo_id, issue).await?;
    }

    if complete {
        let numbers: Vec<i32> = issues.iter().filter_map(|i| i32::try_from(i.number).ok()).collect();
        sqlx::query("DELETE FROM repo_issues WHERE user_id = $1 AND repo_id = $2 AND NOT (number = ANY($3))")
            .bind(user_id)
            .bind(repo_id)
            .bind(&numbers)
            .execute(&mut *tx)
            .await?;
    }

    // Only a complete snapshot may be used as the base for conditional syncs.
    let etag = if complete { etag } else { None };
    let tracked = tracked::mark_synced(&mut *tx, user_id, repo_id, full_name, etag).await?;
    tx.commit().await?;
    Ok(tracked)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct RepoStats {
    pub repo_id: i64,
    pub full_name: String,
    pub private: bool,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub open_issues: i64,
    pub closed_issues: i64,
    pub unassigned_open: i64,
    pub assigned_to_me_open: i64,
    pub opened_last_30_days: i64,
    pub closed_last_30_days: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct RecentIssue {
    pub repo_full_name: String,
    pub number: i32,
    pub title: String,
    pub state: String,
    pub html_url: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Totals {
    pub tracked_repositories: usize,
    pub open_issues: i64,
    pub closed_issues: i64,
    pub unassigned_open: i64,
    pub assigned_to_me_open: i64,
    pub opened_last_30_days: i64,
    pub closed_last_30_days: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dashboard {
    pub totals: Totals,
    pub repositories: Vec<RepoStats>,
    pub top_labels: Vec<LabelCount>,
    pub recently_updated: Vec<RecentIssue>,
}

/// Statistics over the user's tracked repositories. `login` identifies "assigned to me".
pub async fn dashboard(pool: &PgPool, user_id: i64, login: &str) -> sqlx::Result<Dashboard> {
    let repositories: Vec<RepoStats> = sqlx::query_as(
        r#"
SELECT t.repo_id, t.full_name, t.private, t.last_synced_at,
       count(i.number) FILTER (WHERE i.state = 'open')                                  AS open_issues,
       count(i.number) FILTER (WHERE i.state = 'closed')                                AS closed_issues,
       count(i.number) FILTER (WHERE i.state = 'open' AND cardinality(i.assignees) = 0) AS unassigned_open,
       count(i.number) FILTER (WHERE i.state = 'open' AND lower($2) = ANY (SELECT lower(a) FROM unnest(i.assignees) a))
                                                                                        AS assigned_to_me_open,
       count(i.number) FILTER (WHERE i.created_at > now() - interval '30 days')         AS opened_last_30_days,
       count(i.number) FILTER (WHERE i.closed_at > now() - interval '30 days')          AS closed_last_30_days
FROM tracked_repositories t
LEFT JOIN repo_issues i ON i.user_id = t.user_id AND i.repo_id = t.repo_id
WHERE t.user_id = $1
GROUP BY t.repo_id, t.full_name, t.private, t.last_synced_at
ORDER BY lower(t.full_name)
"#,
    )
    .bind(user_id)
    .bind(login)
    .fetch_all(pool)
    .await?;

    let top_labels = sqlx::query_as(
        r#"
SELECT label, count(*) AS count
FROM repo_issues, unnest(labels) AS label
WHERE user_id = $1 AND state = 'open'
GROUP BY label
ORDER BY count DESC, label ASC
LIMIT 10
"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let recently_updated = sqlx::query_as(
        r#"
SELECT t.full_name AS repo_full_name, i.number, i.title, i.state, i.html_url, i.labels, i.assignees, i.updated_at
FROM repo_issues i
JOIN tracked_repositories t ON t.user_id = i.user_id AND t.repo_id = i.repo_id
WHERE i.user_id = $1
ORDER BY i.updated_at DESC
LIMIT 10
"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let totals = repositories.iter().fold(
        Totals {
            tracked_repositories: repositories.len(),
            ..Default::default()
        },
        |mut t, r| {
            t.open_issues += r.open_issues;
            t.closed_issues += r.closed_issues;
            t.unassigned_open += r.unassigned_open;
            t.assigned_to_me_open += r.assigned_to_me_open;
            t.opened_last_30_days += r.opened_last_30_days;
            t.closed_last_30_days += r.closed_last_30_days;
            t
        },
    );

    Ok(Dashboard {
        totals,
        repositories,
        top_labels,
        recently_updated,
    })
}
