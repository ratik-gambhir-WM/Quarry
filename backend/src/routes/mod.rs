mod data_room;
mod deal;
mod documents;
mod research;
mod system;
mod users;

use axum::{
    http::{header, HeaderName, Method, StatusCode},
    Router,
};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{config::AppConfig, state::AppState};

pub fn create_router(state: AppState, config: &AppConfig) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(config.cors_origins.clone()))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, request_id_header.clone()])
        .expose_headers([request_id_header.clone()]);

    let api = Router::new()
        .merge(system::routes())
        .merge(users::routes())
        .merge(deal::routes())
        .merge(documents::routes())
        .merge(data_room::routes())
        .merge(research::routes());

    Router::new()
        .nest("/api", api.clone())
        .nest("/api/v1", api)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            config.request_timeout,
        ))
        .layer(cors)
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .with_state(state)
}

#[cfg(test)]
mod tests {
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
    async fn helix_routes_use_explicit_helix_paths() {
        let app = create_router(AppState::in_memory().unwrap(), &AppConfig::default());
        let empty_email_response = app
            .clone()
            .oneshot(
                Request::get("/api/users/helix/by-email?email=")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let invalid_deal_response = app
            .oneshot(
                Request::get("/api/deals/helix/0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(empty_email_response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(invalid_deal_response.status(), StatusCode::BAD_REQUEST);
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
}
