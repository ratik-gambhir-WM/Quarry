use super::*;
use crate::domains::notebooks::{model::*, test_support};
fn note() -> Note {
    Note {
        summary: NoteSummary {
            note_id: "note".into(),
            notebook_id: "book".into(),
            folder_id: None,
            title: "Title".into(),
            content_schema_version: 1,
            revision: 2,
            created_at: now(),
            updated_at: now(),
        },
        blocks: test_support::blocks(),
    }
}
#[test]
fn notebook_projection_preflights_every_chunk_and_publishes_root_last() {
    let note = note();
    let chunks = vec!["first".into(), "second".into()];
    let prepare = || {
        prepare_projection(
            7,
            &note,
            "hash",
            &chunks,
            vec![vec![0.2, 0.8]; 2],
            NonZeroUsize::new(2).unwrap(),
            "test-model",
        )
        .unwrap()
    };
    let queries = prepare();
    assert_eq!(queries.len(), 3);
    let bytes = queries
        .iter()
        .map(|q| q.to_json_bytes().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        bytes,
        prepare()
            .iter()
            .map(|q| q.to_json_bytes().unwrap())
            .collect::<Vec<_>>()
    );
    for chunk in &bytes[..2] {
        let text = String::from_utf8(chunk.clone()).unwrap();
        assert!(text.contains(CHUNK));
        assert!(!text.contains(ROOT));
        assert!(text.contains("7:book:note:2:hash:"));
        assert!(text.contains("embedding_dimensions"));
    }
    let root = String::from_utf8(bytes[2].clone()).unwrap();
    assert!(root.contains(ROOT));
    assert!(root.contains("old_chunks"));
    assert!(root.contains("chunk_count"));
    assert!(prepare_projection(
        7,
        &note,
        "hash",
        &chunks,
        vec![vec![f32::NAN, 0.2]; 2],
        NonZeroUsize::new(2).unwrap(),
        "test-model"
    )
    .is_err());
    assert!(prepare_projection(
        7,
        &note,
        "hash",
        &["x".repeat(2 * 1024 * 1024)],
        vec![vec![0.2, 0.8]],
        NonZeroUsize::new(2).unwrap(),
        "test-model"
    )
    .is_err());
}
#[test]
fn notebook_empty_projection_publishes_no_vectors_and_removes_previous_revision() {
    let queries = prepare_projection(
        1,
        &note(),
        "empty",
        &[],
        vec![],
        NonZeroUsize::new(2).unwrap(),
        "test-model",
    )
    .unwrap();
    assert_eq!(queries.len(), 1);
    let text = String::from_utf8(queries[0].to_json_bytes().unwrap()).unwrap();
    assert!(text.contains("old_chunks"));
    assert!(!text.contains("F32Array"));
}

struct FixedLivePipeline {
    inner: NotebookPipeline,
    fail_publication: std::sync::atomic::AtomicBool,
}
impl SavePipeline for FixedLivePipeline {
    fn fingerprint(&self) -> String {
        self.inner.fingerprint()
    }
    fn published<'a>(
        &'a self,
        owner: i64,
        note: &'a Note,
        hash: &'a str,
    ) -> BoxFuture<'a, Result<bool, &'static str>> {
        self.inner.published(owner, note, hash)
    }
    fn embed<'a>(
        &'a self,
        chunks: &'a [String],
    ) -> BoxFuture<'a, Result<Vec<Vec<f32>>, &'static str>> {
        Box::pin(async move { Ok(vec![vec![0.25, 0.75]; chunks.len()]) })
    }
    fn publish<'a>(
        &'a self,
        owner: i64,
        note: &'a Note,
        hash: &'a str,
        chunks: &'a [String],
        vectors: Vec<Vec<f32>>,
    ) -> BoxFuture<'a, Result<(), &'static str>> {
        Box::pin(async move {
            if self
                .fail_publication
                .load(std::sync::atomic::Ordering::Acquire)
            {
                return Err("indexing_failed");
            }
            self.inner.publish(owner, note, hash, chunks, vectors).await
        })
    }
    fn delete(&self, owner: i64, note: String) -> BoxFuture<'_, Result<(), &'static str>> {
        self.inner.delete(owner, note)
    }
}
#[tokio::test]
#[ignore = "requires an explicitly selected disposable QUARRY_HELIX_COMPAT_URL"]
async fn postgres_notebook_commit_retry_and_cleanup_use_the_retained_helix_projection() {
    use crate::{
        adapters::postgres::query::{Condition as C, SqlBuilder as Q},
        domains::notebooks::{repository::NotebookRepository, service::NotebookService},
    };
    use std::sync::atomic::Ordering;
    let url = std::env::var("QUARRY_HELIX_COMPAT_URL").expect("explicit disposable Helix URL");
    let database = test_support::database().await;
    let pipeline = Arc::new(FixedLivePipeline {
        inner: NotebookPipeline::new(
            Arc::new(HelixClient::with_config(&url, None).unwrap()),
            None,
            NonZeroUsize::new(2).unwrap(),
            "fixed-verification".into(),
        ),
        fail_publication: std::sync::atomic::AtomicBool::new(true),
    });
    pipeline.inner.initialize().await.unwrap();
    let service =
        NotebookService::with_pipeline(NotebookRepository::new(database.clone()), pipeline.clone());
    let book = service
        .create(1, "Cross-store notebook".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let mut input = test_support::input("Canonical rich note");
    input.blocks = PatchField::Value(test_support::blocks());
    let job = service.save(1, book.clone(), None, input).await.unwrap();
    let failed = test_support::terminal(&service, &book, &job.job_id).await;
    assert_eq!(failed.status, "failed");
    assert!(failed.sqlite_committed && failed.retryable);
    let note = service
        .get_note(1, book.clone(), job.note_id.clone())
        .await
        .unwrap();
    assert_eq!(note.summary.revision, 1);
    assert_eq!(note.blocks, test_support::blocks());
    let hash = database
        .read_one_with_async(
            Q::select("notebook_save_jobs")
                .column("content_hash")
                .and_where(C::equal("job_id", &job.job_id))
                .build(),
            |r| r.get::<_, String>(0),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(!pipeline.published(1, &note, &hash).await.unwrap());
    database
        .write_async(
            Q::update("notebook_save_jobs")
                .set("updated_at", "2020-01-01T00:00:00.000Z")
                .and_where(C::equal("job_id", &job.job_id))
                .build(),
        )
        .await
        .unwrap();
    pipeline.fail_publication.store(false, Ordering::Release);
    service
        .retry(1, book.clone(), job.job_id.clone())
        .await
        .unwrap();
    let completed = test_support::terminal(&service, &book, &job.job_id).await;
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.committed_revision, Some(1));
    assert!(pipeline.published(1, &note, &hash).await.unwrap());
    assert!(service
        .get_note(2, book.clone(), job.note_id.clone())
        .await
        .is_err());
    service
        .delete_note(1, book, job.note_id.clone(), 1)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let confirmed = database
                .read_one_with_async(
                    Q::select("notebook_cleanup")
                        .column("confirmed")
                        .and_where(C::equal("note_id", &job.note_id))
                        .build(),
                    |r| r.get::<_, bool>(0),
                )
                .await
                .unwrap();
            if confirmed == Some(true) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(!pipeline.published(1, &note, &hash).await.unwrap());
}
