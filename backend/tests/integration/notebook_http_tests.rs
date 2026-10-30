use crate::domains::notebooks::test_support::{self, App};
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn call(
    app: &App,
    method: &str,
    path: &str,
    body: Option<Value>,
    actor: i64,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let token = if actor == 1 { "a" } else { "b" }.repeat(43);
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("x-user-id", actor.to_string());
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = app
        .router
        .clone()
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 3 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        headers,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}
async fn book(app: &App) -> String {
    let (status, headers, b) = call(
        app,
        "POST",
        "/api/v1/notebooks",
        Some(json!({"title":"Research"})),
        1,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(headers["location"]
        .to_str()
        .unwrap()
        .starts_with("/api/v1/notebooks/"));
    assert!(b["dealId"].is_null());
    b["notebookId"].as_str().unwrap().to_owned()
}
async fn save(app: &App, path: &str, method: &str, body: Value) -> Value {
    let (status, headers, job) = call(app, method, path, Some(body), 1).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    assert_eq!(headers["location"], job["statusUrl"].as_str().unwrap());
    let terminal = test_support::terminal(
        &app.service,
        job["notebookId"].as_str().unwrap(),
        job["jobId"].as_str().unwrap(),
    )
    .await;
    assert_eq!(terminal.status, "completed");
    job
}
#[tokio::test]
async fn notebook_http_complete_resource_lifecycle_and_terminal_sse() {
    let app = test_support::application();
    let id = book(&app).await;
    let base = format!("/api/v1/notebooks/{id}");
    let (status, _, folder) = call(
        &app,
        "POST",
        &format!("{base}/folders"),
        Some(json!({"name":"Interviews"})),
        1,
    )
    .await;
    assert_eq!(status, 201);
    let fid = folder["folderId"].as_str().unwrap();
    assert_eq!(
        call(&app, "GET", &format!("{base}/folders/{fid}"), None, 1)
            .await
            .0,
        200
    );
    assert_eq!(
        call(
            &app,
            "PATCH",
            &format!("{base}/folders/{fid}"),
            Some(json!({"name":"Site visit"})),
            1
        )
        .await
        .2["name"],
        "Site visit"
    );
    let job=save(&app,&format!("{base}/notes"),"POST",json!({"operationId":uuid::Uuid::new_v4().to_string(),"folderId":fid,"title":"Interview","blocks":test_support::blocks(),"contentSchemaVersion":1})).await;
    let nid = job["noteId"].as_str().unwrap();
    let path = format!("{base}/notes/{nid}");
    let note = call(&app, "GET", &path, None, 1).await.2;
    assert_eq!(note["blocks"], test_support::blocks());
    assert_eq!(note["revision"], 1);
    let moved=save(&app,&path,"PATCH",json!({"operationId":uuid::Uuid::new_v4().to_string(),"expectedRevision":1,"folderId":null})).await;
    let note = call(&app, "GET", &path, None, 1).await.2;
    assert!(note["folderId"].is_null());
    assert_eq!(note["revision"], 2);
    assert_eq!(note["blocks"], test_support::blocks());
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("{base}/notes?folderId=none&limit=1"),
            None,
            1
        )
        .await
        .2["notes"][0]["noteId"],
        nid
    );
    let event_path = format!("{}/events", moved["statusUrl"].as_str().unwrap());
    let response = app
        .router
        .clone()
        .oneshot(
            Request::get(event_path)
                .header("authorization", format!("Bearer {}", "a".repeat(43)))
                .header("x-user-id", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        to_bytes(response.into_body(), 65536),
    )
    .await
    .unwrap()
    .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("event: completed"));
    assert!(text.contains("id: "));
    assert!(!text.contains("Native café"));
    assert_eq!(
        call(&app, "DELETE", &format!("{base}/folders/{fid}"), None, 1)
            .await
            .0,
        204
    );
    assert_eq!(call(&app, "DELETE", &path, None, 1).await.0, 204);
    assert_eq!(call(&app, "GET", &path, None, 1).await.0, 404);
    let deleted = call(&app, "DELETE", &base, None, 1).await;
    assert_eq!(deleted.0, 204);
    assert!(deleted.2.is_null());
    assert_eq!(call(&app, "DELETE", &base, None, 1).await.0, 404);
}
#[tokio::test]
async fn notebook_http_ownership_authentication_validation_and_compatibility() {
    let app = test_support::application();
    let id = book(&app).await;
    let base = format!("/api/v1/notebooks/{id}");
    for (method, path, body) in [
        ("GET", base.clone(), None),
        ("PATCH", base.clone(), Some(json!({"title":"Attack"}))),
        ("DELETE", base.clone(), None),
        (
            "POST",
            format!("{base}/folders"),
            Some(json!({"name":"Attack"})),
        ),
        ("GET", format!("{base}/notes"), None),
    ] {
        assert_eq!(call(&app, method, &path, body, 2).await.0, 404);
    }
    assert_eq!(
        call(&app, "GET", &format!("/api/notebooks/{id}"), None, 1)
            .await
            .0,
        200
    );
    for query in ["limit=0", "offset=-1", "limit=1&limit=2", "unknown=x"] {
        assert_eq!(
            call(&app, "GET", &format!("/api/v1/notebooks?{query}"), None, 1)
                .await
                .0,
            400
        );
    }
    for body in [
        json!({}),
        json!({"title":null}),
        json!({"userId":2}),
        json!({"dealId":""}),
    ] {
        assert_eq!(call(&app, "PATCH", &base, Some(body), 1).await.0, 400);
    }
    assert_eq!(
        call(&app, "GET", "/api/v1/notebooks/not-a-uuid", None, 1)
            .await
            .0,
        400
    );
    let response = app
        .router
        .clone()
        .oneshot(Request::get(&base).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(response.headers()["www-authenticate"], "Bearer");
    let response = app
        .router
        .clone()
        .oneshot(
            Request::get(&base)
                .header("authorization", format!("Bearer {}", "a".repeat(43)))
                .header("x-user-id", "2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}
#[tokio::test]
async fn notebook_http_media_type_streamed_body_limit_and_patch_preflight() {
    let app = test_support::application();
    for (content_type, body, expected) in [
        (None, "{}".to_owned(), 415),
        (Some("application/json"), "{".into(), 400),
        (
            Some("application/json"),
            format!("{{\"title\":\"{}\"}}", "x".repeat(2 * 1024 * 1024)),
            413,
        ),
    ] {
        let mut request = Request::post("/api/v1/notebooks")
            .header("authorization", format!("Bearer {}", "a".repeat(43)))
            .header("x-user-id", "1");
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        let chunks = futures_util::stream::iter([Ok::<_, std::io::Error>(body)]);
        let response = app
            .router
            .clone()
            .oneshot(request.body(Body::from_stream(chunks)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        assert!(serde_json::from_slice::<Value>(&bytes).unwrap()["error"].is_string());
    }
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/v1/notebooks")
                .header("origin", "http://localhost:1420")
                .header("access-control-request-method", "PATCH")
                .header(
                    "access-control-request-headers",
                    "authorization,x-user-id,content-type",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success());
    assert!(response.headers()["access-control-allow-methods"]
        .to_str()
        .unwrap()
        .contains("PATCH"));
}
#[tokio::test]
async fn notebook_http_status_and_events_require_owned_parent_and_revoked_sessions_close() {
    use futures_util::StreamExt;
    let app = test_support::application();
    app.pipeline
        .blocked
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let id = book(&app).await;
    let job = call(
        &app,
        "POST",
        &format!("/api/v1/notebooks/{id}/notes"),
        Some(json!({"operationId":uuid::Uuid::new_v4().to_string(),"title":"Blocked"})),
        1,
    )
    .await
    .2;
    let status = job["statusUrl"].as_str().unwrap();
    assert_eq!(call(&app, "GET", status, None, 2).await.0, 404);
    let response = app
        .router
        .clone()
        .oneshot(
            Request::get(format!("{status}/events"))
                .header("authorization", format!("Bearer {}", "a".repeat(43)))
                .header("x-user-id", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut stream = response.into_body().into_data_stream();
    assert!(stream.next().await.is_some());
    app.sqlite
        .with_connection(|c| c.execute("UPDATE auth_sessions SET revoked_at=1 WHERE user_id=1", []))
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while stream.next().await.is_some() {}
    })
    .await
    .unwrap();
    assert_eq!(call(&app, "GET", status, None, 1).await.0, 401);
}

/// Explicit opt-in visual fixture: in-memory SQLite, fake embeddings/Helix, loopback only.
#[tokio::test]
#[ignore = "interactive Computer visual fixture; listens on 127.0.0.1:39001 until stopped"]
async fn notebook_visual_fixture() {
    let app = test_support::application();
    let (status,_,_)=call(&app,"POST","/api/v1/auth/register",Some(json!({"username":"notebook-visual","password":"Visual-test-only-2026!","email":"notebook.visual.test@westmonroe.com","firstName":"Visual","lastName":"Test"})),1).await;
    assert_eq!(status, 201);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:39001")
        .await
        .unwrap();
    println!("Disposable notebook visual fixture listening on 127.0.0.1:39001");
    axum::serve(listener, app.router).await.unwrap();
}

#[tokio::test]
async fn notebook_http_queue_capacity_returns_retry_after_without_losing_admitted_work() {
    let app = test_support::application();
    app.pipeline
        .blocked
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let id = book(&app).await;
    let path = format!("/api/v1/notebooks/{id}/notes");
    for _ in 0..32 {
        assert_eq!(
            call(
                &app,
                "POST",
                &path,
                Some(json!({"operationId":uuid::Uuid::new_v4().to_string(),"title":"Queued"})),
                1
            )
            .await
            .0,
            202
        );
    }
    let (status, headers, error) = call(
        &app,
        "POST",
        &path,
        Some(json!({"operationId":uuid::Uuid::new_v4().to_string()})),
        1,
    )
    .await;
    assert_eq!(status, 503);
    assert_eq!(headers["retry-after"], "1");
    assert_eq!(error, json!({"error":"save queue capacity exhausted"}));
}
