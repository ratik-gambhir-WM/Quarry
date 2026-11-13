use super::*;
use crate::slide_template_test_support as support;
use std::time::Duration;

async fn json_studio() -> SlideTemplates {
    SlideTemplates::new(
        Arc::new(
            support::repository(std::path::Path::new(":memory:"))
                .await
                .unwrap(),
        ),
        Arc::new(support::FixturePreview),
        None,
        None,
        false,
    )
}

#[tokio::test(flavor = "current_thread")]
async fn json_encoding_is_concurrent_off_runtime_and_retains_capacity_after_request_drop() {
    let studio = Arc::new(json_studio().await);
    let runtime_thread = std::thread::current().id();
    let mut requests = Vec::new();
    let mut releases = Vec::new();
    let mut starts = Vec::new();
    for index in 0..2 {
        let studio = studio.clone();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        releases.push(release);
        starts.push(ready);
        requests.push(tokio::spawn(async move {
            studio
                .encode_json(move || {
                    assert_ne!(std::thread::current().id(), runtime_thread);
                    started.send(()).unwrap();
                    released.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(serde_json::json!({"slide": index}))
                })
                .await
        }));
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        for ready in starts {
            ready.await.unwrap();
        }
    })
    .await
    .expect("both blocking jobs must start while the runtime stays responsive");
    assert_eq!(
        studio.parse_json(b"{}".to_vec()).await.unwrap_err().code,
        "template_busy"
    );
    let cancelled = requests.pop().unwrap();
    cancelled.abort();
    assert!(cancelled.await.unwrap_err().is_cancelled());
    assert_eq!(
        studio.parse_json(b"{}".to_vec()).await.unwrap_err().code,
        "template_busy"
    );
    for release in releases {
        release.send(()).unwrap();
    }
    let bytes = requests.pop().unwrap().await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        serde_json::json!({"slide": 0})
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        while studio.capacity.available_permits() != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        studio.parse_json(bytes).await.unwrap(),
        serde_json::json!({"slide": 0})
    );
}

#[tokio::test]
async fn offloaded_json_preserves_invalid_input_output_bounds_and_shutdown_errors() {
    let studio = json_studio().await;
    assert_eq!(
        studio
            .parse_json(b"invalid".to_vec())
            .await
            .unwrap_err()
            .code,
        "invalid_json"
    );
    let error = studio
        .encode_json(|| Ok("x".repeat(super::super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES)))
        .await
        .unwrap_err();
    assert_eq!(error.code, "response_too_large");
    studio.shutdown();
    assert_eq!(
        studio.parse_json(b"{}".to_vec()).await.unwrap_err().code,
        "template_stopping"
    );
}

#[tokio::test]
async fn committed_slide_job_is_notified_when_the_request_is_cancelled_before_return() {
    let repository = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let classification = Arc::new(SlideClassificationService::new(
        SlideClassificationServiceOptions {
            classifier: Arc::new(support::FixtureClassifier),
            embedder: Arc::new(support::FixtureEmbedder(false)),
            repository: repository.clone(),
            classification_model: "fixture".into(),
            embedding_model: "fixture".into(),
            embedding_dimensions: Some(2),
            embedding_max_text_bytes: 32_000,
            input_limits: Default::default(),
        },
    ));
    let studio = Arc::new(SlideTemplates::new(
        repository.clone(),
        Arc::new(support::FixturePreview),
        Some(classification),
        None,
        false,
    ));
    let (committed, saved) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let request_studio = studio.clone();
    let request = tokio::spawn(async move {
        request_studio
            .run_queued(
                request_studio.worker.as_ref().unwrap().clone(),
                move |import, _, cancellation| async move {
                    let result = import
                        .queue_classified_import(
                            include_bytes!("../../../../fixtures/slide_templates/pptx/styled.pptx"),
                            TemplateKind::Diagram,
                            &cancellation,
                        )
                        .await?;
                    committed.send(result.template_id.clone()).unwrap();
                    released.await.unwrap();
                    Ok(result)
                },
            )
            .await
    });
    let id = saved.await.unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let state = repository
                .find_slide_classification(&id)
                .await
                .unwrap()
                .unwrap();
            if state.embedding_status == "ready" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("saved work must be notified by the publishing task");
    studio.shutdown();
    studio.stop_worker().await;
}

#[test]
fn retrieval_waits_for_the_blocking_pool_without_blocking_the_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).await.unwrap());
        let imported = support::import_fixture(repository.clone(), TemplateKind::Diagram).await;
        let studio = Arc::new(SlideTemplates::new(repository, Arc::new(support::FixturePreview), None, None, false));
        let (release, released) = std::sync::mpsc::channel();
        let (started, ready) = tokio::sync::oneshot::channel();
        let occupied = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        ready.await.unwrap();
        let documents = studio.clone();
        let id = imported.template_id.clone();
        let mut document = tokio::spawn(async move { documents.get(id).await });
        let mut previews = tokio::spawn(async move { studio.previews(1).await });
        // Both requests must yield while their CPU work is queued; this timer must run.
        let queued = tokio::time::timeout(Duration::from_millis(100), async {
            tokio::select! {
                result = &mut document => panic!("document processing bypassed the blocking pool: {result:?}"),
                result = &mut previews => panic!("preview encoding bypassed the blocking pool: {result:?}"),
            }
        }).await;
        release.send(()).unwrap();
        occupied.await.unwrap();
        assert!(queued.is_err());
        let value = tokio::time::timeout(Duration::from_secs(3), document).await.unwrap().unwrap().unwrap().unwrap();
        assert_eq!(value["revision"], 1);
        assert!(value["document"]["presentation"].is_object());
        let previews = tokio::time::timeout(Duration::from_secs(3), previews).await.unwrap().unwrap().unwrap();
        assert_eq!(previews.previews[0].template_id, imported.template_id);
        assert!(previews.previews[0].data_url.starts_with("data:image/png;base64,"));
    });
}
