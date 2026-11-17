use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgExecutor, PgPool};

/// A repository the user has chosen to manage in the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct TrackedRepository {
    pub repo_id: i64,
    pub full_name: String,
    pub private: bool,
    pub created_at: DateTime<Utc>,
    pub last_synced_at: Option<DateTime<Utc>>,
}

const COLUMNS: &str = "repo_id, full_name, private, created_at, last_synced_at";

pub async fn list(pool: &PgPool, user_id: i64) -> sqlx::Result<Vec<TrackedRepository>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM tracked_repositories WHERE user_id = $1 ORDER BY lower(full_name)"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn find_by_id<'e>(
    db: impl PgExecutor<'e>,
    user_id: i64,
    repo_id: i64,
) -> sqlx::Result<Option<TrackedRepository>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM tracked_repositories WHERE user_id = $1 AND repo_id = $2"
    ))
    .bind(user_id)
    .bind(repo_id)
    .fetch_optional(db)
    .await
}

pub async fn find_by_name(pool: &PgPool, user_id: i64, full_name: &str) -> sqlx::Result<Option<TrackedRepository>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM tracked_repositories WHERE user_id = $1 AND lower(full_name) = lower($2)"
    ))
    .bind(user_id)
    .bind(full_name)
    .fetch_optional(pool)
    .await
}

/// Tracks a repository (idempotent). If a *different* repository previously used the same
/// name (it was renamed or deleted and the name reused), that stale entry is replaced.
pub async fn track(
    pool: &PgPool,
    user_id: i64,
    repo_id: i64,
    full_name: &str,
    private: bool,
) -> sqlx::Result<TrackedRepository> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "DELETE FROM tracked_repositories WHERE user_id = $1 AND lower(full_name) = lower($2) AND repo_id <> $3",
    )
    .bind(user_id)
    .bind(full_name)
    .bind(repo_id)
    .execute(&mut *tx)
    .await?;

    let tracked = sqlx::query_as(&format!(
        r#"
INSERT INTO tracked_repositories (user_id, repo_id, full_name, private)
VALUES ($1, $2, $3, $4)
ON CONFLICT (user_id, repo_id) DO UPDATE SET full_name = EXCLUDED.full_name, private = EXCLUDED.private
RETURNING {COLUMNS}
"#
    ))
    .bind(user_id)
    .bind(repo_id)
    .bind(full_name)
    .bind(private)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(tracked)
}

/// Stops tracking; cached issues are removed by `ON DELETE CASCADE`.
pub async fn untrack(pool: &PgPool, user_id: i64, repo_id: i64) -> sqlx::Result<bool> {
    sqlx::query("DELETE FROM tracked_repositories WHERE user_id = $1 AND repo_id = $2")
        .bind(user_id)
        .bind(repo_id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}

/// Records a successful sync. `etag` is the first-page ETag of a *complete* snapshot
/// (`None` disables conditional requests for the next sync).
pub async fn mark_synced<'e>(
    db: impl PgExecutor<'e>,
    user_id: i64,
    repo_id: i64,
    full_name: &str,
    etag: Option<&str>,
) -> sqlx::Result<Option<TrackedRepository>> {
    sqlx::query_as(&format!(
        "UPDATE tracked_repositories SET last_synced_at = now(), full_name = $3, issues_etag = $4 \
         WHERE user_id = $1 AND repo_id = $2 RETURNING {COLUMNS}"
    ))
    .bind(user_id)
    .bind(repo_id)
    .bind(full_name)
    .bind(etag)
    .fetch_optional(db)
    .await
}

/// ETag stored by the last complete sync, if any.
pub async fn issues_etag(pool: &PgPool, user_id: i64, repo_id: i64) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT issues_etag FROM tracked_repositories WHERE user_id = $1 AND repo_id = $2")
        .bind(user_id)
        .bind(repo_id)
        .fetch_optional(pool)
        .await
        .map(Option::flatten)
}

/// Tracked repositories never synced or last synced more than `older_than_secs` ago,
/// least recently synced first.
pub async fn stale(
    pool: &PgPool,
    user_id: i64,
    older_than_secs: i64,
    limit: i64,
) -> sqlx::Result<Vec<TrackedRepository>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM tracked_repositories \
         WHERE user_id = $1 AND (last_synced_at IS NULL OR last_synced_at < now() - make_interval(secs => $2)) \
         ORDER BY last_synced_at ASC NULLS FIRST, repo_id ASC LIMIT $3"
    ))
    .bind(user_id)
    .bind(older_than_secs as f64)
    .bind(limit)
    .fetch_all(pool)
    .await
}
