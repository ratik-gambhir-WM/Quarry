use super::*;
use std::sync::{Condvar, Mutex};

fn deck(count: usize) -> Vec<u8> {
    let mut document = document();
    let slide = document.presentation.slides[0].clone();
    document.presentation.slides = (0..count)
        .map(|index| {
            let mut slide = slide.clone();
            slide.id = format!("slide-{index}");
            for element in &mut slide.elements {
                if let Element::Text(text) = element {
                    text.text = format!("Batch slide {}", index + 1);
                    text.runs.clear();
                    break;
                }
            }
            slide
        })
        .collect();
    pptx::write(&document).unwrap()
}

#[derive(Default)]
struct ParallelPreview {
    started: Mutex<usize>,
    wake: Condvar,
    fail_title: Option<String>,
}
impl TemplatePreviewGenerator for ParallelPreview {
    fn generate(&self, _: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        unreachable!("batch previews must use the slide document")
    }
    fn generate_document_cancellable(
        &self,
        _: &[u8],
        document: &CanvasDocument,
        _: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        assert_eq!(document.presentation.slides.len(), 1);
        let mut started = self.started.lock().unwrap();
        *started += 1;
        self.wake.notify_all();
        let (started, timeout) = self
            .wake
            .wait_timeout_while(started, Duration::from_secs(3), |started| *started < 4)
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "four slides must prepare concurrently"
        );
        drop(started);
        if self.fail_title.as_ref() == Some(&document.presentation.title) {
            return Ok(None);
        }
        support::FixturePreview.generate(&[])
    }
}

struct BatchClassifier {
    entered: Arc<tokio::sync::Semaphore>,
    release: Arc<tokio::sync::Semaphore>,
}
impl SlideClassifier for BatchClassifier {
    fn classify<'a>(
        &'a self,
        input: &'a SlideClassificationInput,
    ) -> ProviderFuture<'a, SlideRetrievalMetadata> {
        Box::pin(async move {
            assert!(input.preview.is_some());
            self.entered.add_permits(1);
            self.release.acquire().await.unwrap().forget();
            support::FixtureClassifier.classify(input).await
        })
    }
}
struct BatchEmbedder {
    entered: Arc<tokio::sync::Semaphore>,
    release: Arc<tokio::sync::Semaphore>,
}
impl SlideEmbedder for BatchEmbedder {
    fn embed<'a>(&'a self, _: &'a str) -> ProviderFuture<'a, Vec<f64>> {
        Box::pin(async move {
            self.entered.add_permits(1);
            self.release.acquire().await.unwrap().forget();
            Ok(vec![1.0, 0.5])
        })
    }
}
fn service(
    repository: Arc<SqliteTemplateRepository>,
    classifier: Arc<dyn SlideClassifier>,
    embedder: Arc<dyn SlideEmbedder>,
) -> Arc<SlideClassificationService> {
    Arc::new(SlideClassificationService::new(
        SlideClassificationServiceOptions {
            classifier,
            classification_model: "fixture".into(),
            embedder,
            embedding_dimensions: Some(2),
            embedding_max_text_bytes: 32_000,
            embedding_model: "fixture".into(),
            input_limits: Default::default(),
            repository,
        },
    ))
}

#[tokio::test]
async fn batch_classified_saves_ordered_deck_before_parallel_background_stages_finish() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let class_entered = Arc::new(tokio::sync::Semaphore::new(0));
    let class_release = Arc::new(tokio::sync::Semaphore::new(0));
    let embed_entered = Arc::new(tokio::sync::Semaphore::new(0));
    let embed_release = Arc::new(tokio::sync::Semaphore::new(0));
    let studio = Arc::new(SlideTemplates::new(
        repository.clone(),
        Arc::new(ParallelPreview::default()),
        Some(service(
            repository.clone(),
            Arc::new(BatchClassifier {
                entered: class_entered.clone(),
                release: class_release.clone(),
            }),
            Arc::new(BatchEmbedder {
                entered: embed_entered.clone(),
                release: embed_release.clone(),
            }),
        )),
        None,
        true,
    ));
    let app = routes(studio.clone());
    let source = deck(12);
    let expected = pptx::read(&source).unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        app.clone().oneshot(
            Request::post(
                "/api/v1/templates/slide-templates/batch-import/classified?kind=commentary",
            )
            .header("content-type", PPTX)
            .body(Body::from(source))
            .unwrap(),
        ),
    )
    .await
    .expect("acknowledgement cannot wait for classification")
    .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["x-imported-template-count"], "12");
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let body: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 60 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["importedCount"], 12);
    assert_eq!(body["retrieval"]["status"], "pending");
    assert_eq!(repository.list(None).unwrap().len(), 12);
    let templates = body["templates"].as_array().unwrap();
    assert_eq!(templates.len(), 12);
    for (template, expected) in templates
        .iter()
        .zip(expected.template_json.presentation.slides)
    {
        let id = template["templateId"].as_str().unwrap();
        assert_eq!(template["previewAvailable"], true);
        assert_eq!(
            template["templateJson"]["presentation"]["title"],
            expected.name
        );
        assert!(repository.find_by_id(id).unwrap().is_some());
        assert!(repository.find_preview(id).unwrap().is_some());
        assert_eq!(
            repository.find_metadata(id).unwrap().unwrap().kind,
            TemplateKind::Commentary
        );
        let state = repository.find_slide_classification(id).unwrap().unwrap();
        assert!(state.metadata.is_none());
        assert!(state.subject_vector.is_none());
    }
    // A single worker cannot enter four blocked classifiers at once.
    tokio::time::timeout(Duration::from_secs(3), class_entered.acquire_many(4))
        .await
        .unwrap()
        .unwrap()
        .forget();
    class_release.add_permits(12);
    tokio::time::timeout(Duration::from_secs(3), embed_entered.acquire_many(4))
        .await
        .unwrap()
        .unwrap()
        .forget();
    embed_release.add_permits(24);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if templates.iter().all(|template| {
                let state = repository
                    .find_slide_classification(template["templateId"].as_str().unwrap())
                    .unwrap()
                    .unwrap();
                state.status == "ready"
                    && state.embedding_status == "ready"
                    && state.subject_vector.is_some()
                    && state.capability_vector.is_some()
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    studio.shutdown();
    studio.stop_worker().await;
}

#[tokio::test]
async fn batch_classified_preview_failure_does_not_publish_partial_deck_or_jobs() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let source = deck(12);
    let fail_title = pptx::read(&source)
        .unwrap()
        .template_json
        .presentation
        .slides[5]
        .name
        .clone();
    let studio = Arc::new(SlideTemplates::new(
        repository.clone(),
        Arc::new(ParallelPreview {
            fail_title: Some(fail_title),
            ..Default::default()
        }),
        Some(service(
            repository.clone(),
            Arc::new(support::FixtureClassifier),
            Arc::new(support::FixtureEmbedder(false)),
        )),
        None,
        false,
    ));
    let (status, body) = send(
        &routes(studio),
        Request::post("/api/v1/templates/slide-templates/batch-import/classified")
            .header("content-type", PPTX)
            .body(Body::from(source))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body["error"],
        "A preview could not be generated. Try importing the template again."
    );
    assert!(repository.list(None).unwrap().is_empty());
    assert!(repository
        .claim_next_slide_classification("2099-01-01T00:00:00Z", "2099-01-01T00:03:00Z")
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn batch_classified_validates_inputs_and_capability_without_writes() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let classifier = service(
        repository.clone(),
        Arc::new(support::FixtureClassifier),
        Arc::new(support::FixtureEmbedder(false)),
    );
    let app = routes(Arc::new(SlideTemplates::new(
        repository.clone(),
        Arc::new(support::FixturePreview),
        Some(classifier),
        None,
        false,
    )));
    for (suffix, mime, source, expected) in [
        ("?kind=unknown", PPTX, deck(1), StatusCode::BAD_REQUEST),
        ("?appId=other", PPTX, deck(1), StatusCode::BAD_REQUEST),
        (
            "",
            "application/json",
            deck(1),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        ("", PPTX, Vec::new(), StatusCode::BAD_REQUEST),
        ("", PPTX, b"invalid".to_vec(), StatusCode::BAD_REQUEST),
        ("", PPTX, deck(101), StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        assert_eq!(
            send(
                &app,
                Request::post(format!(
                    "/api/v1/templates/slide-templates/batch-import/classified{suffix}"
                ))
                .header("content-type", mime)
                .body(Body::from(source))
                .unwrap()
            )
            .await
            .0,
            expected
        );
    }
    assert!(repository.list(None).unwrap().is_empty());
    let disabled = routes(support::studio());
    assert_eq!(
        send(
            &disabled,
            Request::post("/api/v1/templates/slide-templates/batch-import/classified")
                .header("content-type", PPTX)
                .body(Body::from(deck(1)))
                .unwrap()
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn batch_classified_request_cancellation_stops_previews_without_partial_publication() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
    let started = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let studio = Arc::new(SlideTemplates::new(
        repository.clone(),
        Arc::new(SlowPreview {
            started: started.clone(),
            finished: finished.clone(),
        }),
        Some(service(
            repository.clone(),
            Arc::new(support::FixtureClassifier),
            Arc::new(support::FixtureEmbedder(false)),
        )),
        None,
        false,
    ));
    let source = deck(12);
    let active = studio.clone();
    let request =
        tokio::spawn(async move { active.batch_classified(source, TemplateKind::Diagram).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !started.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(3), async {
        while !finished.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // Wait for owned blocking workers to finish so no delayed publication escapes this assertion.
    studio.shutdown();
    studio.stop_worker().await;
    assert!(repository.list(None).unwrap().is_empty());
    assert!(repository
        .claim_next_slide_classification("2099-01-01T00:00:00Z", "2099-01-01T00:03:00Z")
        .unwrap()
        .is_none());
}
