//! End-to-end API tests against a real PostgreSQL database and a fake GitHub.
//! Requires `DATABASE_URL`; each test uses its own schema.

mod common;

use actix_web::test::TestRequest;
use common::{db::TestDb, *};
use serde_json::{json, Value};
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

async fn mock_repo(gh: &MockServer, full_name: &str, id: i64, has_issues: bool) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{full_name}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(repo_json(id, full_name, has_issues)))
        .mount(gh)
        .await;
}

async fn mock_issue_list(gh: &MockServer, full_name: &str, issues: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{full_name}/issues")))
        .and(query_param("state", "all"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issues))
        .mount(gh)
        .await;
}

fn put(uri: &str, token: &str) -> TestRequest {
    authed(TestRequest::put().uri(uri), token)
}

fn post(uri: &str, token: &str) -> TestRequest {
    authed(TestRequest::post().uri(uri), token)
}

#[actix_web::test]
async fn me_connects_and_disconnect_removes_all_user_data() {
    let db = TestDb::new().await;
    let gh = fake_github().await;
    mock_repo(&gh, "acme/app", REPO_ID, true).await;
    let pool = db.pool().await;
    let app = init(state(pool.clone(), &gh)).await;

    let (status, body, _) = send(&app, get("/api/v1/me", TOKEN)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["user"]["login"], "octocat");
    assert_eq!(body["data"]["discover_enabled"], false);
    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(USER_ID)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(login, "octocat");

    let (status, _, _) = send(&app, put("/api/v1/tracked-repos/acme/app", TOKEN)).await;
    assert_eq!(status, 200);

    let (status, _, _) = send(&app, authed(TestRequest::delete().uri("/api/v1/me"), TOKEN)).await;
    assert_eq!(status, 204);
    let remaining: i64 =
        sqlx::query_scalar("SELECT (SELECT count(*) FROM users) + (SELECT count(*) FROM tracked_repositories)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0, "disconnect must cascade");
    db.drop().await;
}

#[actix_web::test]
async fn track_sync_edit_and_dashboard_workflow() {
    let db = TestDb::new().await;
    let gh = fake_github().await;
    mock_repo(&gh, "acme/app", REPO_ID, true).await;
    mock_issue_list(
        &gh,
        "acme/app",
        json!([
            issue_json(1, "open", &["bug"], &["octocat"], "2026-11-14T00:00:00Z"),
            issue_json(2, "open", &["bug", "ui"], &[], "2026-11-13T00:00:00Z"),
            issue_json(3, "closed", &["docs"], &[], "2026-11-12T00:00:00Z"),
        ]),
    )
    .await;
    let app = init(state(db.pool().await, &gh)).await;

    // Track (idempotent) and list.
    for _ in 0..2 {
        let (status, body, _) = send(&app, put("/api/v1/tracked-repos/acme/app", TOKEN)).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["data"]["repo_id"], REPO_ID);
    }
    let (_, body, _) = send(&app, get("/api/v1/tracked-repos", TOKEN)).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert!(body["data"][0]["last_synced_at"].is_null());

    // Sync.
    let (status, body, _) = send(&app, post("/api/v1/tracked-repos/acme/app/sync", TOKEN)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["issues_fetched"], 3);
    assert_eq!(body["data"]["truncated"], false);

    let (status, body, _) = send(&app, get("/api/v1/dashboard", TOKEN)).await;
    assert_eq!(status, 200, "{body}");
    let totals = &body["data"]["totals"];
    assert_eq!(totals["tracked_repositories"], 1);
    assert_eq!(totals["open_issues"], 2);
    assert_eq!(totals["closed_issues"], 1);
    assert_eq!(totals["unassigned_open"], 1);
    assert_eq!(totals["assigned_to_me_open"], 1);
    assert_eq!(body["data"]["top_labels"][0], json!({"label": "bug", "count": 2}));
    assert_eq!(body["data"]["recently_updated"][0]["number"], 1);
    assert!(!body["data"]["repositories"][0]["last_synced_at"].is_null());

    // Create: written through to the cache.
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/issues"))
        .respond_with(ResponseTemplate::new(201).set_body_json(issue_json(4, "open", &[], &[], "2026-11-16T00:00:00Z")))
        .mount(&gh)
        .await;
    let (status, _, _) = send(
        &app,
        post("/api/v1/repos/acme/app/issues", TOKEN).set_json(json!({"title": "New"})),
    )
    .await;
    assert_eq!(status, 201);

    // Close #2 and assign #4 to me.
    Mock::given(method("PATCH"))
        .and(path("/repos/acme/app/issues/2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_json(
            2,
            "closed",
            &["bug", "ui"],
            &[],
            "2026-11-16T01:00:00Z",
        )))
        .mount(&gh)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/repos/acme/app/issues/4"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_json(
            4,
            "open",
            &[],
            &["octocat"],
            "2026-11-16T02:00:00Z",
        )))
        .mount(&gh)
        .await;
    let patch = |n: u32, body: Value| {
        authed(
            TestRequest::patch().uri(&format!("/api/v1/repos/acme/app/issues/{n}")),
            TOKEN,
        )
        .set_json(body)
    };
    let (status, _, _) = send(&app, patch(2, json!({"state": "closed", "state_reason": "completed"}))).await;
    assert_eq!(status, 200);
    let (status, _, _) = send(&app, patch(4, json!({"assignees": ["octocat"]}))).await;
    assert_eq!(status, 200);

    // Label #1 and comment on it.
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/issues/1/labels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 1, "name": "bug", "color": "ff0000"}, {"id": 2, "name": "urgent", "color": "000000"}
        ])))
        .mount(&gh)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/app/issues/1/comments"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 5, "body": "hi", "user": simple_user("octocat"), "html_url": "https://x",
            "created_at": "2026-11-16T00:00:00Z", "updated_at": "2026-11-16T00:00:00Z"
        })))
        .mount(&gh)
        .await;
    let (status, _, _) = send(
        &app,
        post("/api/v1/repos/acme/app/issues/1/labels", TOKEN).set_json(json!({"labels": ["urgent"]})),
    )
    .await;
    assert_eq!(status, 200);
    let (status, _, _) = send(
        &app,
        post("/api/v1/repos/acme/app/issues/1/comments", TOKEN).set_json(json!({"body": "hi"})),
    )
    .await;
    assert_eq!(status, 201);

    let (_, body, _) = send(&app, get("/api/v1/dashboard", TOKEN)).await;
    let totals = &body["data"]["totals"];
    assert_eq!(totals["open_issues"], 2, "#1 and #4 open: {body}");
    assert_eq!(totals["closed_issues"], 2, "#2 and #3 closed");
    assert_eq!(totals["unassigned_open"], 0);
    assert_eq!(totals["assigned_to_me_open"], 2);
    let labels = body["data"]["top_labels"].as_array().unwrap();
    assert!(labels.contains(&json!({"label": "urgent", "count": 1})), "{labels:?}");
    let comments: i32 = sqlx::query_scalar("SELECT comments FROM repo_issues WHERE number = 1")
        .fetch_one(&db.pool().await)
        .await
        .unwrap();
    assert_eq!(comments, 2);

    // A stale write (older updated_at) must not overwrite newer cached data.
    let (status, _, _) = send(&app, post("/api/v1/tracked-repos/acme/app/sync", TOKEN)).await;
    assert_eq!(status, 200);
    let state: String = sqlx::query_scalar("SELECT state FROM repo_issues WHERE number = 2")
        .fetch_one(&db.pool().await)
        .await
        .unwrap();
    assert_eq!(state, "closed", "older GitHub snapshot must not reopen #2 in the cache");

    // Untrack removes the cache.
    let (status, _, _) = send(
        &app,
        authed(TestRequest::delete().uri("/api/v1/tracked-repos/acme/app"), TOKEN),
    )
    .await;
    assert_eq!(status, 204);
    let cached: i64 = sqlx::query_scalar("SELECT count(*) FROM repo_issues")
        .fetch_one(&db.pool().await)
        .await
        .unwrap();
    assert_eq!(cached, 0);
    let (status, _, _) = send(
        &app,
        authed(TestRequest::delete().uri("/api/v1/tracked-repos/acme/app"), TOKEN),
    )
    .await;
    assert_eq!(status, 404);
    db.drop().await;
}

#[actix_web::test]
async fn tracking_and_sync_errors() {
    let db = TestDb::new().await;
    let gh = fake_github().await;
    mock_repo(&gh, "acme/noissues", 1, false).await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({"message": "Not Found"})))
        .mount(&gh)
        .await;
    let app = init(state(db.pool().await, &gh)).await;

    let (status, _, _) = send(&app, put("/api/v1/tracked-repos/acme/noissues", TOKEN)).await;
    assert_eq!(status, 409);
    let (status, _, _) = send(&app, put("/api/v1/tracked-repos/acme/missing", TOKEN)).await;
    assert_eq!(status, 404);
    let (status, body, _) = send(&app, post("/api/v1/tracked-repos/acme/app/sync", TOKEN)).await;
    assert_eq!(status, 404, "{body}");
    db.drop().await;
}

#[actix_web::test]
async fn sync_detects_renamed_repository_and_handles_truncation() {
    let db = TestDb::new().await;
    let gh = fake_github().await;
    mock_repo(&gh, "acme/app", REPO_ID, true).await;
    let pool = db.pool().await;
    let app = init(state(pool.clone(), &gh)).await;
    let (status, _, _) = send(&app, put("/api/v1/tracked-repos/acme/app", TOKEN)).await;
    assert_eq!(status, 200);

    // More pages than sync_max_pages (3): the snapshot is partial, so nothing is deleted.
    sqlx::query(
        "INSERT INTO repo_issues (user_id, repo_id, number, github_id, title, state, html_url, created_at, updated_at) \
         VALUES ($1, $2, 999, 1, 'old', 'open', 'https://x', now(), '2000-01-01')",
    )
    .bind(USER_ID)
    .bind(REPO_ID)
    .execute(&pool)
    .await
    .unwrap();
    Mock::given(method("GET"))
        .and(path("/repos/acme/app/issues"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([issue_json(1, "open", &[], &[], "2026-11-14T00:00:00Z")]))
                .insert_header("link", r#"<https://x>; rel="next""#),
        )
        .mount(&gh)
        .await;
    let (status, body, _) = send(&app, post("/api/v1/tracked-repos/acme/app/sync", TOKEN)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["truncated"], true);
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM repo_issues WHERE number = 999")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kept, 1, "truncated sync must not delete unseen issues");

    // The name now points at a different repository id.
    gh.reset().await;
    let gh2 = fake_github().await;
    mock_repo(&gh2, "acme/app", REPO_ID + 1, true).await;
    let app = init(state(pool.clone(), &gh2)).await;
    let (status, _, _) = send(&app, post("/api/v1/tracked-repos/acme/app/sync", TOKEN)).await;
    assert_eq!(status, 409);
    db.drop().await;
}

#[actix_web::test]
async fn complete_sync_removes_deleted_issues() {
    let db = TestDb::new().await;
    let gh = fake_github().await;
    mock_repo(&gh, "acme/app", REPO_ID, true).await;
    mock_issue_list(
        &gh,
        "acme/app",
        json!([issue_json(1, "open", &[], &[], "2026-11-14T00:00:00Z")]),
    )
    .await;
    let pool = db.pool().await;
    let app = init(state(pool.clone(), &gh)).await;
    send(&app, put("/api/v1/tracked-repos/acme/app", TOKEN)).await;
    sqlx::query(
        "INSERT INTO repo_issues (user_id, repo_id, number, github_id, title, state, html_url, created_at, updated_at) \
         VALUES ($1, $2, 999, 1, 'deleted on GitHub', 'open', 'https://x', now(), now())",
    )
    .bind(USER_ID)
    .bind(REPO_ID)
    .execute(&pool)
    .await
    .unwrap();
    let (status, _, _) = send(&app, post("/api/v1/tracked-repos/acme/app/sync", TOKEN)).await;
    assert_eq!(status, 200);
    let numbers: Vec<i32> = sqlx::query_scalar("SELECT number FROM repo_issues ORDER BY number")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(numbers, vec![1]);
    db.drop().await;
}

#[actix_web::test]
async fn users_cannot_see_each_others_data() {
    let db = TestDb::new().await;
    let gh = fake_github().await;
    mock_repo(&gh, "acme/private", REPO_ID, true).await;
    mock_issue_list(
        &gh,
        "acme/private",
        json!([issue_json(1, "open", &["secret"], &[], "2026-11-14T00:00:00Z")]),
    )
    .await;
    let app = init(state(db.pool().await, &gh)).await;

    send(&app, put("/api/v1/tracked-repos/acme/private", TOKEN)).await;
    let (status, _, _) = send(&app, post("/api/v1/tracked-repos/acme/private/sync", TOKEN)).await;
    assert_eq!(status, 200);

    let (_, body, _) = send(&app, get("/api/v1/dashboard", OTHER_TOKEN)).await;
    assert_eq!(body["data"]["totals"]["tracked_repositories"], 0);
    assert_eq!(body["data"]["top_labels"], json!([]));
    assert_eq!(body["data"]["recently_updated"], json!([]));
    let (_, body, _) = send(&app, get("/api/v1/tracked-repos", OTHER_TOKEN)).await;
    assert_eq!(body["data"], json!([]));
    // Hubot cannot sync or untrack octocat's tracked repository.
    let (status, _, _) = send(&app, post("/api/v1/tracked-repos/acme/private/sync", OTHER_TOKEN)).await;
    assert_eq!(status, 404);
    let (status, _, _) = send(
        &app,
        authed(
            TestRequest::delete().uri("/api/v1/tracked-repos/acme/private"),
            OTHER_TOKEN,
        ),
    )
    .await;
    assert_eq!(status, 404);
    db.drop().await;
}

#[actix_web::test]
async fn data_persists_across_restarts_and_migrations_are_idempotent() {
    let db = TestDb::new().await;
    let gh = fake_github().await;
    mock_repo(&gh, "acme/app", REPO_ID, true).await;
    {
        let pool = db.pool().await;
        let app = init(state(pool.clone(), &gh)).await;
        let (status, _, _) = send(&app, put("/api/v1/tracked-repos/acme/app", TOKEN)).await;
        assert_eq!(status, 200);
        pool.close().await;
    }
    // "Restart": new pool, migrations re-run on startup, new app instance.
    let pool = db.pool().await;
    middleware::db::migrate(&pool)
        .await
        .expect("re-running migrations is a no-op");
    let app = init(state(pool, &gh)).await;
    let (_, body, _) = send(&app, get("/api/v1/tracked-repos", TOKEN)).await;
    assert_eq!(body["data"][0]["full_name"], "acme/app");
    db.drop().await;
}

#[actix_web::test]
async fn readiness_reports_ok_with_database() {
    let db = TestDb::new().await;
    let gh = fake_github().await;
    let app = init(state(db.pool().await, &gh)).await;
    let (status, body, _) = send(&app, TestRequest::get().uri("/api/v1/health/ready")).await;
    assert_eq!(status, 200, "{body}");
    db.drop().await;
}

#[actix_web::test]
async fn discover_lists_filters_and_paginates() {
    use middleware::issue::Issue;
    let db = TestDb::new().await;
    let gh = fake_github().await;
    let pool = db.pool().await;
    for (i, (lang, label)) in [
        ("Rust", "good first issue"),
        ("Rust", "help wanted"),
        ("Go", "good first issue"),
    ]
    .into_iter()
    .enumerate()
    {
        Issue::insert(
            &pool,
            &Issue {
                id: format!("I_{i}"),
                title: format!("Issue {i}"),
                created_at: chrono::Utc::now() - chrono::Duration::hours(i as i64),
                url: format!("https://github.com/a/b/issues/{i}"),
                labels: vec![label.into()],
                repository_name: "a/b".into(),
                fork_count: 1,
                star_count: 2,
                primary_language: lang.into(),
            },
        )
        .await
        .unwrap();
    }
    let app = init(state(pool, &gh)).await;

    let (status, body, _) = send(&app, get("/api/v1/discover/issues?language=rust&per_page=1", TOKEN)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["meta"],
        json!({"page": 1, "per_page": 1, "has_next": true, "total_count": 2})
    );
    assert_eq!(body["data"][0]["id"], "I_0", "newest first");

    let (_, body, _) = send(&app, get("/api/v1/discover/issues?label=GOOD%20FIRST%20ISSUE", TOKEN)).await;
    assert_eq!(body["meta"]["total_count"], 2);

    let (_, body, _) = send(&app, get("/api/v1/discover/languages", TOKEN)).await;
    assert_eq!(
        body["data"],
        json!([{"language": "Rust", "count": 2}, {"language": "Go", "count": 1}])
    );

    let (status, _, _) = send(&app, TestRequest::get().uri("/api/v1/discover/issues")).await;
    assert_eq!(status, 401);
    db.drop().await;
}
