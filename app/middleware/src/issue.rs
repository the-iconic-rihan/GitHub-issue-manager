use serde::Serialize;
use sqlx::PgPool;

use crate::graphql::DateTime;

#[derive(Debug, Serialize)]
pub struct Issue {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) created_at: DateTime,
    pub(crate) url: String,
    pub(crate) labels: Vec<String>,
    pub(crate) repository_name: String,
    pub(crate) fork_count: i64,
    pub(crate) star_count: i64,
    pub(crate) primary_language: String,
}

impl Issue {
    pub(crate) async fn insert(pool: &PgPool, issue: &Issue) -> sqlx::Result<String> {
        let record = sqlx::query!(
            r#"
INSERT INTO issues (
id,
title,
created_at,
url,
labels,
repository_name,
fork_count,
star_count,
primary_language
)
VALUES (
$1,
$2,
$3,
$4,
$5,
$6,
$7,
$8,
$9
)
ON CONFLICT (id)
DO UPDATE
SET (
title,
url,
labels,
repository_name,
fork_count,
star_count,
primary_language
) = (
EXCLUDED.title,
EXCLUDED.url,
EXCLUDED.labels,
EXCLUDED.repository_name,
EXCLUDED.fork_count,
EXCLUDED.star_count,
EXCLUDED.primary_language
)
RETURNING id
        "#,
            &issue.id,
            &issue.title,
            &issue.created_at,
            &issue.url,
            &issue.labels,
            &issue.repository_name,
            &issue.fork_count,
            &issue.star_count,
            &issue.primary_language,
        )
        .fetch_one(pool)
        .await?;

        Ok(record.id)
    }

    pub(crate) async fn retrieve_all(pool: &PgPool) -> sqlx::Result<Vec<Issue>> {
        let records = sqlx::query!("SELECT * FROM issues ORDER BY updated_at DESC",)
            .fetch_all(pool)
            .await?;

        Ok(records
            .into_iter()
            .map(|record| Issue {
                id: record.id,
                title: record.title,
                created_at: record.created_at,
                url: record.url,
                labels: record.labels,
                repository_name: record.repository_name,
                fork_count: record.fork_count,
                star_count: record.star_count,
                primary_language: record.primary_language,
            })
            .collect())
    }

    pub(crate) async fn retrieve_some(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<Issue>> {
        let records = sqlx::query!(
            "SELECT * FROM issues ORDER BY updated_at DESC LIMIT $1",
            &limit
        )
        .fetch_all(pool)
        .await?;

        Ok(records
            .into_iter()
            .map(|record| Issue {
                id: record.id,
                title: record.title,
                created_at: record.created_at,
                url: record.url,
                labels: record.labels,
                repository_name: record.repository_name,
                fork_count: record.fork_count,
                star_count: record.star_count,
                primary_language: record.primary_language,
            })
            .collect())
    }

    pub(crate) async fn retrieve_latest_id(pool: &PgPool) -> sqlx::Result<String> {
        let record = sqlx::query!("SELECT id FROM issues ORDER BY updated_at ASC LIMIT 1",)
            .fetch_one(pool)
            .await?;

        Ok(record.id)
    }

    pub(crate) async fn retrieve(pool: &PgPool, id: &String) -> sqlx::Result<Issue> {
        let record = sqlx::query!("SELECT * FROM issues WHERE id = $1", &id)
            .fetch_one(pool)
            .await?;

        Ok(Issue {
            id: record.id,
            title: record.title,
            created_at: record.created_at,
            url: record.url,
            labels: record.labels,
            repository_name: record.repository_name,
            fork_count: record.fork_count,
            star_count: record.star_count,
            primary_language: record.primary_language,
        })
    }

    pub(crate) async fn retrieve_ids_asc(pool: &PgPool, limit: &i64) -> sqlx::Result<Vec<String>> {
        let records = sqlx::query!(
            "SELECT id FROM issues ORDER BY updated_at ASC LIMIT $1",
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(records.into_iter().map(|record| record.id).collect())
    }

    pub(crate) async fn update_timestamp(pool: &PgPool, id: &String) -> sqlx::Result<String> {
        let now = chrono::offset::Utc::now();

        let records = sqlx::query!(
            "UPDATE issues SET updated_at = $1 WHERE id = $2 RETURNING id",
            &now,
            &id
        )
        .fetch_all(pool)
        .await?;

        let ids: Vec<String> = records.into_iter().map(|r| r.id).collect();

        // If this isn't true, there are more than one rows
        // with similar `id`, primry key
        assert!(ids.len() == 1);

        Ok(ids
            .get(0)
            .expect("More than one row with same `id`")
            .to_owned())
    }

    pub(crate) async fn remove(pool: &PgPool, id: &String) -> sqlx::Result<String> {
        let record = sqlx::query!(
            r#"
DELETE FROM issues
WHERE id = $1
RETURNING id
        "#,
            &id,
        )
        .fetch_one(pool)
        .await?;

        Ok(record.id)
    }
}
