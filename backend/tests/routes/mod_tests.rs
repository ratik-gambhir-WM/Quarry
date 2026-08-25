use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use super::*;

#[tokio::test]
async fn health_route_is_available_under_api_prefix() {
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());

    let response = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn versioned_health_and_capabilities_routes_are_available() {
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());

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
async fn routes_outside_api_prefix_are_not_found() {
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());

    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn legacy_command_routes_are_not_exposed() {
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());

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
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());
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
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());
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
async fn deal_flow_saves_core_fields_then_optional_metadata() {
    const BOUNDARY: &str = "quarry-empty-deal-metadata";
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());
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
                .body(Body::from(format!("--{BOUNDARY}--\r\n")))
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
    assert_eq!(saved["metadata"]["sharepointLink"], Value::Null);
    assert_eq!(saved["files"], serde_json::json!([]));

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
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());
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
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());
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
    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());

    let response = app
        .oneshot(
            Request::post("/api/documents/search/keyword")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"userId":"user-1","queryText":"","limit":10}"#,
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

    let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());
    let response = app
        .oneshot(
            Request::post("/api/documents/process_file")
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
