//! Discover aggregator tests against a fake GitHub GraphQL endpoint and a real PostgreSQL
//! database (requires `DATABASE_URL`; each test uses its own schema).

use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use middleware::{issue::Issue, routine::Routine, routine::StopReason};
use serde_json::{json, Value};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use wiremock::{
    matchers::{body_string_contains, method, path},
    Mock, MockServer, ResponseTemplate,
};

static COUNTER: AtomicU32 = AtomicU32::new(0);

async fn test_pool() -> (PgPool, String) {
    let url = std::env::var("DATABASE_URL").expect("database tests need DATABASE_URL; see README > Running tests");
    let base: PgConnectOptions = url.parse().expect("valid DATABASE_URL");
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let schema = format!(
        "gim_mw_test_{}_{}_{}",
        std::process::id(),
        nanos % 1_000_000_000,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base.clone())
        .await
        .unwrap();
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(base.options([("search_path", schema.as_str())]))
        .await
        .unwrap();
    middleware::db::migrate(&pool).await.unwrap();
    (pool, schema)
}

async fn drop_schema(pool: PgPool, schema: String) {
    let _ = sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&pool)
        .await;
    pool.close().await;
}

fn search_node(id: &str, created_at: &str, labels: Value) -> Value {
    json!({ "__typename": "SearchResultItemEdge", "node": {
        "__typename": "Issue", "id": id, "title": format!("Title {id}"), "createdAt": created_at,
        "url": format!("https://github.com/acme/app/issues/{id}"),
        "labels": labels,
        "repository": {
            "__typename": "Repository", "nameWithOwner": "acme/app", "forkCount": 3, "stargazerCount": 10,
            "primaryLanguage": { "__typename": "Language", "name": "Rust" }
        }
    }})
}

fn search_page(edges: Vec<Value>, has_next: bool, cursor: &str) -> Value {
    json!({ "data": {
        "rateLimit": { "__typename": "RateLimit", "limit": 5000, "cost": 1, "remaining": 4999, "resetAt": "2030-01-01T00:00:00Z" },
        "search": {
            "__typename": "SearchResultItemConnection",
            "pageInfo": { "__typename": "PageInfo", "startCursor": "s", "hasNextPage": has_next, "endCursor": cursor },
            "edges": edges
        }
    }})
}

fn routine(pool: &PgPool, server: &MockServer) -> Routine {
    Routine::new(
        pool.clone(),
        &format!("{}/graphql", server.uri()),
        "server-token-for-tests".into(),
        Duration::from_secs(5),
    )
    .unwrap()
}

#[tokio::test]
async fn insert_collects_pages_and_tolerates_missing_labels() {
    let (pool, schema) = test_pool().await;
    let server = MockServer::start().await;
    // Page 1 (no cursor yet) then page 2 (after "c1").
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains(r#""after":null"#))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_page(
            vec![
                search_node(
                    "I_3",
                    "2026-11-03T00:00:00Z",
                    json!({"nodes": [{"name": "good first issue"}]}),
                ),
                search_node("I_2", "2026-11-02T00:00:00Z", Value::Null),
                json!(null),
            ],
            true,
            "c1",
        )))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains(r#""after":"c1""#))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_page(
            vec![search_node("I_1", "2026-11-01T00:00:00Z", json!({"nodes": []}))],
            false,
            "c2",
        )))
        .expect(1)
        .mount(&server)
        .await;

    let report = routine(&pool, &server).insert_issues(150).await.unwrap();
    assert_eq!(report.stored, 3);
    assert_eq!(report.pages, 2);
    assert_eq!(report.stop_reason, StopReason::NoMoreResults);

    let issue = Issue::retrieve(&pool, "I_3").await.unwrap().unwrap();
    assert_eq!(issue.labels, vec!["good first issue"]);
    assert_eq!(issue.repository_name, "acme/app");
    assert_eq!(issue.primary_language, "Rust");
    assert!(Issue::retrieve(&pool, "I_2").await.unwrap().unwrap().labels.is_empty());
    drop_schema(pool, schema).await;
}

#[tokio::test]
async fn insert_stops_when_caught_up() {
    let (pool, schema) = test_pool().await;
    let existing = Issue {
        id: "I_old".into(),
        title: "old".into(),
        created_at: "2026-11-02T00:00:00Z".parse().unwrap(),
        url: "https://x".into(),
        labels: vec![],
        repository_name: "acme/app".into(),
        fork_count: 0,
        star_count: 0,
        primary_language: String::new(),
    };
    Issue::insert(&pool, &existing).await.unwrap();

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_page(
            vec![
                search_node("I_new", "2026-11-03T00:00:00Z", Value::Null),
                search_node("I_older", "2026-11-01T00:00:00Z", Value::Null),
            ],
            true,
            "c1",
        )))
        .expect(1)
        .mount(&server)
        .await;

    let report = routine(&pool, &server).insert_issues(500).await.unwrap();
    assert_eq!(report.stop_reason, StopReason::CaughtUp);
    assert_eq!(report.stored, 1);
    assert!(Issue::retrieve(&pool, "I_older").await.unwrap().is_none());
    drop_schema(pool, schema).await;
}

#[tokio::test]
async fn update_removes_closed_and_unresolvable_issues() {
    let (pool, schema) = test_pool().await;
    for id in ["I_open", "I_closed", "I_deleted"] {
        Issue::insert(
            &pool,
            &Issue {
                id: id.into(),
                title: id.into(),
                created_at: chrono::Utc::now(),
                url: "https://x".into(),
                labels: vec![],
                repository_name: "acme/app".into(),
                fork_count: 0,
                star_count: 0,
                primary_language: String::new(),
            },
        )
        .await
        .unwrap();
    }
    let ids: Vec<String> = Issue::retrieve_ids_least_recently_checked(&pool, 10).await.unwrap();
    let nodes: Vec<Value> = ids
        .iter()
        .map(|id| match id.as_str() {
            "I_open" => json!({"__typename": "Issue", "id": id, "state": "OPEN"}),
            "I_closed" => json!({"__typename": "Issue", "id": id, "state": "CLOSED"}),
            _ => Value::Null,
        })
        .collect();

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "rateLimit": { "limit": 5000, "cost": 1, "remaining": 4000, "resetAt": "2030-01-01T00:00:00Z" },
                "nodes": nodes
            },
            "errors": [{ "message": "Could not resolve to a node with the global id of 'I_deleted'" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let report = routine(&pool, &server).update_issues(10).await.unwrap();
    assert_eq!((report.checked, report.still_open, report.removed), (3, 1, 2));
    assert!(Issue::retrieve(&pool, "I_open").await.unwrap().is_some());
    assert!(Issue::retrieve(&pool, "I_closed").await.unwrap().is_none());
    drop_schema(pool, schema).await;
}

#[tokio::test]
async fn graphql_errors_without_data_are_reported() {
    let (pool, schema) = test_pool().await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "errors": [{ "message": "Something went wrong" }]
        })))
        .mount(&server)
        .await;
    let err = routine(&pool, &server).insert_issues(10).await.unwrap_err();
    assert!(err.to_string().contains("Something went wrong"), "{err}");

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"message": "Bad credentials"})))
        .mount(&server)
        .await;
    assert!(routine(&pool, &server).insert_issues(10).await.is_err());
    drop_schema(pool, schema).await;
}
