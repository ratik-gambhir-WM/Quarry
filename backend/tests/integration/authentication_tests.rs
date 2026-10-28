use crate::integration_support::test_application;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Value,
    session: Option<&Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(session) = session {
        request = request
            .header(
                "authorization",
                format!("Bearer {}", session["accessToken"].as_str().unwrap()),
            )
            .header("x-user-id", session["user"]["id"].as_i64().unwrap());
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn register(app: &Router, name: &str) -> Value {
    let (status, response) = call(app, "POST", "/api/v1/auth/register", json!({"username": name, "password": "a sufficiently long password", "email": format!("{name}@westmonroe.com"), "firstName":"Test", "lastName":"User"}), None).await;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    response
}
#[tokio::test]
async fn authentication_register_login_me_and_revoke_across_mounts() {
    let app = test_application().unwrap();
    let owner = register(&app.router, "owner").await;
    assert!(owner["user"].get("passwordHash").is_none());
    assert!(owner["user"].get("apiKey").is_none());
    assert_eq!(owner["accessToken"].as_str().unwrap().len(), 43);
    let (_, me) = call(
        &app.router,
        "GET",
        "/api/auth/me",
        Value::Null,
        Some(&owner),
    )
    .await;
    assert_eq!(me["user"]["username"], "owner");
    assert_eq!(call(&app.router,"POST","/api/v1/auth/register",json!({"username":"OWNER", "password":"a sufficiently long password", "email":"second@westmonroe.com", "firstName":"T", "lastName":"U"}),None).await.0,StatusCode::CONFLICT);
    let (_, login) = call(
        &app.router,
        "POST",
        "/api/v1/auth/login",
        json!({"username":" OWNER ", "password":"a sufficiently long password"}),
        None,
    )
    .await;
    assert_ne!(login["accessToken"], owner["accessToken"]);
    for name in ["owner", "unknown"] {
        assert_eq!(
            call(
                &app.router,
                "POST",
                "/api/v1/auth/login",
                json!({"username":name,"password":"incorrect long password"}),
                None
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    let response = app
        .router
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/me")
                .header(
                    "authorization",
                    format!("Bearer {}", owner["accessToken"].as_str().unwrap()),
                )
                .header("x-user-id", "999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        call(
            &app.router,
            "POST",
            "/api/auth/logout",
            Value::Null,
            Some(&owner)
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app.router,
            "GET",
            "/api/v1/auth/me",
            Value::Null,
            Some(&owner)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app.router,
            "GET",
            "/api/v1/auth/me",
            Value::Null,
            Some(&login)
        )
        .await
        .0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn authentication_protects_all_product_routes_and_removes_profile_bypasses() {
    let app = test_application().unwrap();
    for prefix in ["/api", "/api/v1"] {
        for path in [
            "/deals",
            "/companies",
            "/assistant/threads",
            "/documents/process_file/unknown/events",
        ] {
            assert_eq!(
                call(
                    &app.router,
                    "GET",
                    &format!("{prefix}{path}"),
                    Value::Null,
                    None
                )
                .await
                .0,
                StatusCode::UNAUTHORIZED,
                "{prefix}{path}"
            );
        }
        for path in [
            "/users",
            "/users/by-email",
            "/database/status",
            "/login-demo",
            "/summarize/files",
        ] {
            assert_eq!(
                call(
                    &app.router,
                    "GET",
                    &format!("{prefix}{path}"),
                    Value::Null,
                    None
                )
                .await
                .0,
                StatusCode::NOT_FOUND
            );
        }
    }
    let owner = register(&app.router, "owner").await;
    let other = register(&app.router, "other").await;
    let (_, thread) = call(
        &app.router,
        "POST",
        "/api/v1/assistant/threads",
        json!({}),
        Some(&owner),
    )
    .await;
    let path = format!(
        "/api/v1/assistant/threads/{}",
        thread["threadId"].as_str().unwrap()
    );
    assert_eq!(
        call(&app.router, "GET", &path, Value::Null, Some(&other))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app.router,
            "GET",
            &format!("{path}?userEmail=other@westmonroe.com"),
            Value::Null,
            Some(&owner)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app.router,
            "GET",
            "/api/v1/deals/DEAL-other/data-room",
            Value::Null,
            Some(&owner)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn authentication_hashes_are_salted_and_sessions_expire_without_exposing_secrets() {
    let app = test_application().unwrap();
    let owner = register(&app.router, "owner").await;
    register(&app.router, "other").await;
    let hashes = app
        .sqlite
        .with_connection(|c| {
            let mut statement = c.prepare("SELECT password_hash FROM users ORDER BY id")?;
            let values = statement
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(values)
        })
        .unwrap();
    assert_ne!(hashes[0], hashes[1]);
    assert!(hashes
        .iter()
        .all(|h| h.starts_with("$argon2id$v=19$m=19456,t=2,p=1$")));
    let raw = owner["accessToken"].as_str().unwrap();
    let stored = app
        .sqlite
        .with_connection(|c| {
            c.query_row(
                "SELECT token_hash FROM auth_sessions WHERE user_id=1",
                [],
                |r| r.get::<_, String>(0),
            )
        })
        .unwrap();
    assert_ne!(raw, stored);
    assert_eq!(stored.len(), 64);
    app.sqlite
        .with_connection(|c| {
            c.execute(
                "UPDATE auth_sessions SET created_at=0, expires_at=1 WHERE user_id=1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        call(
            &app.router,
            "GET",
            "/api/v1/auth/me",
            Value::Null,
            Some(&owner)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    for password in ["short", &"x".repeat(129)] {
        assert_eq!(call(&app.router,"POST","/api/v1/auth/register",json!({"username":"invalid", "password":password,"email":"invalid@westmonroe.com","firstName":"T","lastName":"U"}),None).await.0,StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn authentication_rejects_cross_owner_deals_companies_and_job_subscriptions() {
    let app = test_application().unwrap();
    let owner = register(&app.router, "owner").await;
    let other = register(&app.router, "other").await;
    let (_, company) = call(
        &app.router,
        "POST",
        "/api/v1/companies",
        json!({"name":"Owned company", "website":null,"industry":null,"description":null}),
        Some(&owner),
    )
    .await;
    let update = format!(
        "/api/v1/companies/{}/update",
        company["companyId"].as_str().unwrap()
    );
    assert_eq!(
        call(
            &app.router,
            "POST",
            &update,
            json!({"name":"Stolen", "website":null,"industry":null,"description":null}),
            Some(&other)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let deal = json!({"dealId":"DEAL-OWNED","dealName":"Owned","dealCode":null,"engagementType":"buy_side","transactionType":"unknown","status":"pipeline","startDate":null,"expectedCloseDate":null,"actualCloseDate":null,"engagementCompletedDate":null,"parties":[],"scopes":[],"localPath":null,"sharepointLink":null});
    assert_eq!(
        call(&app.router, "POST", "/api/v1/deals", deal, Some(&owner))
            .await
            .0,
        StatusCode::CREATED
    );
    for path in [
        "/api/v1/deals/DEAL-OWNED",
        "/api/v1/deals/DEAL-OWNED/documents",
        "/api/v1/deals/DEAL-OWNED/data-room",
    ] {
        assert_eq!(
            call(&app.router, "GET", path, Value::Null, Some(&other))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    let (_, list) = call(
        &app.router,
        "GET",
        "/api/v1/deals",
        Value::Null,
        Some(&other),
    )
    .await;
    assert_eq!(list, json!([]));
    assert_eq!(
        call(
            &app.router,
            "GET",
            "/api/v1/documents/process_file/guessed/events",
            Value::Null,
            Some(&other)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn authentication_logout_closes_an_admitted_stream_and_preflight_allows_headers() {
    use crate::domains::identity::{
        access::ResourceAccess, repository::IdentityRepository, service::IdentityService,
    };
    use crate::domains::users::UserRepository;
    use futures_util::StreamExt;
    use std::sync::Arc;
    let app = test_application().unwrap();
    let owner = register(&app.router, "owner").await;
    let identity = Arc::new(IdentityService::new(IdentityRepository::new(
        app.sqlite.clone(),
        UserRepository::new(app.sqlite.clone()),
    )));
    let stream = Router::new()
        .route(
            "/events",
            axum::routing::get(|| async {
                axum::response::sse::Sse::new(futures_util::stream::pending::<
                    Result<axum::response::sse::Event, std::convert::Infallible>,
                >())
            }),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            crate::app::http::auth::AuthBoundary {
                identity: identity.clone(),
                access: ResourceAccess::new(app.sqlite.clone()),
            },
            crate::app::http::auth::authenticate,
        ));
    let response = stream
        .oneshot(
            Request::get("/events")
                .header(
                    "authorization",
                    format!("Bearer {}", owner["accessToken"].as_str().unwrap()),
                )
                .header("x-user-id", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let actor = identity
        .authenticate(owner["accessToken"].as_str().unwrap())
        .await
        .unwrap();
    identity.logout(&actor).await.unwrap();
    let mut body = response.into_body().into_data_stream();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), body.next())
            .await
            .unwrap()
            .is_none()
    );
    let response = app
        .router
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/auth/me")
                .header("origin", "http://localhost:1420")
                .header("access-control-request-method", "GET")
                .header("access-control-request-headers", "authorization,x-user-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let allowed = response
        .headers()
        .get("access-control-allow-headers")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(allowed.contains("authorization"));
    assert!(allowed.contains("x-user-id"));
}

#[tokio::test]
async fn authentication_registration_requires_the_exact_west_monroe_domain() {
    let app = test_application().unwrap();
    for email in [
        "outsider@example.com",
        "analyst@westmonroe.com.evil.test",
        "analyst@sub.westmonroe.com",
    ] {
        let (status, _) = call(&app.router, "POST", "/api/v1/auth/register", json!({"username":"analyst", "password":"a sufficiently long password", "email":email, "firstName":"Test", "lastName":"User"}), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, response) = call(&app.router, "POST", "/api/v1/auth/register", json!({"username":"analyst", "password":"a sufficiently long password", "email":" Analyst@WestMonroe.com ", "firstName":"Test", "lastName":"User"}), None).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response["user"]["email"], "analyst@westmonroe.com");
}
