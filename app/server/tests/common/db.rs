//! Isolated, migrated database per test: each test gets its own Postgres schema inside the
//! database named by `DATABASE_URL`, so tests never see each other's (or anyone's) data.

use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct TestDb {
    pub options: PgConnectOptions,
    pub schema: String,
}

impl TestDb {
    pub async fn new() -> Self {
        let url = std::env::var("DATABASE_URL").expect(
            "database tests need DATABASE_URL (e.g. postgres://postgres:postgres@localhost:5432/postgres); \
             see README > Running tests",
        );
        let base: PgConnectOptions = url.parse().expect("DATABASE_URL is not a valid Postgres URL");
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let schema = format!(
            "gim_test_{}_{}_{}",
            std::process::id(),
            nanos % 1_000_000_000,
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );

        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(base.clone())
            .await
            .expect("could not connect to DATABASE_URL");
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .expect("create schema");
        admin.close().await;

        let options = base.options([("search_path", schema.as_str())]);
        let db = TestDb { options, schema };
        let pool = db.pool().await;
        middleware::db::migrate(&pool).await.expect("migrations");
        pool.close().await;
        db
    }

    /// A fresh pool (calling this twice simulates an application restart).
    pub async fn pool(&self) -> PgPool {
        PgPoolOptions::new()
            .max_connections(5)
            .connect_with(self.options.clone())
            .await
            .expect("connect to test schema")
    }

    pub async fn drop(self) {
        let pool = self.pool().await;
        let _ = sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&pool)
            .await;
        pool.close().await;
    }
}
