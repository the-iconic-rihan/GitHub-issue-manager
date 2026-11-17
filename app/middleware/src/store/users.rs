use sqlx::PgExecutor;

use crate::github::models::User;

/// Records (or refreshes) a GitHub user. Logins can change, ids cannot.
pub async fn upsert<'e>(db: impl PgExecutor<'e>, user: &User) -> sqlx::Result<()> {
    sqlx::query(
        r#"
INSERT INTO users (id, login, name, avatar_url)
VALUES ($1, $2, $3, $4)
ON CONFLICT (id) DO UPDATE SET
    login = EXCLUDED.login,
    name = EXCLUDED.name,
    avatar_url = EXCLUDED.avatar_url,
    last_seen_at = now()
"#,
    )
    .bind(user.id)
    .bind(&user.login)
    .bind(&user.name)
    .bind(&user.avatar_url)
    .execute(db)
    .await
    .map(|_| ())
}

/// Deletes a user and (via `ON DELETE CASCADE`) everything stored for them.
pub async fn delete<'e>(db: impl PgExecutor<'e>, user_id: i64) -> sqlx::Result<bool> {
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(db)
        .await
        .map(|r| r.rows_affected() > 0)
}
