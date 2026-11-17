use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Extension,
};
use tower::ServiceExt;

#[tokio::test]
async fn unconfigured_engine_preserves_product_routes_without_mounting_native_routes() {
    let api = routes(Arc::new(TemplateService::new(None)));
    let response = api
        .clone()
        .oneshot(
            Request::get("/templates/previews")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let app = crate::create_router(
        api.layer(Extension(crate::integration_support::test_actor())),
        &crate::AppConfig::default().http,
    );
    for prefix in ["/api/v1", "/api"] {
        for (path, status) in [
            ("/templates/previews", StatusCode::SERVICE_UNAVAILABLE),
            ("/templates/slide-templates/catalog", StatusCode::NOT_FOUND),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::get(format!("{prefix}{path}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status, "{prefix}{path}");
        }
    }
}
