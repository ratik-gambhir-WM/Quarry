use crate::integration_support::{seed_identity, test_application, TestApplication};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
async fn call(
    app: &Router,
    actor: i64,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let token = (char::from(b'a' + u8::try_from(actor - 1).unwrap()))
        .to_string()
        .repeat(43);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .header("x-user-id", actor)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn fixture() -> TestApplication {
    let app = test_application().unwrap();
    for (name, token) in [
        ("owner", 'a'),
        ("manager", 'b'),
        ("contributor", 'c'),
        ("viewer", 'd'),
        ("outsider", 'e'),
    ] {
        seed_identity(&app.sqlite, &format!("{name}@example.test"), token);
        app.sqlite
            .with_connection(|c| {
                c.execute(
                    "UPDATE users SET username=?1 WHERE email=?2",
                    [name, &format!("{name}@example.test")],
                )?;
                Ok(())
            })
            .unwrap();
    }
    let body = json!({
        "dealName": "Shared project",
        "dealCode": null,
        "startDate": null,
        "expectedCloseDate": null,
        "actualCloseDate": null,
        "engagementCompletedDate": null,
        "engagementType": "buy_side",
        "transactionType": "unknown",
        "status": "pipeline",
        "parties": [],
        "scopes": [],
        "localPath": "/private/device/source"
    });
    // Retain legacy-ID coverage independently of the UUID-only creation API.
    let input: crate::domains::deals::service::SaveDealInput =
        serde_json::from_value(body).unwrap();
    crate::domains::deals::repository::DealRepository::new(app.sqlite.clone())
        .create(crate::domains::deals::repository::CreateDealRecord {
            deal_id: "DEAL-SHARED".into(),
            user_id: 1,
            fields: input.fields,
            local_path: input.local_path,
            sharepoint_link: input.sharepoint_link,
        })
        .await
        .unwrap();
    for (name, role) in [
        ("manager", "manager"),
        ("contributor", "contributor"),
        ("viewer", "viewer"),
    ] {
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                "/api/v1/deals/DEAL-SHARED/members",
                json!({"email":format!("{name}@example.test"),"role":role})
            )
            .await
            .0,
            StatusCode::CREATED
        );
    }
    app
}
#[tokio::test]
async fn shared_http_role_matrix_on_versioned_and_compatibility_mounts() {
    let app = fixture().await;
    for mount in ["/api/v1", "/api"] {
        for actor in 1..=5 {
            let read = if actor == 5 {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::OK
            };
            for suffix in ["", "/members", "/teams", "/activity", "/documents"] {
                assert_eq!(
                    call(
                        &app.router,
                        actor,
                        "GET",
                        &format!("{mount}/deals/DEAL-SHARED{suffix}"),
                        Value::Null
                    )
                    .await
                    .0,
                    read,
                    "{actor} {suffix}"
                );
            }
            for (suffix, allowed) in [
                ("/update", actor <= 2),
                ("/metadata", actor <= 2),
                ("/extraction/upload", actor <= 2),
                ("/documents/process", actor <= 3),
                ("/documents/process_file", actor <= 3),
                ("/data-room/preview", actor == 1),
            ] {
                let status = call(
                    &app.router,
                    actor,
                    "POST",
                    &format!("{mount}/deals/DEAL-SHARED{suffix}"),
                    json!({}),
                )
                .await
                .0;
                if actor == 5 {
                    assert_eq!(status, StatusCode::NOT_FOUND);
                } else if !allowed {
                    assert_eq!(status, StatusCode::FORBIDDEN, "actor {actor} {suffix}");
                } else {
                    assert_ne!(status, StatusCode::FORBIDDEN, "actor {actor} {suffix}");
                }
            }
            let (_, deal) = call(
                &app.router,
                actor,
                "GET",
                &format!("{mount}/deals/DEAL-SHARED"),
                Value::Null,
            )
            .await;
            if actor <= 4 {
                assert_eq!(deal["createdByUserId"], 1);
                assert_eq!(deal["capabilities"]["upload"], actor <= 3);
                if actor != 1 {
                    assert!(deal["metadata"]["localPath"].is_null());
                }
            }
        }
    }
    // 403 is an operation denial, not session revocation.
    assert_eq!(
        call(&app.router, 4, "GET", "/api/v1/auth/me", Value::Null)
            .await
            .0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn membership_http_validation_conflicts_and_team_lifecycle() {
    let app = fixture().await;
    let root = "/api/v1/deals/DEAL-SHARED";
    for body in [
        json!({"email":"owner@example.test","role":"owner"}),
        json!({"email":"not-an-email","role":"viewer"}),
        json!({"email":"viewer@example.test","role":"admin"}),
        json!({"email":"viewer@example.test","role":"viewer","actorId":1}),
    ] {
        assert!(
            call(&app.router, 1, "POST", &format!("{root}/members"), body)
                .await
                .0
                .is_client_error()
        );
    }
    assert_eq!(
        call(
            &app.router,
            1,
            "POST",
            &format!("{root}/members"),
            json!({"email":"VIEWER@EXAMPLE.TEST","role":"viewer"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "POST",
            &format!("{root}/members"),
            json!({"email":"outsider@example.test","role":"viewer"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    for query in ["?limit=0", "?limit=101", "?cursor=-1"] {
        assert_eq!(
            call(
                &app.router,
                1,
                "GET",
                &format!("{root}/members{query}"),
                Value::Null
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        call(
            &app.router,
            1,
            "PATCH",
            &format!("{root}/members/4"),
            json!({"role":"contributor","expectedRevision":2})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app.router,
            1,
            "PATCH",
            &format!("{root}/members/4"),
            json!({"role":"contributor","expectedRevision":1})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "POST",
            &format!("{root}/teams"),
            json!({"name":" Platform   Team "})
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "POST",
            &format!("{root}/teams"),
            json!({"name":"Another team"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for name in ["".to_string(), "x".repeat(101), "line\nbreak".into()] {
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                &format!("{root}/teams"),
                json!({"name":name})
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let (_, teams) = call(&app.router, 4, "GET", &format!("{root}/teams"), Value::Null).await;
    let team = teams["items"][0]["teamId"].as_str().unwrap();
    assert_eq!(teams["items"][0]["name"], "Platform Team");
    for _ in 0..2 {
        assert_eq!(
            call(
                &app.router,
                2,
                "POST",
                &format!("{root}/teams/{team}/members"),
                json!({"userId":4})
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        call(
            &app.router,
            2,
            "POST",
            &format!("{root}/teams/{team}/members"),
            json!({"userId":5})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "PATCH",
            &format!("{root}/teams/{team}"),
            json!({"name":"Renamed","expectedRevision":1})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "PATCH",
            &format!("{root}/teams/{team}"),
            json!({"name":"Conflict","expectedRevision":1})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "DELETE",
            &format!("{root}/teams/{team}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (_, members) = call(
        &app.router,
        1,
        "GET",
        &format!("{root}/members"),
        Value::Null,
    )
    .await;
    assert_eq!(members["items"].as_array().unwrap().len(), 4);
    assert!(members.to_string().find("email").is_none());
}
#[tokio::test]
async fn transfer_removal_archive_and_private_resources_stay_isolated() {
    let app = fixture().await;
    let root = "/api/v1/deals/DEAL-SHARED";
    let (_, thread) = call(
        &app.router,
        1,
        "POST",
        "/api/v1/assistant/threads",
        json!({"dealId":"DEAL-SHARED"}),
    )
    .await;
    let (_, notebook) = call(
        &app.router,
        1,
        "POST",
        "/api/v1/notebooks",
        json!({"title":"Private notes"}),
    )
    .await;
    assert_eq!(
        call(
            &app.router,
            1,
            "POST",
            &format!("{root}/ownership-transfer"),
            json!({"userId":2,"expectedAccessRevision":4})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "GET",
            &format!("{root}/data-room"),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app.router,
            1,
            "GET",
            &format!("{root}/data-room"),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "DELETE",
            &format!("{root}/members/1"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app.router, 1, "GET", root, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    for suffix in ["/documents", "/members", "/activity", "/data-room"] {
        assert_eq!(
            call(
                &app.router,
                1,
                "GET",
                &format!("{root}{suffix}"),
                Value::Null
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    let (_, list) = call(&app.router, 1, "GET", "/api/v1/deals", Value::Null).await;
    assert_eq!(list, json!([]));
    if let Some(id) = thread["threadId"].as_str() {
        assert_eq!(
            call(
                &app.router,
                2,
                "GET",
                &format!("/api/v1/assistant/threads/{id}"),
                Value::Null
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    let notebook = notebook["notebookId"].as_str().unwrap();
    assert_eq!(
        call(
            &app.router,
            2,
            "GET",
            &format!("/api/v1/notebooks/{notebook}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "POST",
            &format!("{root}/archive"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "POST",
            &format!("{root}/teams"),
            json!({"name":"Blocked"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app.router,
            3,
            "DELETE",
            &format!("{root}/members/3"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app.router,
            2,
            "DELETE",
            &format!("{root}/members/2"),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn membership_email_only_lookup_preserves_authorization_and_conflicts() {
    for mount in ["/api/v1", "/api"] {
        let app = fixture().await;
        let root = format!("{mount}/deals/DEAL-SHARED");
        let members = format!("{root}/members");
        for (actor, body, expected) in [
            (
                1,
                json!({"username":"outsider","role":"viewer"}),
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                1,
                json!({"email":"outsider","role":"viewer"}),
                StatusCode::BAD_REQUEST,
            ),
            (
                1,
                json!({"email":"unknown@example.test","role":"viewer"}),
                StatusCode::NOT_FOUND,
            ),
            (
                2,
                json!({"email":"outsider@example.test","role":"viewer"}),
                StatusCode::FORBIDDEN,
            ),
            (
                1,
                json!({"email":"  OUTSIDER@EXAMPLE.TEST  ","role":"contributor"}),
                StatusCode::CREATED,
            ),
            (
                1,
                json!({"email":"outsider@example.test","role":"viewer"}),
                StatusCode::CONFLICT,
            ),
        ] {
            assert_eq!(
                call(&app.router, actor, "POST", &members, body).await.0,
                expected
            );
        }
        let (_, roster) = call(&app.router, 1, "GET", &members, Value::Null).await;
        let added = roster["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|member| member["userId"] == 5)
            .unwrap();
        assert_eq!(added["username"], "outsider");
        assert_eq!(added["role"], "contributor");
        assert!(added.get("email").is_none());
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                &format!("{root}/archive"),
                Value::Null
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                &members,
                json!({"email":"unknown@example.test","role":"viewer"})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
}

#[tokio::test]
async fn backend_generated_deal_uuids_work_across_product_boundaries() {
    let app = fixture().await;
    let fields = json!({
        "dealName":"Project Fleet", "dealCode":null,
        "engagementType":"buy_side", "transactionType":"unknown", "status":"pipeline",
        "startDate":null, "expectedCloseDate":null, "actualCloseDate":null,
        "engagementCompletedDate":null, "parties":[], "scopes":[]
    });
    let mut previous = None;
    for prefix in ["/api/v1", "/api"] {
        let mut request = fields.clone();
        if prefix == "/api" {
            request["dealId"] = json!("DEAL-CALLER");
        }
        let (status, created) =
            call(&app.router, 1, "POST", &format!("{prefix}/deals"), request).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["deal"]["dealId"].as_str().unwrap();
        assert_eq!(uuid::Uuid::parse_str(id).unwrap().get_version_num(), 4);
        assert_eq!(created["metadata"]["dealId"], id);
        assert_ne!(id, "DEAL-CALLER");
        assert_ne!(previous.as_deref(), Some(id));
        previous = Some(id.to_owned());
        let root = format!("{prefix}/deals/{id}");
        for suffix in ["", "/members", "/teams", "/documents", "/data-room"] {
            assert_eq!(
                call(
                    &app.router,
                    1,
                    "GET",
                    &format!("{root}{suffix}"),
                    Value::Null
                )
                .await
                .0,
                StatusCode::OK,
                "{suffix}"
            );
        }
        // UUIDs must not bypass existing ownership checks.
        assert_eq!(
            call(&app.router, 5, "GET", &root, Value::Null).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                &format!("{root}/update"),
                fields.clone()
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                &format!("{root}/teams"),
                json!({"name":"Fleet team"})
            )
            .await
            .0,
            StatusCode::CREATED
        );
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                &format!("{prefix}/assistant/threads"),
                json!({"threadId":uuid::Uuid::new_v4().to_string(),"dealId":id})
            )
            .await
            .0,
            StatusCode::CREATED
        );
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                &format!("{prefix}/notebooks"),
                json!({"title":"Fleet notebook","dealId":id})
            )
            .await
            .0,
            StatusCode::CREATED
        );
        assert_eq!(
            call(
                &app.router,
                1,
                "POST",
                &format!("{root}/archive"),
                json!({})
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    // Reject invalid caller fields before creating any additional records.
    let mut invalid = fields.clone();
    invalid["dealName"] = json!(" ");
    assert_eq!(
        call(&app.router, 1, "POST", "/api/v1/deals", invalid)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let rows = app
        .sqlite
        .read_async(
            crate::adapters::sqlite::query::SqlBuilder::select("deals")
                .columns(["deal_id"])
                .build(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 3); // The legacy fixture plus two UUID deals, even with identical names.
}
