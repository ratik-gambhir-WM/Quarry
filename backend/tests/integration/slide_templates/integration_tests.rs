use crate::{
    domains::templates::{self, slide_templates::*},
    slide_template_test_support as support,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tower::ServiceExt;
const PPTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

#[tokio::test]
async fn compressed_preview_response_finishes_without_repolling_an_exhausted_stream() {
    let response = routes(support::studio(true))
        .oneshot(
            Request::get("/api/v1/templates/previews?page=1")
                .header("accept-encoding", "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-encoding"], "gzip");
    let bytes = to_bytes(response.into_body(), 32 * 1024 * 1024)
        .await
        .unwrap();
    assert!(bytes.starts_with(&[0x1f, 0x8b]));
}

#[tokio::test]
async fn cross_origin_export_exposes_headers_required_by_the_browser_adapter() {
    let response = routes(support::studio(false))
        .oneshot(
            Request::post("/api/v1/templates/export")
                .header("origin", "http://localhost:1420")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&document()).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let exposed = response.headers()["access-control-expose-headers"]
        .to_str()
        .unwrap();
    for required in ["content-disposition", "x-powerpoint-warning-count"] {
        assert!(exposed
            .split(',')
            .any(|header| header.trim().eq_ignore_ascii_case(required)));
    }
    assert!(response.headers().contains_key("content-disposition"));
    assert!(response
        .headers()
        .contains_key("x-powerpoint-warning-count"));
    assert!(to_bytes(response.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap()
        .starts_with(b"PK"));
}

fn routes(studio: Arc<SlideTemplates>) -> Router {
    crate::create_router(
        templates::route::routes(Arc::new(templates::service::TemplateService::embedded(
            studio.clone(),
        )))
        .merge(templates::slide_templates::route::routes(studio)),
        &crate::AppConfig::default().http,
    )
}
async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 60 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn document() -> CanvasDocument {
    pptx::read(include_bytes!(
        "../../fixtures/slide_templates/pptx/styled.pptx"
    ))
    .unwrap()
    .template_json
}
fn multipart(source: &[u8]) -> Vec<u8> {
    let mut data=format!("--test\r\nContent-Disposition: form-data; name=\"files\"; filename=\"template.pptx\"\r\nContent-Type: {PPTX}\r\n\r\n").into_bytes();
    data.extend_from_slice(source);
    data.extend_from_slice(b"\r\n--test--\r\n");
    data
}
#[tokio::test]
async fn existing_facade_import_counts_single_rejection_and_no_legacy_fallback() {
    let studio = support::studio(false);
    let app = routes(studio.clone());
    let source = pptx::write(&document()).unwrap();
    let request = || {
        Request::post("/api/v1/templates/import?mode=single")
            .header("content-type", "multipart/form-data; boundary=test")
            .body(Body::from(multipart(&source)))
            .unwrap()
    };
    let (status, body) = send(&app, request()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["importMode"], "single");
    assert_eq!(body["importedCount"], 1);
    assert_eq!(body["warningCount"], 1);
    assert_eq!(body.as_object().unwrap().len(), 3);
    let mut deck = document();
    deck.presentation
        .slides
        .push(deck.presentation.slides[0].clone());
    let source = pptx::write(&deck).unwrap();
    let (status, body) = send(
        &app,
        Request::post("/api/v1/templates/import?mode=single")
            .header("content-type", "multipart/form-data; boundary=test")
            .body(Body::from(multipart(&source)))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["error"],
        templates::model::PPTX_MULTI_SLIDE_IMPORT_MESSAGE
    );
    assert_eq!(studio.catalog(None).await.unwrap().templates.len(), 1);
    let (status, body) = send(
        &app,
        Request::post("/api/v1/templates/import?mode=batch")
            .header("content-type", "multipart/form-data; boundary=test")
            .body(Body::from(multipart(&source)))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["importedCount"], 2);
    assert_eq!(studio.catalog(None).await.unwrap().templates.len(), 3);
    // A native static route must not resolve as the facade's template ID route.
    let (status, body) = send(
        &app,
        Request::get("/api/v1/templates/slide-templates/catalog")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["templates"].as_array().unwrap().len(), 3);
}
#[tokio::test]
async fn native_routes_reject_scope_selection_duplicate_queries_and_real_declared_limits() {
    let app = routes(support::studio(false));
    for uri in [
        "/api/v1/templates/slide-templates/catalog?appId=Other",
        "/api/v1/templates/slide-templates/catalog/previews?page=0",
        "/api/v1/templates/slide-templates/catalog/previews?page=101",
        "/api/v1/templates/slide-templates/catalog/previews?page=1&page=2",
        "/api/v1/templates/slide-templates/catalog/bad%2Fid",
    ] {
        assert_eq!(
            send(&app, Request::get(uri).body(Body::empty()).unwrap())
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        send(
            &app,
            Request::get("/api/v1/templates/slide-templates/catalog")
                .header("x-app-id", "Other")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let uri = "/api/v1/templates/slide-templates/import";
    assert_eq!(
        send(
            &app,
            Request::post(uri)
                .header("content-type", PPTX)
                .header("content-length", 26 * 1024 * 1024)
                .body(Body::empty())
                .unwrap()
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        send(
            &app,
            Request::post(uri)
                .header("content-type", PPTX)
                .header("content-encoding", "gzip")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert_eq!(
        send(
            &app,
            Request::post("/api/v1/templates/slide-templates/import/classified")
                .header("content-type", PPTX)
                .body(Body::from(pptx::write(&document()).unwrap()))
                .unwrap()
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
struct SlowPreview {
    started: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}
impl TemplatePreviewGenerator for SlowPreview {
    fn generate(&self, _: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        Ok(None)
    }
    fn generate_document_cancellable(
        &self,
        _: &[u8],
        _: &CanvasDocument,
        c: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        self.started.store(true, Ordering::Release);
        while !c.is_some_and(RequestCancellation::is_cancelled) {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.finished.store(true, Ordering::Release);
        Ok(None)
    }
}
#[tokio::test]
async fn dropping_request_cancels_blocking_preview_and_prevents_publication() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let started = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let studio = Arc::new(SlideTemplates::new(
        repository.clone(),
        Arc::new(SlowPreview {
            started: started.clone(),
            finished: finished.clone(),
        }),
        None,
        None,
        false,
    ));
    let source = pptx::write(&document()).unwrap();
    let task = tokio::spawn(async move { studio.import(source, TemplateKind::Diagram).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !started.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while !finished.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(repository.list(None).unwrap().is_empty());
}

#[tokio::test]
async fn projection_failure_preserves_the_canonical_import_without_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("quarry.sqlite3");
    let repository = Arc::new(support::repository(&path).unwrap());
    let helix = Arc::new(
        crate::adapters::helix::client::HelixClient::with_config("http://127.0.0.1:9", None)
            .unwrap(),
    );
    let index = Arc::new(index::TemplateIndex::new(
        helix,
        std::num::NonZeroUsize::new(2).unwrap(),
    ));
    let studio = SlideTemplates::new(
        repository.clone(),
        Arc::new(DisabledTemplatePreviewGenerator),
        None,
        Some(index),
        false,
    );
    let error = studio
        .import(pptx::write(&document()).unwrap(), TemplateKind::Diagram)
        .await
        .unwrap_err();
    assert_eq!(error.code, "template_projection_failed");
    let saved = repository.list(None).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        studio.reconcile().await.unwrap_err().code,
        "template_projection_failed"
    );
    assert_eq!(repository.list(None).unwrap(), saved);
    drop(studio);
    drop(repository);
    let reopened = support::repository(&path).unwrap();
    assert_eq!(reopened.list(None).unwrap(), saved);
}

#[tokio::test]
async fn classified_http_waits_for_both_embeddings_and_keeps_failed_artifacts_unpublished() {
    for failure in [false, true] {
        let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
        let classifier = Arc::new(SlideClassificationService::new(
            SlideClassificationServiceOptions {
                classifier: Arc::new(support::FixtureClassifier),
                classification_model: "fixture-classifier".into(),
                embedder: Arc::new(support::FixtureEmbedder(failure)),
                embedding_dimensions: Some(2),
                embedding_max_text_bytes: 32_000,
                embedding_model: "fixture-embedder".into(),
                input_limits: Default::default(),
                repository: repository.clone(),
            },
        ));
        let app = routes(Arc::new(SlideTemplates::new(
            repository.clone(),
            Arc::new(DisabledTemplatePreviewGenerator),
            Some(classifier),
            None,
            true,
        )));
        let (status, body) = send(
            &app,
            Request::post("/api/v1/templates/slide-templates/import/classified")
                .header("content-type", PPTX)
                .body(Body::from(pptx::write(&document()).unwrap()))
                .unwrap(),
        )
        .await;
        assert_eq!(
            status,
            if failure {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::CREATED
            }
        );
        let id = repository.list(None).unwrap()[0]
            .template
            .template_id
            .clone();
        if failure {
            // Failure recording is offloaded on cancellation/error; no successful artifacts may be visible.
            let state = repository.find_slide_classification(&id).unwrap().unwrap();
            assert!(state.metadata.is_none());
            assert!(state.subject_vector.is_none());
            assert!(state.capability_vector.is_none());
            assert!(repository
                .search_slide_classifications("Quarry_WestMonroe", "cloud", 10)
                .unwrap()
                .is_empty());
        } else {
            assert_eq!(body["retrieval"]["status"], "ready");
            let state = repository.find_slide_classification(&id).unwrap().unwrap();
            assert_eq!(state.embedding_status, "ready");
            assert_eq!(state.subject_vector.as_ref().unwrap().len(), 2);
            assert_eq!(state.capability_vector.as_ref().unwrap().len(), 2);
            let (status,body)=send(&app,Request::get(format!("/api/v1/templates/slide-templates/catalog/{id}/retrieval?metadata=true&embeddings=true")).body(Body::empty()).unwrap()).await;
            assert_eq!(status, StatusCode::OK);
            assert!(body["metadata"].is_object());
            assert_eq!(body["retrievalStatus"], "ready");
            assert_eq!(body["embeddings"]["status"], "ready");
        }
    }
}

#[tokio::test]
async fn shutdown_cancels_owned_preview_and_rejects_new_work() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let started = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let studio = Arc::new(SlideTemplates::new(
        repository.clone(),
        Arc::new(SlowPreview {
            started: started.clone(),
            finished: finished.clone(),
        }),
        None,
        None,
        false,
    ));
    let worker = studio.clone();
    let source = pptx::write(&document()).unwrap();
    let task = tokio::spawn(async move { worker.import(source, TemplateKind::Diagram).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !started.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    studio.shutdown();
    assert!(task.await.unwrap().is_err());
    assert!(finished.load(Ordering::Acquire));
    assert!(repository.list(None).unwrap().is_empty());
    assert_eq!(
        studio.get("absent".into()).await.unwrap_err().code,
        "template_stopping"
    );
}

#[tokio::test]
async fn native_import_checks_actual_bytes_even_without_declared_length() {
    let app = routes(support::studio(false));
    let body = Body::from(vec![0; 25 * 1024 * 1024 + 1]);
    assert_eq!(
        send(
            &app,
            Request::post("/api/v1/templates/slide-templates/import")
                .header("content-type", PPTX)
                .body(body)
                .unwrap()
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn admission_is_shared_by_facade_and_native_routes_and_held_until_body_drop() {
    let app = routes(support::studio(false)).route(
        "/api/v1/health",
        axum::routing::get(|| async { StatusCode::OK }),
    );
    let mut held = Vec::new();
    for path in [
        "/api/v1/templates/previews",
        "/api/v1/templates/slide-templates/catalog",
        "/api/v1/templates/previews",
        "/api/v1/templates/slide-templates/catalog",
    ] {
        let response = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        held.push(response);
    }
    assert_eq!(
        send(
            &app,
            Request::get("/api/v1/templates/previews")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        send(
            &app,
            Request::get("/api/v1/health").body(Body::empty()).unwrap()
        )
        .await
        .0,
        StatusCode::OK
    );
    held.pop();
    assert_eq!(
        send(
            &app,
            Request::get("/api/v1/templates/previews")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[test]
fn native_preview_maps_raw_powerpoint_to_a_document_without_domain_imports_in_the_adapter() {
    struct DocumentPreview;
    impl crate::adapters::slide_templates::preview::TemplatePreviewGenerator for DocumentPreview {
        fn requires_document(&self) -> bool {
            true
        }
        fn generate(&self, _: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
            anyhow::bail!("document required")
        }
        fn generate_document(
            &self,
            _: &[u8],
            document: &Value,
        ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
            assert!(document["presentation"]["slides"].is_array());
            Ok(None)
        }
    }
    let preview = NativePreview(Arc::new(DocumentPreview));
    assert!(preview
        .generate(include_bytes!(
            "../../fixtures/slide_templates/pptx/styled.pptx"
        ))
        .unwrap()
        .is_none());
}
