//! Storage for Discover (aggregated beginner-friendly) issues: the `issues` table.

use serde::Serialize;
use sqlx::PgPool;

use crate::graphql::DateTime;

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Issue {
    /// GitHub GraphQL node id.
    pub id: String,
    pub title: String,
    pub created_at: DateTime,
    pub url: String,
    pub labels: Vec<String>,
    /// `owner/name` of the repository.
    pub repository_name: String,
    pub fork_count: i64,
    pub star_count: i64,
    /// Empty string when GitHub could not detect a language.
    pub primary_language: String,
}

/// Filters for listing Discover issues.
#[derive(Debug, Clone, Default)]
pub struct DiscoverFilter {
    pub language: Option<String>,
    pub label: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct LanguageCount {
    pub language: String,
    pub count: i64,
}

const COLUMNS: &str = "id, title, created_at, url, labels, repository_name, fork_count, star_count, primary_language";

impl Issue {
    /// Inserts or refreshes an issue. `updated_at` records when it was last confirmed open.
    pub async fn insert(pool: &PgPool, issue: &Issue) -> sqlx::Result<String> {
        sqlx::query_scalar(
            r#"
INSERT INTO issues (id, title, created_at, url, labels, repository_name, fork_count, star_count, primary_language, updated_at)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())
ON CONFLICT (id) DO UPDATE SET
    title = EXCLUDED.title,
    url = EXCLUDED.url,
    labels = EXCLUDED.labels,
    repository_name = EXCLUDED.repository_name,
    fork_count = EXCLUDED.fork_count,
    star_count = EXCLUDED.star_count,
    primary_language = EXCLUDED.primary_language,
    updated_at = now()
RETURNING id
"#,
        )
        .bind(&issue.id)
        .bind(&issue.title)
        .bind(issue.created_at)
        .bind(&issue.url)
        .bind(&issue.labels)
        .bind(&issue.repository_name)
        .bind(issue.fork_count)
        .bind(issue.star_count)
        .bind(&issue.primary_language)
        .fetch_one(pool)
        .await
    }

    pub async fn retrieve(pool: &PgPool, id: &str) -> sqlx::Result<Option<Issue>> {
        sqlx::query_as(&format!("SELECT {COLUMNS} FROM issues WHERE id = $1"))
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Creation time of the newest stored issue; used to stop paging through search results
    /// once everything newer has been collected.
    pub async fn latest_created_at(pool: &PgPool) -> sqlx::Result<Option<DateTime>> {
        sqlx::query_scalar("SELECT max(created_at) FROM issues")
            .fetch_one(pool)
            .await
    }

    /// Ids of the issues whose state was confirmed least recently.
    pub async fn retrieve_ids_least_recently_checked(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar("SELECT id FROM issues ORDER BY updated_at ASC, id ASC LIMIT $1")
            .bind(limit)
            .fetch_all(pool)
            .await
    }

    /// Marks issues as checked now. Returns the number of rows updated.
    pub async fn touch(pool: &PgPool, ids: &[String]) -> sqlx::Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        sqlx::query("UPDATE issues SET updated_at = now() WHERE id = ANY($1)")
            .bind(ids)
            .execute(pool)
            .await
            .map(|r| r.rows_affected())
    }

    /// Removes issues (closed or deleted on GitHub). Returns the number of rows deleted.
    pub async fn remove(pool: &PgPool, ids: &[String]) -> sqlx::Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        sqlx::query("DELETE FROM issues WHERE id = ANY($1)")
            .bind(ids)
            .execute(pool)
            .await
            .map(|r| r.rows_affected())
    }

    /// Lists issues matching `filter`, newest first, plus the total number of matches.
    pub async fn search(pool: &PgPool, filter: &DiscoverFilter) -> sqlx::Result<(Vec<Issue>, i64)> {
        let where_clause = "($1::text IS NULL OR lower(primary_language) = lower($1)) \
                            AND ($2::text IS NULL OR EXISTS (SELECT 1 FROM unnest(labels) l WHERE lower(l) = lower($2)))";

        let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM issues WHERE {where_clause}"))
            .bind(&filter.language)
            .bind(&filter.label)
            .fetch_one(pool)
            .await?;

        let items = sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM issues WHERE {where_clause} ORDER BY created_at DESC, id ASC LIMIT $3 OFFSET $4"
        ))
        .bind(&filter.language)
        .bind(&filter.label)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(pool)
        .await?;

        Ok((items, total))
    }

    /// Languages present in the Discover table with issue counts, most common first.
    pub async fn languages(pool: &PgPool) -> sqlx::Result<Vec<LanguageCount>> {
        sqlx::query_as(
            "SELECT primary_language AS language, count(*) AS count FROM issues \
             WHERE primary_language <> '' GROUP BY primary_language ORDER BY count DESC, language ASC",
        )
        .fetch_all(pool)
        .await
    }
}
