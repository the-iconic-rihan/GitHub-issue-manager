//! GitHub REST API client. Every call is made with the signed-in user's own OAuth token,
//! so GitHub enforces repository permissions.

mod error;
pub mod models;
pub mod params;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{
    header::{HeaderMap, ACCEPT, LINK},
    Method, RequestBuilder, Response,
};
use serde::{de::DeserializeOwned, Serialize};

pub use error::GitHubError;
use models::{Comment, Issue, IssuePatch, Label, NewIssue, Repository, SearchResult, SimpleUser, User};
use params::IssueQuery;

pub const DEFAULT_API_URL: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
/// GitHub's maximum page size.
pub const MAX_PER_PAGE: u32 = 100;

/// One page of results plus enough information to request the next one.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    pub has_next: bool,
    /// Only known for search results.
    pub total_count: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct GitHubClient {
    http: reqwest::Client,
    base: reqwest::Url,
}

impl GitHubClient {
    /// `api_url` is `https://api.github.com` or, for GitHub Enterprise Server,
    /// `https://<host>/api/v3`.
    pub fn new(api_url: &str, timeout: Duration) -> Result<Self, String> {
        let base = reqwest::Url::parse(api_url).map_err(|e| format!("invalid GitHub API URL '{api_url}': {e}"))?;
        if base.cannot_be_a_base() || !matches!(base.scheme(), "http" | "https") {
            return Err(format!("invalid GitHub API URL '{api_url}'"));
        }
        let http = reqwest::Client::builder()
            .user_agent(concat!("github-issue-manager/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| format!("could not build HTTP client: {e}"))?;
        Ok(Self { http, base })
    }

    /// Builds an endpoint URL. Each segment is percent-encoded, so user input such as
    /// label names can never alter the path structure.
    fn endpoint(&self, segments: &[&str]) -> reqwest::Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("base URL validated in GitHubClient::new")
            .pop_if_empty()
            .extend(segments);
        url
    }

    fn request(&self, method: Method, token: &str, segments: &[&str]) -> RequestBuilder {
        self.http
            .request(method, self.endpoint(segments))
            .bearer_auth(token)
            .header(ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
    }

    async fn execute(&self, request: RequestBuilder) -> Result<Response, GitHubError> {
        let response = request.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let headers = response.headers().clone();
        let body = response.bytes().await.unwrap_or_default();
        let err = error::classify(status.as_u16(), &headers, &body, unix_now());
        log::debug!("GitHub API error {status}: {err}");
        Err(err)
    }

    async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, GitHubError> {
        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| GitHubError::Decode(e.to_string()))
    }

    async fn get<T: DeserializeOwned>(&self, token: &str, segments: &[&str]) -> Result<T, GitHubError> {
        let response = self.execute(self.request(Method::GET, token, segments)).await?;
        Self::decode(response).await
    }

    async fn send_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        token: &str,
        segments: &[&str],
        body: &B,
    ) -> Result<T, GitHubError> {
        let response = self.execute(self.request(method, token, segments).json(body)).await?;
        Self::decode(response).await
    }

    async fn get_page<T: DeserializeOwned>(
        &self,
        token: &str,
        segments: &[&str],
        query: &[(&str, String)],
        page: u32,
        per_page: u32,
    ) -> Result<Page<T>, GitHubError> {
        let page = page.max(1);
        let per_page = per_page.clamp(1, MAX_PER_PAGE);
        let request = self
            .request(Method::GET, token, segments)
            .query(query)
            .query(&[("page", page), ("per_page", per_page)]);
        let response = self.execute(request).await?;
        let has_next = link_has_next(response.headers());
        let items = Self::decode(response).await?;
        Ok(Page {
            items,
            page,
            per_page,
            has_next,
            total_count: None,
        })
    }

    pub async fn authenticated_user(&self, token: &str) -> Result<User, GitHubError> {
        self.get(token, &["user"]).await
    }

    /// Repositories the user owns, collaborates on, or can access through an organisation.
    pub async fn list_user_repos(
        &self,
        token: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Repository>, GitHubError> {
        let query = [
            ("sort", "updated".to_owned()),
            ("affiliation", "owner,collaborator,organization_member".to_owned()),
        ];
        self.get_page(token, &["user", "repos"], &query, page, per_page).await
    }

    pub async fn get_repo(&self, token: &str, owner: &str, repo: &str) -> Result<Repository, GitHubError> {
        self.get(token, &["repos", owner, repo]).await
    }

    pub async fn list_repo_labels(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Label>, GitHubError> {
        self.get_page(token, &["repos", owner, repo, "labels"], &[], page, per_page)
            .await
    }

    pub async fn list_assignees(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Page<SimpleUser>, GitHubError> {
        self.get_page(token, &["repos", owner, repo, "assignees"], &[], page, per_page)
            .await
    }

    /// Lists issues (never pull requests). Uses the search API when free text is given or
    /// a label contains a comma (which the list API cannot express).
    pub async fn list_issues(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        query: &IssueQuery,
    ) -> Result<Page<Issue>, GitHubError> {
        let needs_search =
            query.text.as_deref().is_some_and(|t| !t.trim().is_empty()) || query.labels.iter().any(|l| l.contains(','));
        if needs_search {
            return self.search_issues(token, owner, repo, query).await;
        }

        let mut params = vec![
            ("state", query.state.as_str().to_owned()),
            ("sort", query.sort.as_str().to_owned()),
            ("direction", query.direction.as_str().to_owned()),
        ];
        if !query.labels.is_empty() {
            params.push(("labels", query.labels.join(",")));
        }
        if let Some(assignee) = &query.assignee {
            params.push(("assignee", assignee.as_list_param().to_owned()));
        }
        let mut page: Page<Issue> = self
            .get_page(
                token,
                &["repos", owner, repo, "issues"],
                &params,
                query.page,
                query.per_page,
            )
            .await?;
        page.items.retain(|issue| !issue.is_pull_request());
        Ok(page)
    }

    async fn search_issues(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        query: &IssueQuery,
    ) -> Result<Page<Issue>, GitHubError> {
        let page = query.page.max(1);
        let per_page = query.per_page.clamp(1, MAX_PER_PAGE);
        let request = self.request(Method::GET, token, &["search", "issues"]).query(&[
            ("q", query.search_query(owner, repo)),
            ("sort", query.sort.as_str().to_owned()),
            ("order", query.direction.as_str().to_owned()),
            ("page", page.to_string()),
            ("per_page", per_page.to_string()),
        ]);
        let response = self.execute(request).await?;
        let has_next = link_has_next(response.headers());
        let result: SearchResult<Issue> = Self::decode(response).await?;
        Ok(Page {
            items: result.items.into_iter().filter(|i| !i.is_pull_request()).collect(),
            page,
            per_page,
            has_next,
            total_count: Some(result.total_count),
        })
    }

    pub async fn get_issue(&self, token: &str, owner: &str, repo: &str, number: u64) -> Result<Issue, GitHubError> {
        let issue: Issue = self
            .get(token, &["repos", owner, repo, "issues", &number.to_string()])
            .await?;
        if issue.is_pull_request() {
            return Err(GitHubError::NotFound(format!(
                "#{number} is a pull request, not an issue"
            )));
        }
        Ok(issue)
    }

    pub async fn create_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        issue: &NewIssue,
    ) -> Result<Issue, GitHubError> {
        self.send_json(Method::POST, token, &["repos", owner, repo, "issues"], issue)
            .await
    }

    pub async fn update_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        patch: &IssuePatch,
    ) -> Result<Issue, GitHubError> {
        self.send_json(
            Method::PATCH,
            token,
            &["repos", owner, repo, "issues", &number.to_string()],
            patch,
        )
        .await
    }

    pub async fn list_comments(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Comment>, GitHubError> {
        let number = number.to_string();
        self.get_page(
            token,
            &["repos", owner, repo, "issues", &number, "comments"],
            &[],
            page,
            per_page,
        )
        .await
    }

    pub async fn create_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<Comment, GitHubError> {
        #[derive(Serialize)]
        struct Body<'a> {
            body: &'a str,
        }
        let number = number.to_string();
        self.send_json(
            Method::POST,
            token,
            &["repos", owner, repo, "issues", &number, "comments"],
            &Body { body },
        )
        .await
    }

    /// Adds labels to an issue; returns the issue's full label set.
    pub async fn add_labels(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        labels: &[String],
    ) -> Result<Vec<Label>, GitHubError> {
        #[derive(Serialize)]
        struct Body<'a> {
            labels: &'a [String],
        }
        let number = number.to_string();
        self.send_json(
            Method::POST,
            token,
            &["repos", owner, repo, "issues", &number, "labels"],
            &Body { labels },
        )
        .await
    }

    /// Removes one label from an issue; returns the remaining labels.
    pub async fn remove_label(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        label: &str,
    ) -> Result<Vec<Label>, GitHubError> {
        let number = number.to_string();
        let response = self
            .execute(self.request(
                Method::DELETE,
                token,
                &["repos", owner, repo, "issues", &number, "labels", label],
            ))
            .await?;
        Self::decode(response).await
    }

    /// Fetches every issue (open and closed, most recently updated first) for a full sync.
    /// Returns `truncated = true` when more than `max_pages` pages exist.
    pub async fn list_all_issues(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        max_pages: u32,
    ) -> Result<(Vec<Issue>, bool), GitHubError> {
        let params = [
            ("state", "all".to_owned()),
            ("sort", "updated".to_owned()),
            ("direction", "desc".to_owned()),
        ];
        let mut all = Vec::new();
        for page_number in 1..=max_pages.max(1) {
            let page: Page<Issue> = self
                .get_page(
                    token,
                    &["repos", owner, repo, "issues"],
                    &params,
                    page_number,
                    MAX_PER_PAGE,
                )
                .await?;
            all.extend(page.items.into_iter().filter(|i| !i.is_pull_request()));
            if !page.has_next {
                return Ok((all, false));
            }
        }
        Ok((all, true))
    }
}

/// True when the RFC 8288 `Link` header advertises a `rel="next"` page.
fn link_has_next(headers: &HeaderMap) -> bool {
    headers
        .get_all(LINK)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|link| {
            link.split(';')
                .skip(1)
                .any(|param| param.trim().eq_ignore_ascii_case(r#"rel="next""#))
        })
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn issue_json(number: i64, pr: bool) -> serde_json::Value {
        let mut v = serde_json::json!({
            "id": 1000 + number, "number": number, "title": format!("Issue {number}"),
            "body": null, "state": "open", "labels": [], "assignees": [], "comments": 0,
            "html_url": format!("https://github.com/acme/app/issues/{number}"),
            "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-02T00:00:00Z",
            "user": {"id": 1, "login": "octocat", "avatar_url": "https://a", "html_url": "https://h"}
        });
        if pr {
            v["pull_request"] = serde_json::json!({"url": "https://api.github.com/x"});
        }
        v
    }

    #[test]
    fn link_header_parsing() {
        let mut h = HeaderMap::new();
        assert!(!link_has_next(&h));
        h.insert(
            LINK,
            HeaderValue::from_static(
                r#"<https://api.github.com/x?page=2>; rel="next", <https://api.github.com/x?page=5>; rel="last""#,
            ),
        );
        assert!(link_has_next(&h));
        h.insert(
            LINK,
            HeaderValue::from_static(r#"<https://api.github.com/x?page=1>; rel="prev""#),
        );
        assert!(!link_has_next(&h));
    }

    #[test]
    fn endpoint_encodes_segments_and_keeps_base_path() {
        let client = GitHubClient::new("https://ghe.example.com/api/v3/", Duration::from_secs(1)).unwrap();
        let url = client.endpoint(&["repos", "acme", "app", "issues", "1", "labels", "area/ui needs?triage"]);
        assert_eq!(
            url.as_str(),
            "https://ghe.example.com/api/v3/repos/acme/app/issues/1/labels/area%2Fui%20needs%3Ftriage"
        );
    }

    #[test]
    fn rejects_invalid_base_url() {
        assert!(GitHubClient::new("not a url", Duration::from_secs(1)).is_err());
        assert!(GitHubClient::new("ftp://example.com", Duration::from_secs(1)).is_err());
    }

    #[tokio::test]
    async fn list_issues_filters_pull_requests_and_sends_filters() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/app/issues"))
            .and(header("authorization", "Bearer tok"))
            .and(query_param("state", "closed"))
            .and(query_param("labels", "bug,ui"))
            .and(query_param("assignee", "none"))
            .and(query_param("page", "2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!([issue_json(1, false), issue_json(2, true)]))
                    .insert_header("link", r#"<https://x?page=3>; rel="next""#),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = GitHubClient::new(&server.uri(), Duration::from_secs(5)).unwrap();
        let query = IssueQuery {
            state: params::IssueState::Closed,
            labels: vec!["bug".into(), "ui".into()],
            assignee: Some(params::AssigneeFilter::None),
            page: 2,
            per_page: 30,
            ..Default::default()
        };
        let page = client.list_issues("tok", "acme", "app", &query).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].number, 1);
        assert!(page.has_next);
        assert_eq!(page.page, 2);
    }

    #[tokio::test]
    async fn list_issues_with_text_uses_search() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .and(query_param("q", "repo:acme/app is:issue is:open crash"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total_count": 1, "incomplete_results": false, "items": [issue_json(7, false)]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = GitHubClient::new(&server.uri(), Duration::from_secs(5)).unwrap();
        let query = IssueQuery {
            text: Some("crash".into()),
            page: 1,
            per_page: 10,
            ..Default::default()
        };
        let page = client.list_issues("tok", "acme", "app", &query).await.unwrap();
        assert_eq!(page.total_count, Some(1));
        assert_eq!(page.items[0].number, 7);
    }

    #[tokio::test]
    async fn get_issue_rejects_pull_requests() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/app/issues/2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json(2, true)))
            .mount(&server)
            .await;
        let client = GitHubClient::new(&server.uri(), Duration::from_secs(5)).unwrap();
        let err = client.get_issue("tok", "acme", "app", 2).await.unwrap_err();
        assert!(matches!(err, GitHubError::NotFound(_)));
    }

    #[tokio::test]
    async fn rate_limit_is_classified() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_json(serde_json::json!({"message": "API rate limit exceeded"}))
                    .insert_header("x-ratelimit-remaining", "0")
                    .insert_header("x-ratelimit-reset", (unix_now() + 120).to_string()),
            )
            .mount(&server)
            .await;
        let client = GitHubClient::new(&server.uri(), Duration::from_secs(5)).unwrap();
        match client.authenticated_user("tok").await.unwrap_err() {
            GitHubError::RateLimited { retry_after_secs } => assert!((100..=120).contains(&retry_after_secs)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_all_issues_follows_pages_and_reports_truncation() {
        let server = MockServer::start().await;
        for p in 1..=3 {
            Mock::given(method("GET"))
                .and(path("/repos/acme/app/issues"))
                .and(query_param("page", p.to_string()))
                .and(query_param("state", "all"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([issue_json(p, false)]))
                        .insert_header("link", r#"<https://x>; rel="next""#),
                )
                .mount(&server)
                .await;
        }
        let client = GitHubClient::new(&server.uri(), Duration::from_secs(5)).unwrap();
        let (issues, truncated) = client.list_all_issues("tok", "acme", "app", 2).await.unwrap();
        assert_eq!(issues.len(), 2);
        assert!(truncated);
    }

    #[tokio::test]
    async fn remove_label_encodes_name() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/repos/acme/app/issues/3/labels/area%2Fui"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&server)
            .await;
        let client = GitHubClient::new(&server.uri(), Duration::from_secs(5)).unwrap();
        let labels = client.remove_label("tok", "acme", "app", 3, "area/ui").await.unwrap();
        assert!(labels.is_empty());
    }
}
