use super::*;
use crate::slide_template_test_support as support;
use std::time::Duration;

#[tokio::test]
async fn committed_slide_job_is_notified_when_the_request_is_cancelled_before_return() {
    let repository = Arc::new(support::repository(std::path::Path::new(":memory:")).unwrap());
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
    let (release, released) = std::sync::mpsc::channel();
    let request_studio = studio.clone();
    let request = tokio::spawn(async move {
        request_studio
            .run_queued(
                request_studio.worker.as_ref().unwrap().clone(),
                move |import, _, cancellation| {
                    let result = import.queue_classified_import(
                        include_bytes!("../../../../fixtures/slide_templates/pptx/styled.pptx"),
                        TemplateKind::Diagram,
                        &cancellation,
                    )?;
                    committed.send(result.template_id.clone()).unwrap();
                    released.recv().unwrap();
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
            let state = repository.find_slide_classification(&id).unwrap().unwrap();
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
