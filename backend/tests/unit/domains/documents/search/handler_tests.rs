use super::*;
#[test]
fn hybrid_rejects_unknown_scope_provider_and_vector_fields() {
    for extra in ["dealId", "userId", "model", "queryEmbedding", "dimensions"] {
        let mut input = serde_json::json!({"queryText":"Question","limit":30});
        input[extra] = serde_json::json!("untrusted");
        assert!(serde_json::from_value::<super::super::model::HybridSearch>(input).is_err());
    }
    assert!(validate_common(0).is_err());
    assert!(validate_common(101).is_err());
}

#[tokio::test]
async fn hybrid_http_validates_text_limits_path_and_unknown_fields_before_retrieval() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let app = crate::integration_support::authenticated_test_application()
        .await
        .unwrap()
        .router;
    for (path, body) in [
        (
            "/api/v1/documents/search/hybrid",
            serde_json::json!({"queryText":"  ","limit":30}),
        ),
        (
            "/api/v1/documents/search/hybrid",
            serde_json::json!({"queryText":"é".repeat(1001),"limit":30}),
        ),
        (
            "/api/v1/documents/search/hybrid",
            serde_json::json!({"queryText":"q","limit":0}),
        ),
        (
            "/api/v1/documents/search/hybrid",
            serde_json::json!({"queryText":"q","limit":101}),
        ),
        (
            "/api/v1/documents/search/hybrid",
            serde_json::json!({"queryText":"q","limit":30,"dealId":"DEAL-1"}),
        ),
        (
            "/api/v1/deals/%20/documents/search/hybrid",
            serde_json::json!({"queryText":"q","limit":30}),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    for (path, status) in [
        (
            "/api/v1/documents/search/hybrid",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "/api/v1/deals/DEAL-1/documents/search/hybrid",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "/api/v1/deals/DEAL-OTHER/documents/search/hybrid",
            StatusCode::NOT_FOUND,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"queryText":"Question","limit":30}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    let unauthenticated = crate::integration_support::test_application()
        .await
        .unwrap()
        .router;
    let response = unauthenticated
        .oneshot(
            Request::post("/api/v1/documents/search/hybrid")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"queryText":"q","limit":30}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn hybrid_http_trims_once_and_missing_provider_is_explicit_without_graph_reads() {
    use crate::{
        adapters::helix::client::HelixClient,
        domains::{
            documents::{
                graph::repository::DocumentGraphSearch,
                search::{
                    embedding::{EmbeddingProvider, QueryEmbeddingCache},
                    service::DocumentSearchService,
                },
            },
            identity::access::ResourceAccess,
            memberships::{repository::MembershipRepository, service::DealAccessService},
        },
    };
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        Extension,
    };
    use futures_util::future::BoxFuture;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;
    struct Provider(Mutex<Vec<String>>);
    impl EmbeddingProvider for Provider {
        fn embed<'a>(
            &'a self,
            q: &'a str,
        ) -> BoxFuture<'a, crate::shared::error::ServiceResult<Vec<f32>>> {
            Box::pin(async move {
                self.0.lock().unwrap().push(q.into());
                Ok(vec![1.0, 0.0])
            })
        }
    }
    let application = crate::integration_support::authenticated_test_application()
        .await
        .unwrap();
    let access = ResourceAccess::new(
        application.postgres.clone(),
        Arc::new(DealAccessService::new(MembershipRepository::new(
            application.postgres,
        ))),
    );
    let provider = Arc::new(Provider(Mutex::new(Vec::new())));
    let cache = Arc::new(
        QueryEmbeddingCache::new(provider.clone(), 4, std::time::Duration::from_secs(1)).unwrap(),
    );
    let service = DocumentSearchService::new(DocumentGraphSearch::new(Arc::new(
        HelixClient::with_config("http://127.0.0.1:1", None).unwrap(),
    )))
    .with_access(access)
    .with_hybrid(
        crate::app::config::SearchConfig {
            enabled: true,
            ..Default::default()
        },
        Some(cache),
    );
    let missing = service.clone().with_hybrid(
        crate::app::config::SearchConfig {
            enabled: true,
            ..Default::default()
        },
        None,
    );
    let app = super::super::route::routes(Arc::new(service))
        .layer(Extension(crate::integration_support::test_actor()));
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::post("/deals/DEAL-1/documents/search/hybrid")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"queryText":"  C++ Query?  ","limit":30}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &to_bytes(response.into_body(), 1024).await.unwrap()
            )
            .unwrap(),
            serde_json::json!({"results":[]})
        );
    }
    assert_eq!(*provider.0.lock().unwrap(), vec!["C++ Query?"]);
    let app = super::super::route::routes(Arc::new(missing))
        .layer(Extension(crate::integration_support::test_actor()));
    let response = app
        .oneshot(
            Request::post("/deals/DEAL-1/documents/search/hybrid")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"queryText":"q","limit":30}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert!(std::str::from_utf8(&body)
        .unwrap()
        .contains("semantic search unavailable"));
}
