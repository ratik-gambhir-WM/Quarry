use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use docx_rust::{document::Paragraph, Docx};
use serde_json::Value;
use std::io::Cursor;
use tower::ServiceExt;

use crate::{bootstrap::test_application, config::AppConfig};

fn test_router() -> axum::Router {
    let application = test_application().unwrap();
    create_router(application.state, &AppConfig::default().http)
}

use super::*;

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
async fn routes_outside_api_prefix_are_not_found() {
    let app = test_router();

    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
    let app = create_router(application.state, &AppConfig::default().http);

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
