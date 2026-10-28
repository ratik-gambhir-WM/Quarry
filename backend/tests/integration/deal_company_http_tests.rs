use crate::integration_support::test_application;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn request(app: &Router, method: &str, path: &str, value: Value) -> (StatusCode, Value) {
    let email = value
        .get("userEmail")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            if path.contains("other") {
                "other@example.com"
            } else {
                "owner@example.com"
            }
        });
    let other = email == "other@example.com";
    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        object.remove("userEmail");
    }
    let path = path
        .replace("userEmail=owner%40example.com&", "")
        .replace("userEmail=other%40example.com&", "")
        .replace("?userEmail=owner%40example.com", "")
        .replace("?userEmail=other%40example.com", "");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path.as_str())
                .header("content-type", "application/json")
                .header(
                    "authorization",
                    format!("Bearer {}", if other { "b" } else { "a" }.repeat(43)),
                )
                .header("x-user-id", if other { "2" } else { "1" })
                .body(if method == "GET" {
                    Body::empty()
                } else {
                    Body::from(value.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn deal() -> Value {
    json!({"dealId":"DEAL-MODEL","userEmail":"owner@example.com","dealName":"Pipeline","dealCode":null,"engagementType":"sell_side","transactionType":"unknown","status":"pipeline","startDate":null,"expectedCloseDate":null,"actualCloseDate":null,"engagementCompletedDate":null,"parties":[],"scopes":[]})
}
async fn app() -> Router {
    let app = test_application().unwrap();
    crate::integration_support::seed_identity(&app.sqlite, "owner@example.com", 'a');
    crate::integration_support::seed_identity(&app.sqlite, "other@example.com", 'b');
    crate::integration_support::with_test_credentials(app.router)
}

async fn save_metadata_links(app: &Router, fields: &[(&str, &str)]) -> (StatusCode, Value) {
    const BOUNDARY: &str = "quarry-optional-links";
    let mut body = String::new();
    for (name, value) in fields {
        body.push_str(&format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    body.push_str(&format!("--{BOUNDARY}--\r\n"));
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/deals/DEAL-MODEL/metadata")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn optional_metadata_links_accept_blanks_preserve_omitted_and_validate_populated_fields() {
    let app = app().await;
    assert_eq!(
        request(&app, "POST", "/api/v1/deals", deal()).await.0,
        StatusCode::CREATED
    );
    // Browser and desktop adapters send blanks for unfilled optional fields.
    let links = [
        (
            "sharepointLink",
            "https://example.sharepoint.com/sites/deal",
        ),
        (
            "sowLink",
            "https://example.sharepoint.com/sites/deal/sow.pdf",
        ),
        ("factSheetLink", ""),
        ("rlLink", "   "),
    ];
    let (status, saved) = save_metadata_links(&app, &links).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["metadata"]["sharepointLink"], links[0].1);
    assert_eq!(saved["metadata"]["sowLink"], links[1].1);
    assert!(saved["metadata"]["factSheetLink"].is_null());
    assert!(saved["metadata"]["rlLink"].is_null());

    for (field, value) in [
        ("sharepointLink", "https://example.com/not-sharepoint"),
        ("sowLink", "http://example.com/sow"),
        ("factSheetLink", "javascript:alert(1)"),
        ("rlLink", "https://user:password@example.com/list"),
        ("sowLink", "https://example.com/embedded\ncontrol"),
    ] {
        let (status, error) = save_metadata_links(&app, &[(field, value)]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
        assert!(error["error"].as_str().unwrap().contains(field), "{error}");
    }
    // Omission preserves a saved link; an explicitly blank field clears it.
    let (status, saved) = save_metadata_links(&app, &[("sowLink", "")]).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["metadata"]["sharepointLink"], links[0].1);
    assert!(saved["metadata"]["sowLink"].is_null());
    let blanks = [
        ("sharepointLink", ""),
        ("sowLink", " "),
        ("factSheetLink", ""),
        ("rlLink", ""),
    ];
    let (status, saved) = save_metadata_links(&app, &blanks).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, persisted) = request(&app, "GET", "/api/v1/deals/DEAL-MODEL", Value::Null).await;
    for (field, _) in blanks {
        assert!(saved["metadata"][field].is_null());
        assert!(persisted["metadata"][field].is_null());
    }
}
#[tokio::test]
async fn company_directory_is_bounded_scoped_and_deal_updates_preserve_metadata() {
    let app = app().await;
    let input = json!({"userEmail":"owner@example.com","name":"Same Name","website":null,"industry":null,"description":null});
    let (status, first) = request(&app, "POST", "/api/v1/companies", input.clone()).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, second) = request(&app, "POST", "/api/v1/companies", input.clone()).await;
    assert_ne!(first["companyId"], second["companyId"]);
    let (_, list) = request(
        &app,
        "GET",
        "/api/v1/companies?userEmail=owner%40example.com&limit=1",
        Value::Null,
    )
    .await;
    assert_eq!(list["hasMore"], true);
    assert_eq!(list["companies"].as_array().unwrap().len(), 1);
    let (_, other) = request(
        &app,
        "GET",
        "/api/v1/companies?userEmail=other%40example.com",
        Value::Null,
    )
    .await;
    assert_eq!(other["companies"], json!([]));
    let mut input = deal();
    input["parties"] = json!([{"companyId":first["companyId"],"role":"buyer","notes":null},{"companyId":first["companyId"],"role":"client","notes":null}]);
    input["scopes"] = json!(["corporate_it"]);
    let (status, created) = request(&app, "POST", "/api/v1/deals", input.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(
        request(&app, "POST", "/api/v1/deals", input.clone())
            .await
            .0,
        StatusCode::CONFLICT
    );
    let party_id = created["deal"]["parties"][0]["dealPartyId"].clone();
    for key in ["dealId", "userEmail"] {
        input.as_object_mut().unwrap().remove(key);
    }
    input["dealName"] = json!("Edited");
    input["engagementCompletedDate"] = json!("2026-02-01");
    input["status"] = json!("closed");
    let (status, updated) = request(
        &app,
        "POST",
        "/api/v1/deals/DEAL-MODEL/update",
        input.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["deal"]["parties"][0]["dealPartyId"], party_id);
    assert_eq!(updated["metadata"], created["metadata"]);
    assert!(updated["deal"]["actualCloseDate"].is_null());
    input["parties"] = json!([]);
    input["scopes"] = json!([]);
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/deals/DEAL-MODEL/update",
            input.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    input.as_object_mut().unwrap().remove("scopes");
    assert!(
        request(&app, "POST", "/api/v1/deals/DEAL-MODEL/update", input)
            .await
            .0
            .is_client_error()
    );
}
#[tokio::test]
async fn validates_enum_dates_cardinality_and_references_at_the_correct_boundary() {
    let app = app().await;
    for (key, value) in [
        ("status", json!("Active")),
        ("startDate", json!("")),
        ("expectedCloseDate", json!("2026-02-30")),
        ("transactionType", json!("acquisition")),
    ] {
        let mut input = deal();
        input[key] = value;
        assert!(request(&app, "POST", "/api/v1/deals", input)
            .await
            .0
            .is_client_error());
    }
    let mut input = deal();
    input["transactionType"] = json!("platform_acquisition");
    input["status"] = json!("prepare");
    assert_eq!(
        request(&app, "POST", "/api/v1/deals", input.clone())
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    input["status"] = json!("pipeline");
    input["parties"] =
        json!([{"companyId":uuid::Uuid::new_v4().to_string(),"role":"target","notes":null}]);
    assert_eq!(
        request(&app, "POST", "/api/v1/deals", input).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, company)=request(&app,"POST","/api/v1/companies",json!({"userEmail":"other@example.com","name":"Other company","website":null,"industry":null,"description":null})).await;
    let mut input = deal();
    input["parties"] = json!([{"companyId":company["companyId"],"role":"target","notes":null}]);
    assert_eq!(
        request(&app, "POST", "/api/v1/deals", input).await.0,
        StatusCode::CONFLICT
    );
}
#[tokio::test]
async fn nullable_chat_association_owner_checks_scopes_and_archive_retention() {
    let app = app().await;
    assert_eq!(
        request(&app, "POST", "/api/v1/deals", deal()).await.0,
        StatusCode::CREATED
    );
    for (id, linked) in [("free", Value::Null), ("linked", json!("DEAL-MODEL"))] {
        let (status, thread) = request(
            &app,
            "POST",
            "/api/v1/assistant/threads",
            json!({"userEmail":"owner@example.com","threadId":id,"dealId":linked}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(thread["dealId"], linked);
    }
    for (scope, expected) in [
        ("scope=all", "linked"),
        ("scope=freeform", "free"),
        ("scope=deal&dealId=DEAL-MODEL", "linked"),
    ] {
        let (_, page) = request(
            &app,
            "GET",
            &format!("/api/v1/assistant/threads?userEmail=owner%40example.com&{scope}"),
            Value::Null,
        )
        .await;
        assert!(page["threads"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["threadId"] == expected));
        if !scope.ends_with("all") {
            assert_eq!(page["threads"].as_array().unwrap().len(), 1);
        }
    }
    assert_eq!(request(&app,"GET","/api/v1/assistant/threads?userEmail=owner%40example.com&scope=freeform&dealId=DEAL-MODEL",Value::Null).await.0,StatusCode::BAD_REQUEST);
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/assistant/threads",
            json!({"userEmail":"other@example.com","threadId":"wrong-owner","dealId":"DEAL-MODEL"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, archived) = request(&app, "POST", "/api/v1/deals/DEAL-MODEL/archive", json!({})).await;
    assert_eq!(archived["status"], "pipeline");
    assert!(archived["archivedAt"].is_string());
    assert_eq!(
        request(&app, "GET", "/api/v1/deals/DEAL-MODEL", Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    let (_, thread) = request(
        &app,
        "GET",
        "/api/v1/assistant/threads/linked?userEmail=owner%40example.com",
        Value::Null,
    )
    .await;
    assert_eq!(thread["dealId"], "DEAL-MODEL");
}

#[tokio::test]
async fn diligence_status_contract_roundtrips_and_archive_preserves_terminal_stage() {
    let app = app().await;
    let mut input = deal();
    assert_eq!(
        request(&app, "POST", "/api/v1/deals", input.clone())
            .await
            .0,
        StatusCode::CREATED
    );
    input.as_object_mut().unwrap().remove("dealId");
    let frontend = include_str!("../../../frontend/src/data/dealModel.ts");
    for code in [
        "pipeline",
        "prepare",
        "review_plan_discovery",
        "interviews_site_visit",
        "debrief_flash_report",
        "follow_ups_analysis",
        "qa_readout_closeout",
        "closed",
        "pencils_down",
    ] {
        assert!(frontend.contains(&format!("\"{code}\"")));
        input["status"] = json!(code);
        let mut create = input.clone();
        create["dealId"] = json!(format!("DEAL-{code}"));
        let (created_status, created) = request(&app, "POST", "/api/v1/deals", create).await;
        assert_eq!(created_status, StatusCode::CREATED, "{created}");
        assert_eq!(created["deal"]["status"], code);
        let (status, saved) = request(
            &app,
            "POST",
            "/api/v1/deals/DEAL-MODEL/update",
            input.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        assert_eq!(saved["deal"]["status"], code);
        assert_eq!(saved["deal"]["actualCloseDate"], Value::Null);
        assert_eq!(saved["deal"]["engagementCompletedDate"], Value::Null);
        let (_, read) = request(&app, "GET", "/api/v1/deals/DEAL-MODEL", Value::Null).await;
        assert_eq!(read["status"], code, "{read}");
    }
    for code in ["active", "on_hold", "completed", "cancelled", "unknown"] {
        input["status"] = json!(code);
        let (status, error) = request(
            &app,
            "POST",
            "/api/v1/deals/DEAL-MODEL/update",
            input.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!error.to_string().contains(code));
        let mut create = input.clone();
        create["dealId"] = json!("DEAL-INVALID");
        assert_eq!(
            request(&app, "POST", "/api/v1/deals", create).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    for _ in 0..2 {
        let (status, archived) =
            request(&app, "POST", "/api/v1/deals/DEAL-MODEL/archive", json!({})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(archived["status"], "pencils_down");
        assert!(archived["archivedAt"].is_string());
    }
}

#[tokio::test]
async fn deal_json_validation_preserves_body_limits_and_content_type_errors() {
    let app = app().await;
    for (content_type, body, expected) in [
        (
            "text/plain",
            "{}".to_string(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            "application/json",
            " ".repeat(2 * 1024 * 1024 + 1),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        ("application/json", "{".to_string(), StatusCode::BAD_REQUEST),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/deals")
                    .header("authorization", format!("Bearer {}", "a".repeat(43)))
                    .header("x-user-id", "1")
                    .header("content-type", content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}
