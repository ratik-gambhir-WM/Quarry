use super::*;
use crate::{domains::templates::slide_templates::*, slide_template_test_support as support};
use std::sync::atomic::AtomicUsize;

struct RetryingClassifier {
    calls: Arc<AtomicUsize>,
    failures: usize,
    retryable: bool,
}
impl SlideClassifier for RetryingClassifier {
    fn classify<'a>(
        &'a self,
        input: &'a SlideClassificationInput,
    ) -> ProviderFuture<'a, SlideRetrievalMetadata> {
        Box::pin(async move {
            assert!(input.preview.is_some());
            if self.calls.fetch_add(1, Ordering::SeqCst) < self.failures {
                return Err(SlideProviderError::new(
                    "classification_unavailable",
                    self.retryable,
                ));
            }
            support::FixtureClassifier.classify(input).await
        })
    }
}
struct RetryingEmbedder {
    calls: Arc<AtomicUsize>,
    failures: usize,
    hang: bool,
}
impl SlideEmbedder for RetryingEmbedder {
    fn embed<'a>(&'a self, _: &'a str) -> ProviderFuture<'a, Vec<f64>> {
        Box::pin(async move {
            let attempt = self.calls.fetch_add(1, Ordering::SeqCst);
            if self.hang {
                std::future::pending::<()>().await;
            }
            if attempt < self.failures {
                Err(SlideProviderError::new("embedding_timeout", true))
            } else {
                Ok(vec![1.0, 0.5])
            }
        })
    }
}
fn worker(
    repository: Arc<PostgresTemplateRepository>,
    classifier: Arc<dyn SlideClassifier>,
    embedder: Arc<dyn SlideEmbedder>,
) -> Arc<SlideClassificationWorker> {
    let service = Arc::new(SlideClassificationService::new(
        SlideClassificationServiceOptions {
            classifier,
            classification_model: "fixture".into(),
            embedder,
            embedding_dimensions: Some(2),
            embedding_max_text_bytes: 32_000,
            embedding_model: "fixture".into(),
            input_limits: Default::default(),
            repository: repository.clone(),
        },
    ));
    Arc::new(SlideClassificationWorker::new(
        SlideClassificationWorkerOptions {
            classification_max_attempts: 3,
            classification_timeout: Duration::from_millis(100),
            concurrency: 1,
            embedding_max_attempts: 3,
            embedding_timeout: Duration::from_millis(30),
            lease_padding: Duration::from_millis(100),
            retry_base: Duration::ZERO,
            now: Arc::new(Utc::now),
            random: Arc::new(|| 0.0),
            import: Arc::new(ImportService::new(repository.clone())),
            projection: None,
            mutations: Arc::new(tokio::sync::Mutex::new(())),
            repository,
            service,
        },
    ))
}
async fn queue(repository: Arc<PostgresTemplateRepository>) -> String {
    ImportService::with_preview_generator(repository, Arc::new(support::FixturePreview))
        .queue_classified_import(
            include_bytes!("../../../../../fixtures/slide_templates/pptx/styled.pptx"),
            TemplateKind::Diagram,
            &RequestCancellation::default(),
        )
        .await
        .unwrap()
        .template_id
}
async fn terminal(repository: &PostgresTemplateRepository, id: &str) -> StoredSlideClassification {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let state = repository
                .find_slide_classification(id)
                .await
                .unwrap()
                .unwrap();
            if state.status == "failed"
                || matches!(state.embedding_status.as_str(), "ready" | "failed")
            {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn retries_stages_independently_and_preserves_classification_on_embedding_retry() {
    let repo = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let id = queue(repo.clone()).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let worker = worker(
        repo.clone(),
        Arc::new(RetryingClassifier {
            calls: calls.clone(),
            failures: 1,
            retryable: true,
        }),
        Arc::new(RetryingEmbedder {
            calls: Arc::new(AtomicUsize::new(0)),
            failures: 1,
            hang: false,
        }),
    );
    worker.notify();
    let state = terminal(&repo, &id).await;
    worker.stop(Duration::from_secs(1)).await;
    assert_eq!(state.status, "ready");
    assert_eq!(state.attempt_count, 2);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(state.embedding_status, "ready");
    assert_eq!(state.embedding_attempt_count, 2);
    assert!(state.subject_vector.is_some());
    assert!(state.capability_vector.is_some());
    assert!(state.last_error_code.is_none());
    assert!(state.embedding_last_error_code.is_none());
}
#[tokio::test]
async fn exhausted_classification_retries_and_nonretryable_errors_are_durable() {
    for retryable in [true, false] {
        let repo = Arc::new(
            support::repository(std::path::Path::new(":memory:"))
                .await
                .unwrap(),
        );
        let id = queue(repo.clone()).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let worker = worker(
            repo.clone(),
            Arc::new(RetryingClassifier {
                calls: calls.clone(),
                failures: usize::MAX,
                retryable,
            }),
            Arc::new(support::FixtureEmbedder(false)),
        );
        worker.notify();
        let state = terminal(&repo, &id).await;
        worker.stop(Duration::from_secs(1)).await;
        assert_eq!(state.status, "failed");
        assert_eq!(state.attempt_count, if retryable { 3 } else { 1 });
        assert_eq!(
            state.last_error_code.as_deref(),
            Some("classification_unavailable")
        );
        assert!(state.next_attempt_at.is_none());
        assert!(state.metadata.is_none());
        assert!(repo.find_preview(&id).await.unwrap().is_some());
        assert!(repo.find_by_id(&id).await.unwrap().is_some());
    }
}
#[tokio::test]
async fn embedding_timeout_retries_are_bounded_without_reclassifying() {
    let repo = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let id = queue(repo.clone()).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let worker = worker(
        repo.clone(),
        Arc::new(RetryingClassifier {
            calls: calls.clone(),
            failures: 0,
            retryable: true,
        }),
        Arc::new(RetryingEmbedder {
            calls: Arc::new(AtomicUsize::new(0)),
            failures: 0,
            hang: true,
        }),
    );
    worker.notify();
    let state = terminal(&repo, &id).await;
    worker.stop(Duration::from_secs(1)).await;
    assert_eq!(state.status, "ready");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(state.metadata.is_some());
    assert_eq!(state.embedding_status, "failed");
    assert_eq!(state.embedding_attempt_count, 3);
    assert_eq!(
        state.embedding_last_error_code.as_deref(),
        Some("embedding_timeout")
    );
    assert!(state.subject_vector.is_none());
    assert!(state.capability_vector.is_none());
    assert!(repo.find_preview(&id).await.unwrap().is_some());
}
#[tokio::test]
async fn worker_recovers_durable_pending_jobs_and_expired_leases_after_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("templates.sqlite");
    let (pending, expired) = {
        let repo = Arc::new(support::repository(&path).await.unwrap());
        let first = queue(repo.clone()).await;
        repo.claim_next_slide_classification(
            "2020-01-01T00:00:00.000Z",
            "2020-01-01T00:01:00.000Z",
        )
        .await
        .unwrap()
        .unwrap();
        let second = queue(repo).await;
        (second, first)
    };
    let repo = Arc::new(support::repository(&path).await.unwrap());
    let worker = worker(
        repo.clone(),
        Arc::new(support::FixtureClassifier),
        Arc::new(support::FixtureEmbedder(false)),
    );
    worker.notify();
    assert_eq!(terminal(&repo, &pending).await.embedding_status, "ready");
    let state = terminal(&repo, &expired).await;
    assert_eq!(state.embedding_status, "ready");
    assert_eq!(state.attempt_count, 2);
    worker.stop(Duration::from_secs(1)).await;
}
#[tokio::test]
async fn stopping_cancels_provider_and_leaves_retryable_work_for_restart() {
    let repo = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let id = queue(repo.clone()).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let first = worker(
        repo.clone(),
        Arc::new(support::FixtureClassifier),
        Arc::new(RetryingEmbedder {
            calls: calls.clone(),
            failures: 0,
            hang: true,
        }),
    );
    first.notify();
    tokio::time::timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    first.stop(Duration::ZERO).await;
    let saved = repo.find_slide_classification(&id).await.unwrap().unwrap();
    assert_eq!(saved.status, "ready");
    assert_eq!(saved.embedding_status, "pending");
    let next = worker(
        repo.clone(),
        Arc::new(support::FixtureClassifier),
        Arc::new(support::FixtureEmbedder(false)),
    );
    next.notify();
    assert_eq!(terminal(&repo, &id).await.embedding_status, "ready");
    next.stop(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn saved_preview_outside_provider_bounds_fails_explicitly_without_text_only_fallback() {
    let repo = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let id = queue(repo.clone()).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let mut worker = worker(
        repo.clone(),
        Arc::new(RetryingClassifier {
            calls: calls.clone(),
            failures: 0,
            retryable: true,
        }),
        Arc::new(support::FixtureEmbedder(false)),
    );
    Arc::get_mut(&mut Arc::get_mut(&mut worker).unwrap().options.service)
        .unwrap()
        .options
        .input_limits
        .max_preview_bytes = 0;
    worker.notify();
    let state = terminal(&repo, &id).await;
    worker.stop(Duration::from_secs(1)).await;
    assert_eq!(state.status, "failed");
    assert_eq!(
        state.last_error_code.as_deref(),
        Some("classification_preview_unavailable")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(repo.find_preview(&id).await.unwrap().is_some());
}

async fn idle(worker: &Arc<SlideClassificationWorker>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while Arc::strong_count(worker) != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("empty queues must not retain a polling coordinator");
}

#[tokio::test]
async fn notification_drains_then_retires_and_a_later_import_starts_a_new_task() {
    let repo = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let worker = worker(
        repo.clone(),
        Arc::new(support::FixtureClassifier),
        Arc::new(support::FixtureEmbedder(false)),
    );
    assert_eq!(Arc::strong_count(&worker), 1, "construction must not spawn");
    worker.notify(); // Empty startup recovery.
    idle(&worker).await;
    for _ in 0..2 {
        let id = queue(repo.clone()).await;
        for _ in 0..20 {
            worker.notify();
        }
        let state = terminal(&repo, &id).await;
        assert_eq!(state.embedding_status, "ready");
        assert_eq!(
            state.attempt_count, 1,
            "duplicate wakes must not duplicate a claim"
        );
        assert_eq!(state.embedding_attempt_count, 1);
        idle(&worker).await;
    }
    worker.stop(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn future_leases_are_recovered_without_another_notification() {
    let repo = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let id = queue(repo.clone()).await;
    repo.claim_next_slide_classification(
        &iso(Utc::now()),
        &iso(Utc::now() + chrono::Duration::milliseconds(100)),
    )
    .await
    .unwrap()
    .unwrap();
    let worker = worker(
        repo.clone(),
        Arc::new(support::FixtureClassifier),
        Arc::new(support::FixtureEmbedder(false)),
    );
    worker.notify();
    let state = terminal(&repo, &id).await;
    assert_eq!(state.embedding_status, "ready");
    assert_eq!(state.attempt_count, 2);
    idle(&worker).await;
    worker.stop(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn delayed_provider_retries_continue_after_the_first_task_finishes() {
    let repo = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let id = queue(repo.clone()).await;
    let mut worker = worker(
        repo.clone(),
        Arc::new(RetryingClassifier {
            calls: Arc::new(AtomicUsize::new(0)),
            failures: 1,
            retryable: true,
        }),
        Arc::new(support::FixtureEmbedder(false)),
    );
    Arc::get_mut(&mut worker).unwrap().options.retry_base = Duration::from_millis(100);
    worker.notify();
    let state = terminal(&repo, &id).await;
    assert_eq!(state.embedding_status, "ready");
    assert_eq!(state.attempt_count, 2);
    idle(&worker).await;
    worker.stop(Duration::from_secs(1)).await;
}

struct HeldClassifier {
    started: tokio::sync::mpsc::Sender<()>,
    release: Arc<tokio::sync::Semaphore>,
}
impl SlideClassifier for HeldClassifier {
    fn classify<'a>(
        &'a self,
        input: &'a SlideClassificationInput,
    ) -> ProviderFuture<'a, SlideRetrievalMetadata> {
        Box::pin(async move {
            self.started.send(()).await.unwrap();
            self.release.acquire().await.unwrap().forget();
            support::FixtureClassifier.classify(input).await
        })
    }
}

#[tokio::test]
async fn duplicate_notifications_cannot_exceed_four_active_classification_tasks() {
    let repo = Arc::new(
        support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let mut ids = Vec::new();
    for _ in 0..8 {
        ids.push(queue(repo.clone()).await);
    }
    let (started, mut active) = tokio::sync::mpsc::channel(8);
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let mut worker = worker(
        repo.clone(),
        Arc::new(HeldClassifier {
            started,
            release: release.clone(),
        }),
        Arc::new(support::FixtureEmbedder(false)),
    );
    let options = &mut Arc::get_mut(&mut worker).unwrap().options;
    options.concurrency = 4;
    options.classification_timeout = Duration::from_secs(5);
    for _ in 0..20 {
        worker.notify();
    }
    for _ in 0..4 {
        tokio::time::timeout(Duration::from_secs(3), active.recv())
            .await
            .unwrap()
            .unwrap();
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), active.recv())
            .await
            .is_err()
    );
    release.add_permits(8);
    for id in ids {
        let state = terminal(&repo, &id).await;
        assert_eq!(state.embedding_status, "ready");
        assert_eq!(state.attempt_count, 1);
    }
    idle(&worker).await;
    worker.stop(Duration::from_secs(1)).await;
}
