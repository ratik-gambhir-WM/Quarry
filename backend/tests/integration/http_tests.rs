use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Json, Router,
};
use docx_rust::{document::Paragraph, Docx};
use serde_json::Value;
use std::{io::Cursor, sync::Arc};
use tower::ServiceExt;

use crate::integration_support::authenticated_test_application as test_application;

async fn test_router() -> axum::Router {
    test_application().await.unwrap().router
}

#[tokio::test]
async fn assistant_threads_create_list_and_enforce_the_resolved_owner() {
    let application = test_application().await.unwrap();
    let app = application.router;
    let create = app
        .clone()
        .oneshot(
            Request::post("/api/v1/assistant/threads")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"threadId":"thread-1"}"#))
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
        .clone()
        .oneshot(
            Request::get("/api/v1/assistant/threads/thread-1")
                .header("authorization", format!("Bearer {}", "b".repeat(43)))
                .header("x-user-id", "2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(other_owner.status(), StatusCode::NOT_FOUND);

    let deleted = app
        .clone()
        .oneshot(
            Request::delete("/api/v1/assistant/threads/thread-1?userEmail=analyst%40example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let missing = app
        .oneshot(
            Request::get("/api/v1/assistant/threads/thread-1?userEmail=analyst%40example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn handlers_reject_transport_validation_failures_before_service_calls() {
    let app = test_router().await;

    let invalid_deal = app
        .clone()
        .oneshot(
            Request::post("/api/v1/deals")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"dealName":"Acme","dealCode":null,"status":"pipeline","engagementType":"buy_side","startDate":"2026-05-01","expectedCloseDate":null,"actualCloseDate":null,"engagementCompletedDate":"2026-01-01","transactionType":"unknown","parties":[],"scopes":[],"userEmail":"analyst@example.com"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_deal.status(), StatusCode::BAD_REQUEST);

    let invalid_assistant_owner = app
        .clone()
        .oneshot(
            Request::get("/api/v1/assistant/threads?userEmail=%20%20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_assistant_owner.status(), StatusCode::OK);

    let invalid_document_id = app
        .oneshot(
            Request::get("/api/v1/deals/%20/documents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_document_id.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn health_route_is_available_under_api_prefix() {
    let app = test_router().await;

    let response = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn versioned_health_and_capabilities_routes_are_available() {
    let app = test_router().await;

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
async fn temporary_chat_routes_are_removed() {
    let app = test_router().await;
    for path in ["/api/v1/query_model", "/api/query_model"] {
        let response = app
            .clone()
            .oneshot(Request::post(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn saved_chat_fails_explicitly_when_provider_is_unconfigured() {
    let app = test_router().await;
    let create = app
        .clone()
        .oneshot(
            Request::post("/api/v1/assistant/threads")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"threadId":"thread-1"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let (content_type, body) = thread_run_multipart("hello", None, None);
    let response = app
        .oneshot(
            Request::post("/api/v1/assistant/threads/thread-1/runs")
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn saved_chat_rejects_client_context_before_starting_a_run() {
    let app = test_router().await;
    for context in [
        r#"[{"role":"user","content":"unpaired"}]"#,
        r#"[{"role":"assistant","content":"wrong"},{"role":"user","content":"order"}]"#,
        r#"[{"role":"user","content":"question","extra":true},{"role":"assistant","content":"answer"}]"#,
    ] {
        let (content_type, body) = thread_run_multipart("hello", Some(context), None);
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/assistant/threads/thread-1/runs")
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
async fn saved_chat_loads_thread_history_streams_and_persists_the_response() {
    async fn upstream(Json(body): Json<Value>) -> axum::response::Response {
        assert_eq!(body["input"].as_array().unwrap().len(), 3);
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"][0]["text"], "first");
        assert_eq!(body["input"][1]["role"], "assistant");
        assert_eq!(body["input"][1]["content"][0]["text"], "answer");
        assert_eq!(body["input"][2]["content"][0]["text"], "next");
        assert_eq!(body["store"], false);
        axum::response::Response::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from(
                "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"complete\"}]}]}}\n\n",
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
        "test-key",
        format!("http://{address}/v1/responses"),
        crate::integration_support::prompt_catalog(),
    );
    use crate::{
        adapters::postgres::client::PostgresClient,
        domains::{
            assistant::chat::repository::{AssistantChatRepository, StartRunInput},
            users::{
                repository::{test_support::AddUserInput, UserRepository},
                UserDirectory,
            },
        },
    };
    let postgres = PostgresClient::test_database().await.unwrap();
    crate::app::migrations::migrate(&postgres).await.unwrap();
    let users = UserRepository::new(postgres.clone());
    let user = users
        .create(AddUserInput {
            first_name: "Test".into(),
            last_name: "User".into(),
            email: "analyst@example.com".into(),
            role: "Analyst".into(),
        })
        .await
        .unwrap();
    let repository = AssistantChatRepository::new(postgres);
    repository
        .create_thread(user.id, "thread-1".into(), None)
        .await
        .unwrap();
    repository
        .start_run(StartRunInput {
            assistant_message_id: "a1".into(),
            user_message_id: "u1".into(),
            model: "test".into(),
            parent_message_id: None,
            prompt: "first".into(),
            request_id: "r1".into(),
            thread_id: "thread-1".into(),
            user_id: user.id,
        })
        .await
        .unwrap();
    repository
        .finish_run("a1".into(), "completed", "answer".into(), None)
        .await
        .unwrap();
    repository
        .create_thread(user.id, "other-thread".into(), None)
        .await
        .unwrap();
    repository
        .start_run(StartRunInput {
            assistant_message_id: "other-a".into(),
            user_message_id: "other-u".into(),
            model: "test".into(),
            parent_message_id: None,
            prompt: "Different thread question".into(),
            request_id: "other-r".into(),
            thread_id: "other-thread".into(),
            user_id: user.id,
        })
        .await
        .unwrap();
    repository
        .finish_run(
            "other-a".into(),
            "completed",
            "Different thread answer".into(),
            None,
        )
        .await
        .unwrap();
    let service = crate::domains::assistant::chat::service::AssistantChatService::with_persistence(
        Some(Arc::new(client)),
        "gpt-5.5".into(),
        repository.clone(),
        UserDirectory::new(users),
        crate::integration_support::prompt_catalog(),
    );
    let api = crate::domains::assistant::chat::route::routes(Arc::new(service));
    let app = crate::app::http::create_router(
        api.layer(axum::Extension(crate::integration_support::test_actor())),
        &crate::AppConfig::default().http,
    );
    let (content_type, body) = thread_run_multipart("next", None, Some("a1"));
    let response = app
        .oneshot(
            Request::post("/api/v1/assistant/threads/thread-1/runs")
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
    let saved = repository
        .get_thread(user.id, "thread-1".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.messages.last().unwrap().content, "complete");
    assert_eq!(saved.messages.last().unwrap().status, "completed");
    upstream_task.abort();
    let _ = upstream_task.await;
}

#[tokio::test]
async fn routes_outside_api_prefix_are_not_found() {
    let app = test_router().await;

    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn template_preview_route_validates_page_and_degrades_when_unconfigured() {
    let app = test_router().await;
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
        .await
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
    let app = test_router().await;
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
async fn template_export_route_validates_json_before_capability_lookup() {
    let app = test_router().await;
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
async fn template_import_route_validates_mode_and_upload_before_capability_lookup() {
    let app = test_router().await;
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
        .await
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

fn thread_run_multipart(
    prompt: &str,
    context: Option<&str>,
    parent: Option<&str>,
) -> (String, Vec<u8>) {
    const BOUNDARY: &str = "quarry-query-boundary";
    let mut fields = vec![
        ("prompt", prompt),
        ("userMessageId", "u2"),
        ("assistantMessageId", "a2"),
        ("requestId", "r2"),
    ];
    if let Some(context) = context {
        fields.push(("context", context));
    }
    if let Some(parent) = parent {
        fields.push(("parentMessageId", parent));
    }
    let mut body = String::new();
    for (name, value) in fields {
        body.push_str(&format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    body.push_str(&format!("--{BOUNDARY}--\r\n"));
    (
        format!("multipart/form-data; boundary={BOUNDARY}"),
        body.into_bytes(),
    )
}

#[tokio::test]
async fn legacy_command_routes_are_not_exposed() {
    let app = test_router().await;

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
async fn deal_flow_saves_core_fields_then_optional_metadata() {
    const BOUNDARY: &str = "quarry-empty-deal-metadata";
    let app = test_router().await;
    let user_id = 1;

    let create_deal = app
        .clone()
        .oneshot(
            Request::post("/api/v1/deals")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "dealName":"Acme acquisition of WidgetCo",
                        "status":"pipeline", "dealCode":null, "engagementType":"buy_side",
                        "startDate":null, "expectedCloseDate":null, "actualCloseDate":null, "engagementCompletedDate":null,
                        "transactionType":"unknown", "parties":[], "scopes":[],
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
    let deal_id = created_deal["deal"]["dealId"].as_str().unwrap();
    assert_eq!(uuid::Uuid::parse_str(deal_id).unwrap().get_version_num(), 4);
    assert_eq!(created_deal["deal"]["userId"], user_id);
    assert!(created_deal["metadata"].get("userId").is_none());

    let get_deal = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/deals/{deal_id}"))
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
            Request::get(format!("/api/v1/deals/{deal_id}/data-room"))
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
    assert_eq!(empty_data_room["dealId"], deal_id);
    assert_eq!(empty_data_room["tree"], serde_json::json!([]));

    let save_metadata = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/deals/{deal_id}/metadata"))
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
    assert_eq!(saved["deal"]["dealId"], deal_id);
    assert_eq!(saved["deal"]["userId"], user_id);
    assert!(saved["metadata"].get("userId").is_none());
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
            Request::get(format!("/api/v1/deals/{deal_id}"))
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
            Request::post(format!("/api/v1/deals/{deal_id}/metadata"))
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
            Request::post(format!("/api/v1/deals/{deal_id}/archive"))
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
    let app = test_router().await;
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
    let app = test_router().await;
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
    let app = test_router().await;

    let response = app
        .oneshot(
            Request::post("/api/documents/search/keyword")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"queryText":"","limit":10}"#))
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

    let app = test_router().await;
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
    let app = test_router().await;

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
    let app = test_router().await;

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
async fn process_file_accepts_image_uploads_at_the_handler_boundary() {
    const BOUNDARY: &str = "quarry-image-upload-boundary";
    let multipart = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"userId\"\r\n\r\nuser-1\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"evidence.png\"\r\nContent-Type: image/png\r\n\r\nimage-bytes\r\n--{BOUNDARY}--\r\n"
    );
    let app = test_router().await;

    let response = app
        .oneshot(
            Request::post("/api/v1/deals/DEAL-PATH/documents/process_file")
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
async fn process_file_accepts_modern_office_extensions_on_both_api_mounts() {
    for (path, filename) in [
        (
            "/api/v1/deals/DEAL-PATH/documents/process_file",
            "model.XLSX",
        ),
        ("/api/deals/DEAL-PATH/documents/process_file", "board.pptx"),
    ] {
        const BOUNDARY: &str = "quarry-office-upload-boundary";
        let multipart = format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"userId\"\r\n\r\nuser-1\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\nbytes\r\n--{BOUNDARY}--\r\n"
        );
        let response = test_router()
            .await
            .oneshot(
                Request::post(path)
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={BOUNDARY}"),
                    )
                    .body(Body::from(multipart))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED, "{filename}");
    }
}

#[tokio::test]
async fn process_file_rejects_legacy_office_extensions_at_the_handler_boundary() {
    for filename in ["legacy.xls", "legacy.ppt"] {
        const BOUNDARY: &str = "quarry-legacy-office-upload-boundary";
        let multipart = format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"userId\"\r\n\r\nuser-1\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{filename}\"\r\n\r\nbytes\r\n--{BOUNDARY}--\r\n"
        );
        let response = test_router()
            .await
            .oneshot(
                Request::post("/api/v1/deals/DEAL-PATH/documents/process_file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={BOUNDARY}"),
                    )
                    .body(Body::from(multipart))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{filename}");
    }
}

#[tokio::test]
async fn process_file_uses_authenticated_identity_instead_of_blank_legacy_user_id() {
    const BOUNDARY: &str = "quarry-blank-document-user";
    let multipart = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"userId\"\r\n\r\n   \r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"report.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.4\r\n--{BOUNDARY}--\r\n"
    );
    let app = test_router().await;

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

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn stored_document_routes_list_files_and_return_pdf_and_raw_text() {
    let mut docx = Docx::default();
    docx.document
        .push(Paragraph::default().push_text("Raw route text from DOCX."));
    let docx_bytes = docx.write(Cursor::new(Vec::new())).unwrap().into_inner();
    let application = test_application().await.unwrap();
    application
        .postgres
        .with_connection(async |connection| {
            connection.execute(
                "UPDATE users SET first_name='Avery' WHERE id=1", Vec::<crate::adapters::postgres::query::SqlValue>::new(),
            ).await?;
            let user_id = 1;
            for deal_id in ["DEAL-DOCUMENTS", "DEAL-OTHER"] {
                connection.execute(
                    r#"
                    INSERT INTO deals (
                        deal_id, user_id, deal_name, status, created_at, updated_at, engagement_type, transaction_type
                    ) VALUES (?1, ?2, 'Project Test', 'prepare', '2026-01-01', '2026-02-01',
                              'buy_side', 'unknown')
                    "#,
                    crate::pg_params![deal_id, user_id],
                ).await?;
                connection.execute("INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES(?1,?2,'owner','now','now')",crate::pg_params![deal_id,user_id]).await?;
                connection.execute("INSERT INTO deal_access_revisions(deal_id) VALUES(?1)",[deal_id]).await?;
            }
            insert_stored_document(
                &connection,
                "DEAL-DOCUMENTS",
                "file-zulu",
                "version-zulu",
                "Zulu.pdf",
                "application/pdf",
                b"%PDF-1.4\nzulu",
            ).await?;
            insert_stored_document(
                &connection,
                "DEAL-DOCUMENTS",
                "file-alpha",
                "version-alpha",
                "Alpha.pdf",
                "application/pdf",
                b"%PDF-1.4\nalpha",
            ).await?;
            insert_stored_document(
                &connection,
                "DEAL-DOCUMENTS",
                "file-raw",
                "version-raw",
                "Raw.docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                &docx_bytes,
            ).await?;
            insert_stored_document(
                &connection,
                "DEAL-OTHER",
                "file-other",
                "version-other",
                "Other.pdf",
                "application/pdf",
                b"%PDF-1.4\nother",
            ).await?;
            Ok(())
         }).await
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
            {"fileId": "file-alpha", "displayName": "Alpha.pdf", "uploadedByUserId": null, "uploadedByName": null, "attributionOrigin": "legacy_unknown"},
            {"fileId": "file-raw", "displayName": "Raw.docx", "uploadedByUserId": null, "uploadedByName": null, "attributionOrigin": "legacy_unknown"},
            {"fileId": "file-zulu", "displayName": "Zulu.pdf", "uploadedByUserId": null, "uploadedByName": null, "attributionOrigin": "legacy_unknown"}
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

async fn insert_stored_document(
    connection: &crate::adapters::postgres::client::test_support::TestConnection,
    deal_id: &str,
    file_id: &str,
    version_id: &str,
    display_name: &str,
    mime_type: &str,
    bytes: &[u8],
) -> crate::adapters::postgres::client::test_support::TestResult<()> {
    connection
        .execute(
            r#"
        INSERT INTO quarry_files (
            file_id, deal_id, display_name, metadata_json, created_at, updated_at
        ) VALUES (?1, ?2, ?3, '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')
        "#,
            crate::pg_params![file_id, deal_id, display_name],
        )
        .await?;
    connection
        .execute(
            r#"
        INSERT INTO quarry_file_versions (
            version_id, file_id, version_number, original_filename, mime_type,
            content_sha256, byte_size, is_current, created_at
        ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, 1, '2026-01-01T00:00:00Z')
        "#,
            crate::pg_params![
                version_id,
                file_id,
                display_name,
                mime_type,
                "a".repeat(64),
                bytes.len() as i64
            ],
        )
        .await?;
    connection
        .execute(
            "INSERT INTO quarry_file_blobs (version_id, file_bytes) VALUES (?1, ?2)",
            crate::pg_params![version_id, bytes],
        )
        .await?;
    Ok(())
}

#[tokio::test]
async fn research_request_checks_run_before_unconfigured_provider_lookup() {
    let app = test_router().await;
    for (method, path, body, expected) in [
        (
            "POST",
            "/api/v1/indexes",
            Some(r#"{"files":[]}"#),
            StatusCode::BAD_REQUEST,
        ),
        (
            "POST",
            "/api/v1/indexes",
            Some(r#"{"files":[{"filename":"memo.pdf","text":" "}]}"#),
            StatusCode::BAD_REQUEST,
        ),
        (
            "GET",
            "/api/v1/indexes/bad%5Cid/status",
            None,
            StatusCode::BAD_REQUEST,
        ),
        (
            "POST",
            "/api/v1/graphrag/query",
            Some(r#"{"resourceId":"resource","question":" "}"#),
            StatusCode::BAD_REQUEST,
        ),
        (
            "POST",
            "/api/v1/graphrag/query",
            Some(r#"{"resourceId":"../private","question":"Question"}"#),
            StatusCode::BAD_REQUEST,
        ),
        (
            "POST",
            "/api/v1/graphrag/query",
            Some(r#"{"resourceId":"..","question":"Question"}"#),
            StatusCode::BAD_REQUEST,
        ),
        (
            "POST",
            "/api/v1/indexes",
            Some(r#"{"files":[{"filename":"memo.pdf","text":"Document"}]}"#),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "POST",
            "/api/v1/graphrag/query",
            Some(r#"{"resourceId":"resource","question":"Question"}"#),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let mut request = Request::builder().method(method).uri(path);
        if body.is_some() {
            request = request.header("content-type", "application/json");
        }
        let response = app
            .clone()
            .oneshot(
                request
                    .body(body.map_or_else(Body::empty, Body::from))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{method} {path}");
    }
}

#[tokio::test]
async fn assistant_reference_scope_is_rejected_before_storage_or_provider_lookup() {
    let boundary = "assistant-reference-boundary";
    let body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\nQuestion\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"fileScopeDealId\"\r\n\r\nDEAL-A\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"fileReferences\"\r\n\r\n[{{\"dealId\":\"DEAL-B\",\"fileId\":\"file-1\",\"name\":\"Document.pdf\"}}]\r\n--{boundary}--\r\n");
    let response = test_router()
        .await
        .oneshot(
            Request::post("/api/v1/assistant/threads/thread-1/runs")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["error"],
        "referenced file is outside this data room"
    );
}

#[tokio::test]
async fn data_room_relative_paths_are_checked_before_loading_the_configured_source() {
    let app = test_router().await;
    for path in ["", "../outside.pdf", "/absolute.pdf", "./document.pdf"] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/deals/DEAL-PATH/data-room/preview")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"relativePath":path}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["error"],
            "document path must be a non-empty path relative to the deal data room"
        );
    }
}
