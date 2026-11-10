use super::*;
use crate::{
    adapters::{
        helix::client::HelixClient, office::converter::OfficeConverter,
        sqlite::client::SqliteClient,
    },
    app::migrations,
    domains::documents::{
        graph::repository::DocumentGraphReader, ingestion::service::DocumentIngestionService,
        store::sqlite::DocumentStore,
    },
    integration_support::seed_identity,
};
use std::num::NonZeroUsize;
fn service(sqlite: SqliteClient, helix: Arc<HelixClient>) -> Arc<DealDeletionService> {
    let graph = DocumentGraphWriter::new(helix.clone(), NonZeroUsize::new(2).unwrap());
    let files = DocumentStore::new(sqlite.clone());
    let lifecycle = DealLifecycle::default();
    let ingestion = Arc::new(
        DocumentIngestionService::new(
            files.clone(),
            DocumentGraphReader::new(helix),
            graph.clone(),
            None,
            "test".into(),
            "test".into(),
            1,
        )
        .with_lifecycle(lifecycle.clone()),
    );
    let jobs = Arc::new(DocumentJobService::new(ingestion, Duration::from_secs(1)));
    DealDeletionService::new(
        DeletionRepository::new(sqlite),
        graph,
        lifecycle,
        jobs,
        Arc::new(StoredDocumentService::new(
            files,
            OfficeConverter::new(None),
        )),
    )
}
fn seed(sqlite: &SqliteClient) {
    migrations::migrate(sqlite).unwrap();
    seed_identity(sqlite, "deletion-test@example.com", 'a');
    sqlite.with_connection(|c|c.execute_batch("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES('DELETE-RESTART',1,'Synthetic','buy_side','unknown','pipeline','now','now'); INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES('DELETE-RESTART',1,'owner','now','now'); INSERT INTO deal_access_revisions(deal_id) VALUES('DELETE-RESTART');")).unwrap();
}
#[tokio::test]
async fn failed_remote_fence_keeps_canonical_data_and_a_recoverable_receipt() {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    seed(&sqlite);
    let worker = service(
        sqlite.clone(),
        Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap()),
    );
    let op = worker.accept("DELETE-RESTART".into(), 1).await.unwrap();
    worker.tick().await.unwrap();
    let status = worker.status(op.deletion_id, 1).await.unwrap();
    assert_eq!(status.status, super::super::model::DeletionStatus::Retrying);
    assert_eq!(status.error_code.as_deref(), Some("cleanup_unavailable"));
    assert_eq!(
        sqlite
            .with_connection(
                |c| c.query_row("SELECT count(*) FROM deals", [], |r| r.get::<_, i64>(0))
            )
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn lifecycle_waits_for_real_task_guards() {
    let gate = DealLifecycle::default();
    let task = gate.admit("synthetic").await;
    let drain = gate.drain("synthetic");
    tokio::pin!(drain);
    assert!(futures_util::poll!(&mut drain).is_pending());
    drop(task);
    let exclusive = drain.await;
    let late = gate.admit("synthetic");
    tokio::pin!(late);
    assert!(futures_util::poll!(&mut late).is_pending());
    drop(exclusive);
    drop(late.await);
}
#[tokio::test]
#[ignore = "requires an explicitly supplied disposable QUARRY_HELIX_COMPAT_URL"]
async fn deletion_worker_resumes_after_acceptance_and_after_canonical_purge() {
    let url = std::env::var("QUARRY_HELIX_COMPAT_URL").expect("disposable Helix URL");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.sqlite");
    let sqlite = SqliteClient::open(&path).unwrap();
    seed(&sqlite);
    let helix = Arc::new(HelixClient::with_config(&url, None).unwrap());
    let first = service(sqlite.clone(), helix.clone());
    first.graph.initialize().await.unwrap();
    let op = first.accept("DELETE-RESTART".into(), 1).await.unwrap();
    drop(first);
    drop(sqlite);
    let sqlite = SqliteClient::open(&path).unwrap();
    migrations::migrate(&sqlite).unwrap();
    let second = service(sqlite.clone(), helix.clone());
    second.graph.fence_deal(&op.deal_id).await.unwrap();
    second
        .repository
        .snapshot_and_purge(op.clone())
        .await
        .unwrap();
    drop(second);
    drop(sqlite);
    let sqlite = SqliteClient::open(&path).unwrap();
    migrations::migrate(&sqlite).unwrap();
    let third = service(sqlite, helix);
    third.tick().await.unwrap();
    assert_eq!(
        third.status(op.deletion_id, 1).await.unwrap().status,
        super::super::model::DeletionStatus::Completed
    );
    assert_eq!(
        third.accept(op.deal_id, 1).await.unwrap().status,
        super::super::model::DeletionStatus::Completed
    );
}

#[tokio::test]
async fn cloned_admission_survives_a_cancelled_parent_until_blocking_work_finishes() {
    let gate = DealLifecycle::default();
    let admission = gate.admit("blocking-preview").await;
    let blocking_admission = admission.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
    let parent = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || {
            let _admission = blocking_admission;
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            finished_tx.send(()).unwrap();
        })
        .await
        .unwrap();
    });
    started_rx.await.unwrap();
    parent.abort();
    drop(admission);
    let drain = gate.drain("blocking-preview");
    tokio::pin!(drain);
    assert!(futures_util::poll!(&mut drain).is_pending());
    release_tx.send(()).unwrap();
    finished_rx.await.unwrap();
    drop(drain.await);
}
