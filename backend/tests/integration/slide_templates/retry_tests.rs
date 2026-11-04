use super::*;
use std::sync::atomic::AtomicUsize;

struct RetryClassifier {
    calls: Arc<AtomicUsize>,
    fail_first: bool,
    release: Arc<tokio::sync::Semaphore>,
}
impl SlideClassifier for RetryClassifier {
    fn classify<'a>(
        &'a self,
        input: &'a SlideClassificationInput,
    ) -> ProviderFuture<'a, SlideRetrievalMetadata> {
        Box::pin(async move {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_first && call == 0 {
                return Err(SlideProviderError::new("classification_timeout", false));
            }
            self.release.acquire().await.unwrap().forget();
            support::FixtureClassifier.classify(input).await
        })
    }
}
struct RetryEmbedder(Arc<AtomicBool>);
impl SlideEmbedder for RetryEmbedder {
    fn embed<'a>(&'a self, _: &'a str) -> ProviderFuture<'a, Vec<f64>> {
        Box::pin(async move {
            if self.0.load(Ordering::SeqCst) {
                Err(SlideProviderError::new("embedding_timeout", false))
            } else {
                Ok(vec![1.0, 0.5])
            }
        })
    }
}
fn configured(
    repository: Arc<SqliteTemplateRepository>,
    classifier: Arc<dyn SlideClassifier>,
    embedder: Arc<dyn SlideEmbedder>,
) -> Arc<SlideTemplates> {
    let service = Arc::new(SlideClassificationService::new(
        SlideClassificationServiceOptions {
            classifier,
            embedder,
            repository: repository.clone(),
            classification_model: "fixture".into(),
            embedding_model: "fixture".into(),
            embedding_dimensions: Some(2),
            embedding_max_text_bytes: 32_000,
            input_limits: Default::default(),
        },
    ));
    Arc::new(SlideTemplates::new(
        repository,
        Arc::new(support::FixturePreview),
        Some(service),
        None,
        false,
    ))
}
fn retry(id: &str) -> Request<Body> {
    Request::post(format!(
        "/api/v1/templates/slide-templates/catalog/{id}/retrieval/retry"
    ))
    .header("content-type", "application/json")
    .body(Body::from("{}"))
    .unwrap()
}
async fn import(app: &Router) -> String {
    let (status, body) = send(
        app,
        Request::post("/api/v1/templates/slide-templates/import/classified")
            .header("content-type", PPTX)
            .body(Body::from(pptx::write(&document()).unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body["templateId"].as_str().unwrap().to_owned()
}
async fn wait_for(repository: &SqliteTemplateRepository, id: &str, status: &str, embedding: bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let state = repository.find_slide_classification(id).unwrap().unwrap();
            if (if embedding {
                &state.embedding_status
            } else {
                &state.status
            }) == status
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn retry_failed_classification_is_durable_and_does_not_reimport_or_duplicate_work() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let studio = configured(
        repository.clone(),
        Arc::new(RetryClassifier {
            calls: calls.clone(),
            fail_first: true,
            release: release.clone(),
        }),
        Arc::new(support::FixtureEmbedder(false)),
    );
    let app = routes(studio.clone());
    let id = import(&app).await;
    wait_for(&repository, &id, "failed", false).await;
    let preview = repository.find_preview(&id).unwrap().unwrap().bytes;
    let response = app.clone().oneshot(retry(&id)).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"templateId": id, "stage": "classification"})
    );
    assert_eq!(send(&app, retry(&id)).await.0, StatusCode::CONFLICT);
    release.add_permits(1);
    wait_for(&repository, &id, "ready", true).await;
    let state = repository.find_slide_classification(&id).unwrap().unwrap();
    assert_eq!(state.attempt_count, 1);
    assert_eq!(state.embedding_attempt_count, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        repository.find_preview(&id).unwrap().unwrap().bytes,
        preview
    );
    assert_eq!(studio.catalog(None).await.unwrap().templates.len(), 1);
    assert_eq!(send(&app, retry(&id)).await.0, StatusCode::CONFLICT);
    studio.shutdown();
    studio.stop_worker().await;
}

#[tokio::test]
async fn retry_failed_embeddings_preserves_metadata_and_skips_classification() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let fail_embedding = Arc::new(AtomicBool::new(true));
    let studio = configured(
        repository.clone(),
        Arc::new(RetryClassifier {
            calls: calls.clone(),
            fail_first: false,
            release: Arc::new(tokio::sync::Semaphore::new(10)),
        }),
        Arc::new(RetryEmbedder(fail_embedding.clone())),
    );
    let app = routes(studio.clone());
    let id = import(&app).await;
    wait_for(&repository, &id, "failed", true).await;
    let before = repository.find_slide_classification(&id).unwrap().unwrap();
    fail_embedding.store(false, Ordering::SeqCst);
    let (status, result) = send(&app, retry(&id)).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(result["stage"], "embedding");
    wait_for(&repository, &id, "ready", true).await;
    let after = repository.find_slide_classification(&id).unwrap().unwrap();
    assert_eq!(before.metadata, after.metadata);
    assert_eq!(before.classified_at, after.classified_at);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(after.embedding_attempt_count, 1);
    studio.shutdown();
    studio.stop_worker().await;
}

#[tokio::test]
async fn retry_validates_transport_missing_templates_and_configuration() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let studio = configured(
        repository,
        Arc::new(support::FixtureClassifier),
        Arc::new(support::FixtureEmbedder(false)),
    );
    let app = routes(studio.clone());
    assert_eq!(send(&app, retry("missing")).await.0, StatusCode::NOT_FOUND);
    for (id, body) in [
        ("bad!id", "{}"),
        ("missing", "[]"),
        ("missing", "{\"appId\":\"other\"}"),
    ] {
        let request = Request::post(format!(
            "/api/v1/templates/slide-templates/catalog/{id}/retrieval/retry"
        ))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
        assert_eq!(send(&app, request).await.0, StatusCode::BAD_REQUEST);
    }
    let mut request = retry("missing");
    request
        .headers_mut()
        .insert("x-app-id", "OtherApp".parse().unwrap());
    assert_eq!(send(&app, request).await.0, StatusCode::BAD_REQUEST);
    let mut request = retry("missing");
    request
        .headers_mut()
        .insert("content-type", "text/plain".parse().unwrap());
    assert_eq!(
        send(&app, request).await.0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    let mut request = retry("missing");
    *request.body_mut() = Body::from("x".repeat(1025));
    assert_eq!(send(&app, request).await.0, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        send(&routes(support::studio(false)), retry("missing"))
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    studio.shutdown();
    studio.stop_worker().await;
}
