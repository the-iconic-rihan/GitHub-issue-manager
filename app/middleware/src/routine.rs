//! The Discover aggregator: periodically collects beginner-friendly issues from all of
//! GitHub with a server-wide token and keeps their open/closed state current.

use std::{fmt, time::Duration};

use graphql_client::{GraphQLQuery, Response};
use log::warn;
use serde::Serialize;
use sqlx::PgPool;

use crate::error::{Error, Result};
use crate::{search, status};

/// Stop paging when fewer GraphQL rate-limit points than this remain, so the server token
/// keeps headroom for other work.
pub(crate) const MIN_RATE_LIMIT_REMAINING: i64 = 100;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The requested number of issues was processed.
    #[default]
    LimitReached,
    /// Reached issues already stored by a previous run.
    CaughtUp,
    /// GitHub has no further search results.
    NoMoreResults,
    /// Stopped to preserve the GitHub rate limit.
    RateLimited,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct InsertReport {
    pub pages: usize,
    pub fetched: usize,
    pub stored: usize,
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpdateReport {
    pub checked: usize,
    pub still_open: u64,
    pub removed: u64,
}

/// Sends GraphQL queries to GitHub with the server token.
#[derive(Clone)]
pub(crate) struct GraphQLTransport {
    http: reqwest::Client,
    url: reqwest::Url,
    token: String,
}

impl fmt::Debug for GraphQLTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GraphQLTransport")
            .field("url", &self.url.as_str())
            .field("token", &"<redacted>")
            .finish()
    }
}

impl GraphQLTransport {
    pub(crate) async fn post<Q: GraphQLQuery>(&self, variables: Q::Variables) -> Result<Q::ResponseData> {
        let body = Q::build_query(variables);
        let response = self
            .http
            .post(self.url.clone())
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<Response<Q::ResponseData>>()
            .await?;

        let messages = response
            .errors
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ");

        match response.data {
            Some(data) => {
                if !messages.is_empty() {
                    // Partial data, e.g. some node ids no longer resolve. Callers handle the nulls.
                    warn!("GitHub GraphQL returned partial data: {messages}");
                }
                Ok(data)
            }
            None if messages.is_empty() => Err(Error::Missing("GraphQL response has no `data`".into())),
            None => Err(Error::GraphQL(messages)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Routine {
    pool: PgPool,
    transport: GraphQLTransport,
}

impl Routine {
    /// `graphql_url` is normally `https://api.github.com/graphql`; `token` is a server-owned
    /// GitHub token (no scopes are required to search public issues).
    pub fn new(pool: PgPool, graphql_url: &str, token: String, timeout: Duration) -> Result<Self> {
        let url = reqwest::Url::parse(graphql_url).map_err(|e| Error::Other(format!("invalid GRAPHQL_URL: {e}")))?;
        let http = reqwest::Client::builder()
            .user_agent(concat!("github-issue-manager/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .build()?;

        Ok(Self {
            pool,
            transport: GraphQLTransport { http, url, token },
        })
    }

    /// Collects up to `limit` newly created matching issues.
    pub async fn insert_issues(&self, limit: u16) -> Result<InsertReport> {
        search::collect(&self.pool, &self.transport, limit).await
    }

    /// Re-checks up to `limit` stored issues and removes those no longer open.
    pub async fn update_issues(&self, limit: u16) -> Result<UpdateReport> {
        status::refresh(&self.pool, &self.transport, limit).await
    }
}
