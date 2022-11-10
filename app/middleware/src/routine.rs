use log::debug;
use reqwest::RequestBuilder;
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::{
    error::{Error, Result},
    issue::Issue,
    search::IssueEasyPage,
    status::IssueStatusPage,
};

pub struct Routine {
    pool: Option<PgPool>,
    request_builder: Option<RequestBuilder>,
}

// Custome clone impl that uses `try_clone` method on `RequestBuilder`.
// `try_clone` becuase streams cannot be cloned.
impl Clone for Routine {
    fn clone(&self) -> Self {
        let pool = self.pool.clone();
        let request_builder = self.request_builder.as_ref().and_then(|r| r.try_clone());

        Self {
            pool,
            request_builder,
        }
    }
}

impl Routine {
    pub async fn new() -> Result<Self> {
        let postgresdb_service_host = std::env::var("POSTGRESDB_SERVICE_HOST")?;
        let postgresdb_service_port = std::env::var("POSTGRESDB_SERVICE_PORT")?;
        let postgres_db = std::env::var("POSTGRES_DB")?;
        let postgres_user = std::env::var("POSTGRES_USER")?;
        let postgres_password = std::env::var("POSTGRES_PASSWORD")?;

        let database_url = format!(
            "postgres://{}:{}@{}:{}/{}",
            postgres_user,
            postgres_password,
            postgresdb_service_host,
            postgresdb_service_port,
            postgres_db
        );

        debug!("database_url: {}", &database_url);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        let pool = Some(pool);

        let graphql_url = std::env::var("GRAPHQL_URL").unwrap();

        debug!("graphql_url: {}", &graphql_url);

        let github_access_token = std::env::var("GITHUB_ACCESS_TOKEN").unwrap();

        debug!("github_access_token: {}", &github_access_token);

        let client = reqwest::Client::builder()
            .user_agent("GitFirst (rv:1) GraphQL client")
            .build()
            .map_err(Error::Reqwest)?;

        let request_builder = Some(client.post(&graphql_url).bearer_auth(&github_access_token));

        Ok(Self {
            pool,
            request_builder,
        })
    }

    pub async fn retrieve(&self, id: &String) -> Result<Issue> {
        let pool = &self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Missing("Database pool is missing".to_owned()))?;

        Issue::retrieve(pool, id).await.map_err(|e| e.into())
    }

    pub async fn remove(&self, id: &String) -> Result<Issue> {
        let pool = &self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Missing("Database pool is missing".to_owned()))?;

        Issue::retrieve(pool, id).await.map_err(|e| e.into())
    }

    pub async fn insert(&self, issue: &Issue) -> Result<String> {
        let pool = &self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Missing("Database pool is missing".to_owned()))?;

        Issue::insert(pool, issue).await.map_err(|e| e.into())
    }

    pub async fn retrieve_all(&self) -> Result<Vec<Issue>> {
        let pool = &self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Missing("Database pool is missing".to_owned()))?;

        Issue::retrieve_all(pool).await.map_err(|e| e.into())
    }

    pub async fn retrieve_some(&self, limit: i64) -> Result<Vec<Issue>> {
        let pool = &self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Missing("Database pool is missing".to_owned()))?;

        Issue::retrieve_some(pool, limit)
            .await
            .map_err(|e| e.into())
    }

    pub async fn insert_issues(&self, issues: u16) -> Result<()> {
        let pool = &self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Missing("Database pool is missing".to_owned()))?;

        let request_builder = &self
            .request_builder
            .as_ref()
            .ok_or_else(|| Error::Missing("Request builder is missing".to_owned()))?;

        IssueEasyPage::new(pool, request_builder, issues).await
    }

    pub async fn update_issues(&self, issues: u16) -> Result<()> {
        let pool = &self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Missing("Database pool is missing".to_owned()))?;

        let request_builder = &self
            .request_builder
            .as_ref()
            .ok_or_else(|| Error::Missing("Request builder is missing".to_owned()))?;

        IssueStatusPage::update(pool, request_builder, issues).await
    }
}
