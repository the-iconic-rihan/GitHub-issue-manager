//! HTTP API tests that need no database: auth, validation, GitHub error mapping, and
//! request forwarding. The database pool points nowhere, so these also prove that a cache
//! (database) outage never breaks a successful GitHub write.

mod common;

use actix_web::test::TestRequest;
use common::*;
use serde_json::json;
use wiremock::{
    matchers::{body_json, method, path, query_param},
    Mock, ResponseTemplate,
};

#[actix_web::test]
async fn liveness_is_public() {
    let gh = fake_github().await;
    let app = init(state(unreachable_pool(), &gh)).await;
    let (status, body, _) = send(&app, TestRequest::get().uri("/api/v1/health/live")).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["status"], "ok");
}

#[actix_web::test]
async fn readiness_fails_without_database() {
    let gh = fake_github().await;
    let app = init(state(unreachable_pool(), &gh)).await;
    let (status, body, _) = send(&app, TestRequest::get().uri("/api/v1/health/ready")).await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "service_unavailable");
}

#[actix_web::test]
async fn unknown_routes_return_json_404() {
    let gh = fake_github().await;
    let app = init(state(unreachable_pool(), &gh)).await;
    for uri in ["/api/v1/nope", "/elsewhere"] {
        let (status, body, _) = send(&app, TestRequest::get().uri(uri)).await;
        assert_eq!(status, 404, "{uri}");
        assert_eq!(body["error"]["code"], "not_found");
    }
}

#[actix_web::test]
async fn requests_without_or_with_bad_tokens_are_rejected() {
    let gh = fake_github().await;
    let app = init(state(unreachable_pool(), &gh)).await;

    let (status, body, _) = send(&app, TestRequest::get().uri("/api/v1/repos")).await;
    assert_eq!(status, 401);
    assert_eq!(body["error"]["code"], "unauthorized");

    let (status, _, _) = send(&app, get("/api/v1/repos", "revoked-token")).await;
    assert_eq!(status, 401);

    let (status, _, _) = send(
        &app,
        TestRequest::get()
            .uri("/api/v1/repos")
            .insert_header(("Authorization", "Basic abc")),
    )
    .await;
    assert_eq!(status, 401);
}

#[actix_web::test]
async fn token_validation_is_cached() {
    let gh = fake_github().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/issues"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&gh)
        .await;
    let app = init(state(unreachable_pool(), &gh)).await;
    for _ in 0..3 {
        let (status, _, _) = send(&app, get("/api/v1/repos/acme/app/issues", TOKEN)).await;
        assert_eq!(status, 200);
    }
    let user_calls = gh
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.url.path() == "/user")
        .count();
    assert_eq!(user_calls, 1, "token should be validated once and cached");
}

#[actix_web::test]
async fn list_issues_forwards_filters_and_returns_pagination_meta() {
    let gh = fake_github().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/issues"))
        .and(query_param("state", "all"))
        .and(query_param("labels", "bug,help wanted"))
        .and(query_param("assignee", "octocat"))
        .and(query_param("sort", "updated"))
        .and(query_param("direction", "asc"))
        .and(query_param("page", "2"))
        .and(query_param("per_page", "10"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([issue_json(
                    3,
                    "open",
                    &["bug"],
                    &["octocat"],
                    "2026-11-15T00:00:00Z"
                )]))
                .insert_header("link", r#"<https://api.github.com/x?page=3>; rel="next""#),
        )
        .expect(1)
        .mount(&gh)
        .await;
    let app = init(state(unreachable_pool(), &gh)).await;
    let uri = "/api/v1/repos/acme/app/issues?state=all&labels=bug,help%20wanted&assignee=octocat&sort=updated&direction=asc&page=2&per_page=10";
    let (status, body, _) = send(&app, get(uri, TOKEN)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"][0]["number"], 3);
    assert_eq!(body["meta"], json!({"page": 2, "per_page": 10, "has_next": true}));
    assert!(body["data"][0].get("pull_request").is_none());
}

#[actix_web::test]
async fn search_returns_total_count() {
    let gh = fake_github().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "repo:acme/app is:issue is:open login bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 41, "incomplete_results": false,
            "items": [issue_json(9, "open", &[], &[], "2026-11-15T00:00:00Z")]
        })))
        .expect(1)
        .mount(&gh)
        .await;
    let app = init(state(unreachable_pool(), &gh)).await;
    let (status, body, _) = send(&app, get("/api/v1/repos/acme/app/issues?q=login%20bug", TOKEN)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["meta"]["total_count"], 41);
}

#[actix_web::test]
async fn invalid_input_is_rejected_before_calling_github() {
    let gh = fake_github().await;
    let app = init(state(unreachable_pool(), &gh)).await;

    let (status, body, _) = send(&app, get("/api/v1/repos/acme/app/issues?per_page=500", TOKEN)).await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "per_page");

    let (status, _, _) = send(&app, get("/api/v1/repos/acme/app/issues?state=bogus", TOKEN)).await;
    assert_eq!(status, 400);

    let (status, body, _) = send(&app, get("/api/v1/repos/ac%20me/app/issues", TOKEN)).await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "owner");

    let (status, _, _) = send(&app, get("/api/v1/repos/acme/app/issues/abc", TOKEN)).await;
    assert_eq!(status, 400);

    let create = authed(TestRequest::post().uri("/api/v1/repos/acme/app/issues"), TOKEN);
    let (status, body, _) = send(&app, create.set_json(json!({"title": "   "}))).await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "title");

    let create = authed(TestRequest::post().uri("/api/v1/repos/acme/app/issues"), TOKEN);
    let (status, _, _) = send(&app, create.set_json(json!({"title": "x", "unexpected": 1}))).await;
    assert_eq!(status, 400);

    let malformed = authed(TestRequest::post().uri("/api/v1/repos/acme/app/issues"), TOKEN)
        .insert_header(("content-type", "application/json"))
        .set_payload("{not json");
    let (status, body, _) = send(&app, malformed).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "bad_request");

    let patch = authed(TestRequest::patch().uri("/api/v1/repos/acme/app/issues/1"), TOKEN);
    let (status, _, _) = send(&app, patch.set_json(json!({}))).await;
    assert_eq!(status, 422);

    let comment = authed(
        TestRequest::post().uri("/api/v1/repos/acme/app/issues/1/comments"),
        TOKEN,
    );
    let (status, _, _) = send(&app, comment.set_json(json!({"body": "  "}))).await;
    assert_eq!(status, 422);

    // Only the token validation call reached GitHub.
    let paths: Vec<String> = gh
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().to_owned())
        .collect();
    assert!(paths.iter().all(|p| p == "/user"), "unexpected GitHub calls: {paths:?}");
}

#[actix_web::test]
async fn create_issue_succeeds_even_when_cache_is_unavailable() {
    let gh = fake_github().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/issues"))
        .and(body_json(
            json!({"title": "New bug", "body": "Steps", "labels": ["bug"], "assignees": ["octocat"]}),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(issue_json(
            12,
            "open",
            &["bug"],
            &["octocat"],
            "2026-11-16T00:00:00Z",
        )))
        .expect(1)
        .mount(&gh)
        .await;
    let app = init(state(unreachable_pool(), &gh)).await;
    let req = authed(TestRequest::post().uri("/api/v1/repos/acme/app/issues"), TOKEN).set_json(json!({
        "title": " New bug ", "body": "Steps", "labels": ["bug"], "assignees": ["octocat"]
    }));
    let (status, body, _) = send(&app, req).await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["data"]["number"], 12);
}

#[actix_web::test]
async fn close_and_reopen_send_state_reason() {
    let gh = fake_github().await;
    Mock::given(method("PATCH"))
        .and(path("/repos/acme/app/issues/5"))
        .and(body_json(json!({"state": "closed", "state_reason": "not_planned"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_json(
            5,
            "closed",
            &[],
            &[],
            "2026-11-16T00:00:00Z",
        )))
        .expect(1)
        .mount(&gh)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/repos/acme/app/issues/5"))
        .and(body_json(json!({"state": "open", "state_reason": "reopened"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_json(5, "open", &[], &[], "2026-11-16T01:00:00Z")))
        .expect(1)
        .mount(&gh)
        .await;
    let app = init(state(unreachable_pool(), &gh)).await;

    let close = authed(TestRequest::patch().uri("/api/v1/repos/acme/app/issues/5"), TOKEN)
        .set_json(json!({"state": "closed", "state_reason": "not_planned"}));
    let (status, body, _) = send(&app, close).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["state"], "closed");

    let reopen = authed(TestRequest::patch().uri("/api/v1/repos/acme/app/issues/5"), TOKEN)
        .set_json(json!({"state": "open", "state_reason": "reopened"}));
    let (status, body, _) = send(&app, reopen).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["state"], "open");
}

#[actix_web::test]
async fn assignees_labels_and_comments_are_forwarded() {
    let gh = fake_github().await;
    Mock::given(method("PATCH"))
        .and(path("/repos/acme/app/issues/5"))
        .and(body_json(json!({"assignees": ["hubot"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_json(
            5,
            "open",
            &[],
            &["hubot"],
            "2026-11-16T00:00:00Z",
        )))
        .expect(1)
        .mount(&gh)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/issues/5/labels"))
        .and(body_json(json!({"labels": ["area/ui"]})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{"id": 1, "name": "area/ui", "color": "00ff00", "description": null}])),
        )
        .expect(1)
        .mount(&gh)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/repos/acme/app/issues/5/labels/area%2Fui"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&gh)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/issues/5/comments"))
        .and(body_json(json!({"body": "Looking into it"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 77, "body": "Looking into it", "user": simple_user("octocat"),
            "html_url": "https://github.com/acme/app/issues/5#issuecomment-77", "author_association": "OWNER",
            "created_at": "2026-11-16T00:00:00Z", "updated_at": "2026-11-16T00:00:00Z"
        })))
        .expect(1)
        .mount(&gh)
        .await;
    let app = init(state(unreachable_pool(), &gh)).await;

    let assign = authed(TestRequest::patch().uri("/api/v1/repos/acme/app/issues/5"), TOKEN)
        .set_json(json!({"assignees": ["hubot", "HUBOT"]}));
    let (status, body, _) = send(&app, assign).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["assignees"][0]["login"], "hubot");

    let add = authed(TestRequest::post().uri("/api/v1/repos/acme/app/issues/5/labels"), TOKEN)
        .set_json(json!({"labels": ["area/ui"]}));
    let (status, body, _) = send(&app, add).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"][0]["name"], "area/ui");

    let remove = authed(
        TestRequest::delete().uri("/api/v1/repos/acme/app/issues/5/labels?name=area%2Fui"),
        TOKEN,
    );
    let (status, body, _) = send(&app, remove).await;
    assert_eq!(status, 200, "{body}");

    let comment = authed(
        TestRequest::post().uri("/api/v1/repos/acme/app/issues/5/comments"),
        TOKEN,
    )
    .set_json(json!({"body": "Looking into it"}));
    let (status, body, _) = send(&app, comment).await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["data"]["id"], 77);
}

#[actix_web::test]
async fn github_errors_are_mapped() {
    let gh = fake_github().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/limited/issues"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({"message": "API rate limit exceeded"}))
                .insert_header("retry-after", "30"),
        )
        .mount(&gh)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/secret/issues/1"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({"message": "Not Found"})))
        .mount(&gh)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/repos/acme/app/issues/1"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({"message": "Must have push access"})))
        .mount(&gh)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/issues"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "message": "Validation Failed",
            "errors": [{"resource": "Issue", "field": "assignees", "code": "invalid", "value": "ghost"}]
        })))
        .mount(&gh)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/down/issues"))
        .respond_with(ResponseTemplate::new(503).set_body_string("unavailable"))
        .mount(&gh)
        .await;
    let app = init(state(unreachable_pool(), &gh)).await;

    let (status, body, retry) = send(&app, get("/api/v1/repos/acme/limited/issues", TOKEN)).await;
    assert_eq!((status, retry.as_deref()), (429, Some("30")));
    assert_eq!(body["error"]["code"], "rate_limited");

    let (status, _, _) = send(&app, get("/api/v1/repos/acme/secret/issues/1", TOKEN)).await;
    assert_eq!(status, 404);

    let patch =
        authed(TestRequest::patch().uri("/api/v1/repos/acme/app/issues/1"), TOKEN).set_json(json!({"title": "new"}));
    let (status, body, _) = send(&app, patch).await;
    assert_eq!(status, 403);
    assert!(body["error"]["message"].as_str().unwrap().contains("push access"));

    let create = authed(TestRequest::post().uri("/api/v1/repos/acme/app/issues"), TOKEN)
        .set_json(json!({"title": "x", "assignees": ["ghost"]}));
    let (status, body, _) = send(&app, create).await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["code"], "github_validation_failed");
    assert!(body["error"]["message"].as_str().unwrap().contains("ghost"));

    let (status, body, _) = send(&app, get("/api/v1/repos/acme/down/issues", TOKEN)).await;
    assert_eq!(status, 502);
    assert_eq!(body["error"]["code"], "github_unavailable");
}

#[actix_web::test]
async fn commands_require_configuration_and_token() {
    let gh = fake_github().await;
    let app = init(state(unreachable_pool(), &gh)).await;
    let req = TestRequest::post()
        .uri("/api/v1/command")
        .set_json(json!({"action": "insert", "target": "issues", "limit": 10}));
    let (status, _, _) = send(&app, authed(req, "whatever-token-value")).await;
    assert_eq!(status, 503, "commands are disabled without configured tokens");

    let mut st = app_state(unreachable_pool(), &gh);
    st.commander_token = Some("commander-secret-token".into());
    let app = init(actix_web::web::Data::new(st)).await;
    let req = TestRequest::post()
        .uri("/api/v1/command")
        .set_json(json!({"action": "insert", "target": "issues", "limit": 10}));
    let (status, _, _) = send(&app, authed(req, "wrong-commander-token")).await;
    assert_eq!(status, 401);

    let req = TestRequest::post()
        .uri("/api/v1/command")
        .set_json(json!({"action": "insert", "target": "issues", "limit": 10}));
    let (status, body, _) = send(&app, authed(req, "commander-secret-token")).await;
    assert_eq!(status, 503, "Discover is disabled: {body}");

    let req = TestRequest::post()
        .uri("/api/v1/command")
        .set_json(json!({"action": "insert", "target": "repos", "limit": 10}));
    let (status, _, _) = send(&app, authed(req, "commander-secret-token")).await;
    assert_eq!(status, 422);
}
