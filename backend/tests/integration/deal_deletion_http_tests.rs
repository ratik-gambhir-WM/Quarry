use crate::{
    adapters::{helix::client::HelixClient, sqlite::client::SqliteClient},
    app::{
        bootstrap::test_support::assemble_api, config::AppConfig, http::create_router, migrations,
    },
    integration_support::{prompt_catalog, seed_identity},
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
fn app() -> (Router, SqliteClient) {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrations::migrate(&sqlite).unwrap();
    seed_identity(&sqlite, "deletion-owner@example.com", 'a');
    seed_identity(&sqlite, "deletion-member@example.com", 'b');
    sqlite.with_connection(|c|c.execute_batch("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES('DEAL-DELETE-HTTP',1,'Synthetic','buy_side','unknown','pipeline','now','now'); INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES('DEAL-DELETE-HTTP',1,'owner','now','now'),('DEAL-DELETE-HTTP',2,'manager','now','now'); INSERT INTO deal_access_revisions(deal_id) VALUES('DEAL-DELETE-HTTP');")).unwrap();
    let config = AppConfig::default();
    let api = assemble_api(
        &config,
        sqlite.clone(),
        Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap()),
        reqwest::Client::new(),
        prompt_catalog(),
    );
    (create_router(api, &config.http), sqlite)
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    actor: Option<i64>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(actor) = actor {
        request = request
            .header(
                "authorization",
                format!("Bearer {}", if actor == 1 { "a" } else { "b" }.repeat(43)),
            )
            .header("x-user-id", actor.to_string());
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn deletion_http_checks_identity_owner_and_syntax_before_accepting_and_hides_pending_deals() {
    let (app, sqlite) = app();
    assert_eq!(
        request(&app, "DELETE", "/api/v1/deals/DEAL-DELETE-HTTP", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&app, "DELETE", "/api/v1/deals/DEAL-DELETE-HTTP", Some(2))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "DELETE", "/api/v1/deals/invalid%20deal", Some(1))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        sqlite
            .with_connection(
                |c| c.query_row("SELECT count(*) FROM deal_deletions", [], |r| r
                    .get::<_, i64>(0))
            )
            .unwrap(),
        0
    );
    let (status, headers, op) =
        request(&app, "DELETE", "/api/v1/deals/DEAL-DELETE-HTTP", Some(1)).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(headers["location"], op["statusUrl"].as_str().unwrap());
    let repeat = request(&app, "DELETE", "/api/deals/DEAL-DELETE-HTTP", Some(1)).await;
    assert_eq!(repeat.0, StatusCode::ACCEPTED);
    assert_eq!(repeat.2["deletionId"], op["deletionId"]);
    for actor in [1, 2] {
        assert_eq!(
            request(&app, "GET", "/api/v1/deals/DEAL-DELETE-HTTP", Some(actor))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/deals/DEAL-DELETE-HTTP/archive",
            Some(1)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&app, "GET", "/api/v1/deals", Some(1)).await.2,
        json!([])
    );
    let url = op["statusUrl"].as_str().unwrap();
    let owned = request(&app, "GET", url, Some(1)).await;
    assert_eq!(owned.0, StatusCode::OK);
    assert_eq!(owned.1["cache-control"], "no-store");
    assert_eq!(
        request(&app, "GET", url, Some(2)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, "GET", url, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&app, "GET", "/api/v1/deal-deletions/not-an-id", Some(1))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}
