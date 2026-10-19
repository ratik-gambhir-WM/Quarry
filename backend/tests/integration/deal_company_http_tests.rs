use crate::integration_support::test_application;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn request(app: &Router, method: &str, path: &str, value: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
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
    let app = test_application().unwrap().router;
    for email in ["owner@example.com", "other@example.com"] {
        assert_eq!(request(&app,"POST","/api/v1/users",json!({"firstName":"Test","lastName":"Owner","email":email,"apiKey":"unused","role":"Analyst"})).await.0, StatusCode::CREATED);
    }
    app
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
    input["status"] = json!("completed");
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
    input["status"] = json!("active");
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
