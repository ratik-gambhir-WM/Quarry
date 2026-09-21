use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, Request, StatusCode},
    routing::get,
    Json, Router,
};
use docx_rust::{document::Paragraph, Docx};
use serde_json::Value;
use std::{io::Cursor, sync::Arc};
use tower::ServiceExt;

use crate::integration_support::test_application;

fn test_router() -> axum::Router {
    test_application().unwrap().router
}

#[tokio::test]
async fn assistant_threads_create_list_and_enforce_the_resolved_owner() {
    let application = test_application().unwrap();
    crate::domains::users::repository::UserRepository::new(application.sqlite.clone())
        .create(crate::domains::users::repository::AddUserInput {
            api_key: "development-key".to_string(),
            email: "analyst@example.com".to_string(),
            first_name: "Avery".to_string(),
            last_name: "Analyst".to_string(),
            role: "Analyst".to_string(),
        })
        .await
        .unwrap();
    let app = application.router;
    let create = app
        .clone()
        .oneshot(
            Request::post("/api/v1/assistant/threads")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"userEmail":"analyst@example.com","threadId":"thread-1"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);

    let list = app
        .clone()
        .oneshot(
            Request::get("/api/v1/assistant/threads?userEmail=analyst%40example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(list.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!(body["threads"][0]["threadId"], "thread-1");

    let other_owner = app
        .oneshot(
            Request::get("/api/v1/assistant/threads/thread-1?userEmail=someone%40example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(other_owner.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn health_route_is_available_under_api_prefix() {
    let app = test_router();

    let response = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn versioned_health_and_capabilities_routes_are_available() {
    let app = test_router();

    let health = app
        .clone()
        .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let capabilities = app
        .oneshot(
            Request::get("/api/v1/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(capabilities.status(), StatusCode::OK);
    assert!(health.headers().contains_key("x-request-id"));
    assert!(capabilities.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn query_model_is_available_under_both_prefixes_and_fails_before_stream_when_unconfigured() {
    let app = test_router();
    for path in ["/api/v1/query_model", "/api/query_model"] {
        let (content_type, body) = query_multipart("hello", "[]");
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

#[tokio::test]
async fn query_model_rejects_invalid_context_before_capability_lookup() {
    let app = test_router();
    for context in [
        r#"[{"role":"user","content":"unpaired"}]"#,
        r#"[{"role":"assistant","content":"wrong"},{"role":"user","content":"order"}]"#,
        r#"[{"role":"user","content":"question","extra":true},{"role":"assistant","content":"answer"}]"#,
    ] {
        let (content_type, body) = query_multipart("hello", context);
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/query_model")
                    .header("content-type", content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn query_model_streams_the_typed_lifecycle_without_buffering() {
    async fn upstream(Json(body): Json<Value>) -> axum::response::Response {
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][1]["role"], "assistant");
        assert_eq!(body["input"][2]["content"][0]["text"], "next");
        assert_eq!(body["store"], false);
        axum::response::Response::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from(
                "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"complete\"}}\n\n",
            ))
            .unwrap()
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream_task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/v1/responses", axum::routing::post(upstream)),
        )
        .await
        .unwrap();
    });
    let client = crate::adapters::openai::client::OpenAiClient::with_responses_url(
        reqwest::Client::new(),
        "test-key",
        format!("http://{address}/v1/responses"),
    );
    let service = crate::domains::assistant::chat::service::AssistantChatService::new(
        Some(Arc::new(client)),
        "gpt-5.5".to_string(),
    );
    let api = crate::domains::assistant::chat::route::routes(Arc::new(service));
    let app = crate::app::http::create_router(api, &crate::AppConfig::default().http);
    let context = r#"[{"role":"user","content":"first"},{"role":"assistant","content":"answer"}]"#;
    let (content_type, body) = query_multipart("next", context);
    let response = app
        .oneshot(
            Request::post("/api/v1/query_model")
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let body = String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("event: started"));
    assert!(body.contains("event: delta"));
    assert!(body.contains("\"response\":\"complete\""));
    upstream_task.abort();
}

#[tokio::test]
async fn routes_outside_api_prefix_are_not_found() {
    let app = test_router();

    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn template_preview_route_validates_page_and_degrades_when_unconfigured() {
    let app = test_router();
    let unavailable = app
        .clone()
        .oneshot(
            Request::get("/api/v1/templates/previews")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    for query in ["page=0", "page=-1", "page=invalid", "page=101"] {
        let invalid = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/templates/previews?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn template_delete_route_degrades_when_unconfigured() {
    let response = test_router()
        .oneshot(
            Request::delete("/api/v1/templates/example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn template_document_route_validates_ids_and_degrades_when_unconfigured() {
    let app = test_router();
    let unavailable = app
        .clone()
        .oneshot(
            Request::get("/api/v1/templates/example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);

    let invalid = app
        .oneshot(
            Request::get("/api/v1/templates/%20%20%20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn template_document_route_relays_valid_json_with_private_no_store() {
    async fn upstream(headers: HeaderMap) -> Json<Value> {
        assert_eq!(
            headers
                .get("X-App-Id")
                .and_then(|value| value.to_str().ok()),
            Some("Quarry_WestMonroe")
        );
        Json(serde_json::json!({
            "presentation": {
                "preserveElementOrder": true,
                "showBranding": false,
                "slides": [],
                "title": "Integration template",
                "unknownField": { "preserved": true }
            }
        }))
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream_task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/api/v1/templates/example", get(upstream)),
        )
        .await
        .unwrap();
    });
    let client = crate::adapters::diligence_studio::client::DiligenceStudioClient::new(
        reqwest::Client::new(),
        reqwest::Url::parse(&format!("http://{address}/api/v1/")).unwrap(),
    );
    let api = crate::domains::templates::route::routes(Arc::new(
        crate::domains::templates::service::TemplateService::new(Some(Arc::new(client))),
    ));
    let app = crate::app::http::create_router(api, &crate::AppConfig::default().http);

    let response = app
        .oneshot(
            Request::get("/api/v1/templates/example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["presentation"]["unknownField"]["preserved"], true);
    upstream_task.abort();
}

#[tokio::test]
async fn template_export_route_validates_json_before_capability_lookup() {
    let app = test_router();
    let invalid = app
        .clone()
        .oneshot(
            Request::post("/api/v1/templates/export")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"notPresentation":{}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let unavailable = app
        .oneshot(
            Request::post("/api/v1/templates/export")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"presentation":{}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn template_export_route_relays_bounded_powerpoint_downloads() {
    async fn upstream(headers: HeaderMap, Json(body): Json<Value>) -> axum::response::Response {
        assert_eq!(headers["X-App-Id"], "Quarry_WestMonroe");
        assert_eq!(body["presentation"]["title"], "Integration export");
        axum::response::Response::builder()
            .header(
                "content-type",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            )
            .header(
                "content-disposition",
                "attachment; filename=\"Integration export.pptx\"",
            )
            .header("x-powerpoint-warning-count", "1")
            .body(Body::from(b"PK\x03\x04powerpoint".as_slice()))
            .unwrap()
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream_task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/api/v1/export", axum::routing::post(upstream)),
        )
        .await
        .unwrap();
    });
    let client = crate::adapters::diligence_studio::client::DiligenceStudioClient::new(
        reqwest::Client::new(),
        reqwest::Url::parse(&format!("http://{address}/api/v1/")).unwrap(),
    );
    let api = crate::domains::templates::route::routes(Arc::new(
        crate::domains::templates::service::TemplateService::new(Some(Arc::new(client))),
    ));
    let app = crate::app::http::create_router(api, &crate::AppConfig::default().http);

    let response = app
        .oneshot(
            Request::post("/api/v1/templates/export")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"presentation":{"title":"Integration export"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert_eq!(response.headers()["x-powerpoint-warning-count"], "1");
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"Integration export.pptx\""
    );
    assert!(to_bytes(response.into_body(), 1024)
        .await
        .unwrap()
        .starts_with(b"PK"));
    upstream_task.abort();
}

#[tokio::test]
async fn template_import_route_validates_mode_and_upload_before_capability_lookup() {
    let app = test_router();
    for path in [
        "/api/v1/templates/import?mode=single",
        "/api/templates/import?mode=batch",
    ] {
        let (content_type, body) = template_multipart(&[("files", "template.pptx", b"pptx")]);
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    for path in [
        "/api/v1/templates/import",
        "/api/v1/templates/import?mode=unsupported",
        "/api/v1/templates/import?mode=single&mode=batch",
    ] {
        let (content_type, body) = template_multipart(&[("files", "template.pptx", b"pptx")]);
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    for parts in [
        vec![],
        vec![("files", "template.txt", b"pptx".as_slice())],
        vec![("files", "../template.pptx", b"pptx".as_slice())],
        vec![("files", "template.pptx", b"".as_slice())],
        vec![
            ("files", "one.pptx", b"one".as_slice()),
            ("files", "two.pptx", b"two".as_slice()),
        ],
        vec![("unexpected", "template.pptx", b"pptx".as_slice())],
    ] {
        let (content_type, body) = template_multipart(&parts);
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/templates/import?mode=single")
                    .header("content-type", content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn template_import_route_accepts_a_file_above_the_default_body_limit() {
    let bytes = vec![1; 2 * 1024 * 1024 + 1];
    let (content_type, body) = template_multipart(&[("files", "template.pptx", &bytes)]);
    let response = test_router()
        .oneshot(
            Request::post("/api/v1/templates/import?mode=batch")
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

fn template_multipart(parts: &[(&str, &str, &[u8])]) -> (String, Vec<u8>) {
    const BOUNDARY: &str = "quarry-template-boundary";
    let mut body = Vec::new();
    for (field_name, filename, bytes) in parts {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={BOUNDARY}"), body)
}

fn query_multipart(prompt: &str, context: &str) -> (String, Vec<u8>) {
    const BOUNDARY: &str = "quarry-query-boundary";
    let body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\n{prompt}\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"context\"\r\n\r\n{context}\r\n--{BOUNDARY}--\r\n"
    );
    (
        format!("multipart/form-data; boundary={BOUNDARY}"),
        body.into_bytes(),
    )
}

#[tokio::test]
async fn legacy_command_routes_are_not_exposed() {
    let app = test_router();

    let response = app
        .oneshot(
            Request::post("/api/commands/greet")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Quarry"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sqlite_user_can_be_saved_and_fetched_by_email() {
    let app = test_router();
    let create_response = app
        .clone()
        .oneshot(
            Request::post("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                            "firstName":"Ada",
                            "lastName":"Lovelace",
                            "email":"ada@example.com",
                            "apiKey":"test-key",
                            "role":"Analyst"
                        }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(created["email"], "ada@example.com");
    assert_eq!(created["firstName"], "Ada");

    let get_response = app
        .oneshot(
            Request::get("/api/users/by-email?email=ada%40example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let fetched: Value = serde_json::from_slice(
        &to_bytes(get_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(fetched["id"], created["id"]);
    assert_eq!(fetched["email"], created["email"]);
}

#[tokio::test]
async fn sqlite_user_lookup_returns_not_found_for_unknown_email() {
    let app = test_router();
    let response = app
        .oneshot(
            Request::get("/api/users/by-email?email=missing%40example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sqlite_user_handlers_reject_blank_required_fields() {
    let app = test_router();
    let create_response = app
        .clone()
        .oneshot(
            Request::post("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "firstName":" ","lastName":"Analyst",
                        "email":"analyst@example.com","apiKey":"test-key","role":"Analyst"
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::BAD_REQUEST);

    let lookup_response = app
        .oneshot(
            Request::get("/api/users/by-email?email=%20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(lookup_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deal_flow_saves_core_fields_then_optional_metadata() {
    const BOUNDARY: &str = "quarry-empty-deal-metadata";
    let app = test_router();
    let create_user = app
        .clone()
        .oneshot(
            Request::post("/api/v1/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "firstName":"Avery","lastName":"Analyst",
                        "email":"analyst@example.com","apiKey":"test-key","role":"Analyst"
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_user.status(), StatusCode::CREATED);
    let created_user: Value =
        serde_json::from_slice(&to_bytes(create_user.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    let user_id = created_user["id"].as_i64().unwrap();

    let create_deal = app
        .clone()
        .oneshot(
            Request::post("/api/v1/deals")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "dealId":"DEAL-000184",
                        "dealName":"Acme acquisition of WidgetCo",
                        "status":"Active",
                        "startDate":"2026-02-14",
                        "closeDate":"2026-05-01",
                        "transactionType":"Acquisition",
                        "targetCompany":"WidgetCo",
                        "primaryBuyer":"CVS",
                        "dealSponsor":"Thoma Bravo",
                        "userEmail":"analyst@example.com",
                        "localPath":null,
                        "sharepointLink":null
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_deal.status(), StatusCode::CREATED);
    let created_deal: Value =
        serde_json::from_slice(&to_bytes(create_deal.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(created_deal["deal"]["userId"], user_id);
    assert_eq!(created_deal["metadata"]["userId"], user_id);

    let get_deal = app
        .clone()
        .oneshot(
            Request::get("/api/v1/deals/DEAL-000184")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_deal.status(), StatusCode::OK);
    let fetched_deal: Value =
        serde_json::from_slice(&to_bytes(get_deal.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(fetched_deal["userId"], user_id);

    let list_deals = app
        .clone()
        .oneshot(Request::get("/api/v1/deals").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(list_deals.status(), StatusCode::OK);
    let listed_deals: Value =
        serde_json::from_slice(&to_bytes(list_deals.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(listed_deals[0]["userId"], user_id);

    let empty_data_room = app
        .clone()
        .oneshot(
            Request::get("/api/v1/deals/DEAL-000184/data-room")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(empty_data_room.status(), StatusCode::OK);
    let empty_data_room: Value = serde_json::from_slice(
        &to_bytes(empty_data_room.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(empty_data_room["dealId"], "DEAL-000184");
    assert_eq!(empty_data_room["tree"], serde_json::json!([]));

    let save_metadata = app
        .clone()
        .oneshot(
            Request::post("/api/v1/deals/DEAL-000184/metadata")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(format!(
                    "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"sharepointLink\"\r\n\r\n https://northwind.sharepoint.com/sites/acme \r\n\
                     --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"sowLink\"\r\n\r\n https://example.com/sow \r\n\
                     --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"factSheetLink\"\r\n\r\nhttps://example.com/fact-sheet\r\n\
                     --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"rlLink\"\r\n\r\nhttps://example.com/request-list\r\n\
                     --{BOUNDARY}--\r\n"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save_metadata.status(), StatusCode::OK);
    let saved: Value = serde_json::from_slice(
        &to_bytes(save_metadata.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["deal"]["dealId"], "DEAL-000184");
    assert_eq!(saved["deal"]["userId"], user_id);
    assert_eq!(saved["metadata"]["userId"], user_id);
    assert_eq!(saved["metadata"]["keyQuestionsJson"], "[]");
    assert_eq!(
        saved["metadata"]["sharepointLink"],
        "https://northwind.sharepoint.com/sites/acme"
    );
    assert_eq!(saved["metadata"]["sowLink"], "https://example.com/sow");
    assert_eq!(
        saved["metadata"]["factSheetLink"],
        "https://example.com/fact-sheet"
    );
    assert_eq!(
        saved["metadata"]["rlLink"],
        "https://example.com/request-list"
    );
    assert_eq!(saved["files"], serde_json::json!([]));

    let persisted_metadata = app
        .clone()
        .oneshot(
            Request::get("/api/v1/deals/DEAL-000184")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(persisted_metadata.status(), StatusCode::OK);
    let persisted_metadata: Value = serde_json::from_slice(
        &to_bytes(persisted_metadata.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        persisted_metadata["metadata"]["sharepointLink"],
        "https://northwind.sharepoint.com/sites/acme"
    );
    assert_eq!(
        persisted_metadata["metadata"]["sowLink"],
        "https://example.com/sow"
    );
    assert_eq!(
        persisted_metadata["metadata"]["factSheetLink"],
        "https://example.com/fact-sheet"
    );
    assert_eq!(
        persisted_metadata["metadata"]["rlLink"],
        "https://example.com/request-list"
    );

    let invalid_metadata = app
        .clone()
        .oneshot(
            Request::post("/api/v1/deals/DEAL-000184/metadata")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(format!(
                    "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"sharepointUrl\"\r\n\r\nhttps://northwind.sharepoint.com/sites/acme\r\n\
                     --{BOUNDARY}--\r\n"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_metadata.status(), StatusCode::BAD_REQUEST);

    let archive_deal = app
        .oneshot(
            Request::post("/api/v1/deals/DEAL-000184/archive")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(archive_deal.status(), StatusCode::OK);
    let archived_deal: Value = serde_json::from_slice(
        &to_bytes(archive_deal.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(archived_deal["userId"], user_id);
}

#[tokio::test]
async fn redundant_user_exists_route_is_not_exposed() {
    let app = test_router();
    let response = app
        .oneshot(
            Request::get("/api/users/exists?email=ada%40example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn helix_user_routes_are_not_exposed() {
    let app = test_router();
    let get_response = app
        .clone()
        .oneshot(
            Request::get("/api/users/helix/by-email?email=ada%40example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let create_response = app
        .oneshot(
            Request::post("/api/users/helix")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(create_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn document_search_validates_input_before_calling_helix() {
    let app = test_router();

    let response = app
        .oneshot(
            Request::post("/api/documents/search/keyword")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"workspaceId":"user-1","queryText":"","limit":10}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn process_file_accepts_multipart_bodies_above_axums_default_limit() {
    const BOUNDARY: &str = "quarry-large-upload-boundary";
    const FILE_BYTES: usize = 2 * 1024 * 1024 + 1;

    let mut multipart = Vec::with_capacity(FILE_BYTES + 512);
    multipart.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"userId\"\r\n\r\nuser-1\r\n"
        )
        .as_bytes(),
    );
    multipart.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"large.pdf\"\r\nContent-Type: application/pdf\r\n\r\n"
            )
            .as_bytes(),
        );
    multipart.resize(multipart.len() + FILE_BYTES, b'x');
    multipart.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

    let app = test_router();
    let response = app
        .oneshot(
            Request::post("/api/deals/DEAL-LARGE/documents/process_file")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(multipart))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn process_file_rejects_a_competing_multipart_deal_id() {
    const BOUNDARY: &str = "quarry-deal-id-boundary";
    let multipart = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"dealId\"\r\n\r\nDEAL-OTHER\r\n--{BOUNDARY}--\r\n"
    );
    let app = test_router();

    let response = app
        .oneshot(
            Request::post("/api/deals/DEAL-PATH/documents/process_file")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(multipart))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn process_file_rejects_unsupported_uploads_at_the_handler_boundary() {
    const BOUNDARY: &str = "quarry-invalid-document-extension";
    let multipart = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"userId\"\r\n\r\nuser-1\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"notes.txt\"\r\nContent-Type: text/plain\r\n\r\nnotes\r\n--{BOUNDARY}--\r\n"
    );
    let app = test_router();

    let response = app
        .oneshot(
            Request::post("/api/deals/DEAL-PATH/documents/process_file")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(multipart))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn process_file_rejects_blank_user_id_at_the_handler_boundary() {
    const BOUNDARY: &str = "quarry-blank-document-user";
    let multipart = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"userId\"\r\n\r\n   \r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"report.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.4\r\n--{BOUNDARY}--\r\n"
    );
    let app = test_router();

    let response = app
        .oneshot(
            Request::post("/api/deals/DEAL-PATH/documents/process_file")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(multipart))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn stored_document_routes_list_files_and_return_pdf_and_raw_text() {
    let mut docx = Docx::default();
    docx.document
        .push(Paragraph::default().push_text("Raw route text from DOCX."));
    let docx_bytes = docx.write(Cursor::new(Vec::new())).unwrap().into_inner();
    let application = test_application().unwrap();
    application
        .sqlite
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO users (first_name, last_name, email, api_key, role) VALUES ('Avery', 'Analyst', 'analyst@example.com', 'key', 'Analyst')",
                [],
            )?;
            let user_id = connection.last_insert_rowid();
            for deal_id in ["DEAL-DOCUMENTS", "DEAL-OTHER"] {
                connection.execute(
                    r#"
                    INSERT INTO deals (
                        deal_id, user_id, deal_name, status, start_date, close_date,
                        transaction_type, target_company, primary_buyer, deal_sponsor
                    ) VALUES (?1, ?2, 'Project Test', 'Active', '2026-01-01', '2026-02-01',
                              'Buy-side', 'Target', 'Buyer', 'Test Capital')
                    "#,
                    rusqlite::params![deal_id, user_id],
                )?;
            }
            insert_stored_document(
                connection,
                "DEAL-DOCUMENTS",
                "file-zulu",
                "version-zulu",
                "Zulu.pdf",
                "application/pdf",
                b"%PDF-1.4\nzulu",
            )?;
            insert_stored_document(
                connection,
                "DEAL-DOCUMENTS",
                "file-alpha",
                "version-alpha",
                "Alpha.pdf",
                "application/pdf",
                b"%PDF-1.4\nalpha",
            )?;
            insert_stored_document(
                connection,
                "DEAL-DOCUMENTS",
                "file-raw",
                "version-raw",
                "Raw.docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                &docx_bytes,
            )?;
            insert_stored_document(
                connection,
                "DEAL-OTHER",
                "file-other",
                "version-other",
                "Other.pdf",
                "application/pdf",
                b"%PDF-1.4\nother",
            )?;
            Ok(())
        })
        .unwrap();
    let app = application.router;

    let list_response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/deals/DEAL-DOCUMENTS/documents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed: Value = serde_json::from_slice(
        &to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        listed,
        serde_json::json!([
            {"fileId": "file-alpha", "displayName": "Alpha.pdf"},
            {"fileId": "file-raw", "displayName": "Raw.docx"},
            {"fileId": "file-zulu", "displayName": "Zulu.pdf"}
        ])
    );

    let pdf_response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/deals/DEAL-DOCUMENTS/documents/file-alpha/pdf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pdf_response.status(), StatusCode::OK);
    assert_eq!(
        pdf_response.headers().get("content-type").unwrap(),
        "application/pdf"
    );
    assert_eq!(
        pdf_response.headers().get("content-disposition").unwrap(),
        "inline"
    );
    assert_eq!(
        to_bytes(pdf_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .as_ref(),
        b"%PDF-1.4\nalpha"
    );

    let text_response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/deals/DEAL-DOCUMENTS/documents/file-raw/text")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(text_response.status(), StatusCode::OK);
    let raw_text: Value = serde_json::from_slice(
        &to_bytes(text_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(raw_text["fileName"], "Raw.docx");
    assert_eq!(raw_text["sourceKind"], "docx");
    assert_eq!(raw_text["text"], "Raw route text from DOCX.");

    let cross_deal_response = app
        .oneshot(
            Request::get("/api/v1/deals/DEAL-DOCUMENTS/documents/file-other/pdf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_deal_response.status(), StatusCode::NOT_FOUND);
}

fn insert_stored_document(
    connection: &rusqlite::Connection,
    deal_id: &str,
    file_id: &str,
    version_id: &str,
    display_name: &str,
    mime_type: &str,
    bytes: &[u8],
) -> rusqlite::Result<()> {
    connection.execute(
        r#"
        INSERT INTO quarry_files (
            file_id, deal_id, workspace_id, display_name, metadata_json, created_at, updated_at
        ) VALUES (?1, ?2, 'analyst@example.com', ?3, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')
        "#,
        rusqlite::params![file_id, deal_id, display_name],
    )?;
    connection.execute(
        r#"
        INSERT INTO quarry_file_versions (
            version_id, file_id, version_number, original_filename, mime_type,
            content_sha256, byte_size, is_current, created_at
        ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, 1, '2026-01-01T00:00:00Z')
        "#,
        rusqlite::params![
            version_id,
            file_id,
            display_name,
            mime_type,
            "a".repeat(64),
            bytes.len() as i64
        ],
    )?;
    connection.execute(
        "INSERT INTO quarry_file_blobs (version_id, file_bytes) VALUES (?1, ?2)",
        rusqlite::params![version_id, bytes],
    )?;
    Ok(())
}
