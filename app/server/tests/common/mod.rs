//! Test helpers: a fake GitHub (wiremock) and app construction.
#![allow(dead_code)]

pub mod db;

use std::time::Duration;

use actix_web::{
    body::{to_bytes, MessageBody},
    dev::{Service, ServiceResponse},
    test, web,
};
use middleware::github::GitHubClient;
use serde_json::{json, Value};
use server::{auth::AuthCache, build_app, state::AppState};
use sqlx::{postgres::PgPoolOptions, PgPool};
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

pub const TOKEN: &str = "gho_valid_token_for_octocat";
pub const OTHER_TOKEN: &str = "gho_valid_token_for_hubot";
pub const USER_ID: i64 = 42;
pub const OTHER_USER_ID: i64 = 7;
pub const REPO_ID: i64 = 9001;

pub fn user_json(id: i64, login: &str) -> Value {
    json!({
        "id": id, "login": login, "name": format!("{login} name"),
        "avatar_url": format!("https://avatars.example/{login}"),
        "html_url": format!("https://github.com/{login}")
    })
}

pub fn simple_user(login: &str) -> Value {
    json!({ "id": login.len() as i64, "login": login, "avatar_url": "https://a", "html_url": "https://h" })
}

pub fn repo_json(id: i64, full_name: &str, has_issues: bool) -> Value {
    let (owner, name) = full_name.split_once('/').unwrap();
    json!({
        "id": id, "name": name, "full_name": full_name, "owner": simple_user(owner),
        "private": false, "description": "A repo", "html_url": format!("https://github.com/{full_name}"),
        "open_issues_count": 2, "has_issues": has_issues, "archived": false,
        "updated_at": "2026-11-01T00:00:00Z",
        "permissions": { "admin": true, "maintain": true, "push": true, "triage": true, "pull": true }
    })
}

pub fn issue_json(number: i64, state: &str, labels: &[&str], assignees: &[&str], updated_at: &str) -> Value {
    json!({
        "id": 50_000 + number, "number": number, "title": format!("Issue {number}"), "body": "Body",
        "state": state, "state_reason": if state == "closed" { json!("completed") } else { Value::Null },
        "user": simple_user("octocat"),
        "labels": labels.iter().enumerate().map(|(i, l)| json!({"id": i, "name": l, "color": "ff0000", "description": null})).collect::<Vec<_>>(),
        "assignees": assignees.iter().map(|a| simple_user(a)).collect::<Vec<_>>(),
        "comments": 1, "html_url": format!("https://github.com/acme/app/issues/{number}"), "locked": false,
        "created_at": "2026-11-10T00:00:00Z", "updated_at": updated_at,
        "closed_at": if state == "closed" { json!("2026-11-12T00:00:00Z") } else { Value::Null }
    })
}

/// Starts a fake GitHub that knows two users. Any other token gets 401.
pub async fn fake_github() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(user_json(USER_ID, "octocat")))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("authorization", format!("Bearer {OTHER_TOKEN}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(user_json(OTHER_USER_ID, "hubot")))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"message": "Bad credentials"})))
        .with_priority(10)
        .mount(&server)
        .await;
    server
}

/// A pool that never connects (for tests that must not touch the database).
pub fn unreachable_pool() -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(500))
        .connect_lazy("postgres://nobody:nothing@127.0.0.1:1/none")
        .expect("lazy pool")
}

pub fn app_state(pool: PgPool, github: &MockServer) -> AppState {
    AppState {
        pool,
        github: GitHubClient::new(&github.uri(), Duration::from_secs(5)).unwrap(),
        auth_cache: AuthCache::new(Duration::from_secs(60)),
        sync_max_pages: 3,
        auto_sync: server::state::AutoSync::default(),
        discover: None,
        commander_token: None,
        admin_token: None,
    }
}

pub fn state(pool: PgPool, github: &MockServer) -> web::Data<AppState> {
    web::Data::new(app_state(pool, github))
}

pub async fn init(
    state: web::Data<AppState>,
) -> impl Service<actix_http::Request, Response = ServiceResponse<impl MessageBody>, Error = actix_web::Error> {
    test::init_service(build_app(state)).await
}

/// Sends a request and returns `(status, json body or Null, retry-after header)`.
pub async fn send<S, B>(app: &S, req: test::TestRequest) -> (u16, Value, Option<String>)
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let resp = test::call_service(app, req.to_request()).await;
    let status = resp.status().as_u16();
    let retry = resp
        .headers()
        .get("retry-after")
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = to_bytes(resp.into_body()).await.ok().unwrap_or_default();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
    };
    (status, body, retry)
}

pub fn get(uri: &str, token: &str) -> test::TestRequest {
    test::TestRequest::get()
        .uri(uri)
        .insert_header(("Authorization", format!("Bearer {token}")))
}

pub fn with_json(req: test::TestRequest, body: Value) -> test::TestRequest {
    req.set_json(body)
}

pub fn authed(req: test::TestRequest, token: &str) -> test::TestRequest {
    req.insert_header(("Authorization", format!("Bearer {token}")))
}
