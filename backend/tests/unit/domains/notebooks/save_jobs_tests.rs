use super::*;
use crate::domains::notebooks::{model::*, repository::NotebookRepository, test_support};
use std::sync::atomic::Ordering;
#[tokio::test]
async fn notebook_save_is_admitted_while_provider_paused_and_completion_waits_for_publication() {
    let app = test_support::application();
    app.pipeline.blocked.store(true, Ordering::SeqCst);
    let book = app
        .service
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let mut input = test_support::input("Test");
    input.blocks = PatchField::Value(test_support::blocks());
    let job = tokio::time::timeout(
        Duration::from_millis(500),
        app.service.save(1, book.clone(), None, input),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(job.status, "queued");
    tokio::time::timeout(Duration::from_secs(3), async {
        while app.pipeline.embeds.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let pending = app
        .service
        .job(1, book.clone(), job.job_id.clone())
        .await
        .unwrap();
    assert_eq!(pending.status, "processing");
    assert!(pending.sqlite_committed);
    assert!(!app
        .pipeline
        .published
        .lock()
        .unwrap()
        .contains(&(job.note_id.clone(), 1)));
    app.pipeline.gate.notify_one();
    let completed = test_support::terminal(&app.service, &book, &job.job_id).await;
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.committed_revision, Some(1));
    assert!(app
        .pipeline
        .published
        .lock()
        .unwrap()
        .contains(&(job.note_id, 1)));
}
#[tokio::test]
async fn notebook_index_failure_retains_canonical_content_retry_does_not_increment_revision() {
    let app = test_support::application();
    app.pipeline.fail.store(true, Ordering::SeqCst);
    let book = app
        .service
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let job = app
        .service
        .save(1, book.clone(), None, test_support::input("Stored"))
        .await
        .unwrap();
    let failed = test_support::terminal(&app.service, &book, &job.job_id).await;
    assert_eq!(failed.status, "failed");
    assert!(failed.sqlite_committed);
    assert!(failed.retryable);
    assert_eq!(failed.error_code.as_deref(), Some("embedding_failed"));
    assert_eq!(
        app.service
            .get_note(1, book.clone(), job.note_id.clone())
            .await
            .unwrap()
            .summary
            .revision,
        1
    );
    app.sqlite
        .with_connection(|c| {
            c.execute(
                "UPDATE notebook_save_jobs SET updated_at='2020-01-01T00:00:00.000Z'",
                [],
            )
        })
        .unwrap();
    app.pipeline.fail.store(false, Ordering::SeqCst);
    app.service
        .retry(1, book.clone(), job.job_id.clone())
        .await
        .unwrap();
    let completed = test_support::terminal(&app.service, &book, &job.job_id).await;
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.attempt, 2);
    assert!(completed.sequence > failed.sequence);
    assert_eq!(
        app.service
            .get_note(1, book, job.note_id)
            .await
            .unwrap()
            .summary
            .revision,
        1
    );
}
#[tokio::test]
async fn notebook_restart_recovers_admission_and_post_commit_checkpoint_without_reapplying_mutation(
) {
    let sqlite = test_support::database();
    let repo = NotebookRepository::new(sqlite.clone());
    let book = repo
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let job = repo
        .admit(1, book.clone(), None, test_support::input("Recover"))
        .await
        .unwrap();
    let claim = repo.claim().await.unwrap().unwrap();
    repo.persist(&claim).await.unwrap();
    sqlite
        .with_connection(|c| c.execute("UPDATE notebook_save_jobs SET lease_until=0", []))
        .unwrap();
    let pipeline = Arc::new(test_support::FakePipeline::default());
    // Simulate a crash after Helix publication but before the durable completion checkpoint.
    pipeline
        .published
        .lock()
        .unwrap()
        .insert((job.note_id.clone(), 1));
    let recovered =
        NotebookService::with_pipeline(NotebookRepository::new(sqlite), pipeline.clone());
    let completed = test_support::terminal(&recovered, &book, &job.job_id).await;
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.committed_revision, Some(1));
    assert_eq!(pipeline.embeds.load(Ordering::SeqCst), 0);
    assert_eq!(
        recovered
            .get_note(1, book, job.note_id)
            .await
            .unwrap()
            .summary
            .revision,
        1
    );
}
#[tokio::test]
async fn notebook_folder_relocation_and_delete_fence_stale_worker_without_resurrection() {
    let app = test_support::application();
    let book = app
        .service
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let folder = app
        .service
        .create_folder(1, book.clone(), "Folder".into())
        .await
        .unwrap();
    app.pipeline.blocked.store(true, Ordering::SeqCst);
    let mut input = test_support::input("Race");
    input.folder_id = PatchField::Value(folder.folder_id.clone());
    let job = app
        .service
        .save(1, book.clone(), None, input)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while app.pipeline.embeds.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    app.service
        .delete_folder(1, book.clone(), folder.folder_id)
        .await
        .unwrap();
    app.pipeline.gate.notify_one();
    let failed = test_support::terminal(&app.service, &book, &job.job_id).await;
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.error_code.as_deref(), Some("revision_conflict"));
    assert!(!failed.retryable);
    assert_eq!(
        app.service
            .get_note(1, book.clone(), job.note_id.clone())
            .await
            .unwrap()
            .summary
            .revision,
        2
    );
    app.service
        .delete_note(1, book.clone(), job.note_id.clone())
        .await
        .unwrap();
    assert!(app
        .service
        .get_note(1, book.clone(), job.note_id)
        .await
        .is_err());
    app.service.delete_notebook(1, book).await.unwrap();
}
#[tokio::test]
async fn notebook_empty_note_completes_without_embedding_input() {
    let app = test_support::application();
    let book = app
        .service
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let job = app
        .service
        .save(1, book.clone(), None, test_support::input(""))
        .await
        .unwrap();
    assert_eq!(
        test_support::terminal(&app.service, &book, &job.job_id)
            .await
            .status,
        "completed"
    );
    assert_eq!(app.pipeline.embeds.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn notebook_recovery_starts_queued_work_and_deletion_fences_inflight_indexing() {
    let sqlite = test_support::database();
    let repo = NotebookRepository::new(sqlite.clone());
    let book = repo
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let queued = repo
        .admit(
            1,
            book.clone(),
            None,
            test_support::input("Queued before crash"),
        )
        .await
        .unwrap();
    let pipeline = Arc::new(test_support::FakePipeline::default());
    pipeline.blocked.store(true, Ordering::SeqCst);
    let recovered =
        NotebookService::with_pipeline(NotebookRepository::new(sqlite), pipeline.clone());
    tokio::time::timeout(Duration::from_secs(3), async {
        while pipeline.embeds.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    recovered
        .delete_note(1, book.clone(), queued.note_id.clone())
        .await
        .unwrap();
    pipeline.gate.notify_one();
    let terminal = test_support::terminal(&recovered, &book, &queued.job_id).await;
    assert_eq!(terminal.status, "failed");
    assert!(!terminal.retryable);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if pipeline.published.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(recovered.get_note(1, book, queued.note_id).await.is_err());
}
