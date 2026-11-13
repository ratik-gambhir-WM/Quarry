use super::*;
use crate::{
    adapters::{
        helix::client::HelixClient, office::converter::OfficeConverter,
        postgres::client::PostgresClient,
    },
    app::migrations,
    domains::documents::{
        graph::repository::DocumentGraphReader, ingestion::service::DocumentIngestionService,
        store::postgres::DocumentStore,
    },
    integration_support::seed_identity,
};
use std::{num::NonZeroUsize, time::Duration};
fn service(postgres: PostgresClient, helix: Arc<HelixClient>) -> Arc<DealDeletionService> {
    let graph = DocumentGraphWriter::new(helix.clone(), NonZeroUsize::new(2).unwrap());
    let files = DocumentStore::new(postgres.clone());
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
        DeletionRepository::new(postgres),
        graph,
        lifecycle,
        jobs,
        Arc::new(StoredDocumentService::new(
            files,
            OfficeConverter::new(None),
        )),
    )
}
async fn seed(postgres: &PostgresClient) {
    migrations::migrate(postgres).await.unwrap();
    seed_identity(postgres, "deletion-test@example.com", 'a').await;
    seed_deal(postgres, "DELETE-RESTART").await;
}
async fn seed_deal(postgres: &PostgresClient, deal: &str) {
    postgres.with_connection(async |c| {
        c.execute("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES(?1,1,'Synthetic','buy_side','unknown','pipeline','now','now')", [deal]).await?;
        c.execute("INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES(?1,1,'owner','now','now')", [deal]).await?;
        c.execute("INSERT INTO deal_access_revisions(deal_id) VALUES(?1)", [deal]).await?;
        Ok(())
     }).await.unwrap();
}
#[tokio::test]
async fn failed_remote_fence_keeps_canonical_data_and_a_recoverable_receipt() {
    let postgres = PostgresClient::test_database().await.unwrap();
    seed(&postgres).await;
    let worker = service(
        postgres.clone(),
        Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap()),
    );
    let op = worker
        .repository
        .accept("DELETE-RESTART".into(), 1, 1)
        .await
        .unwrap();
    worker.tick().await.unwrap();
    let status = worker.status(op.deletion_id, 1).await.unwrap();
    assert_eq!(status.status, super::super::model::DeletionStatus::Retrying);
    assert_eq!(status.error_code.as_deref(), Some("cleanup_unavailable"));
    assert_eq!(
        postgres
            .with_connection(async |c| {
                c.query_row(
                    "SELECT count(*) FROM deals",
                    Vec::<crate::adapters::postgres::query::SqlValue>::new(),
                    |r| r.get::<_, i64>(0),
                )
                .await
            })
            .await
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn lifecycle_requires_drained_tasks_and_excludes_late_admission() {
    let gate = DealLifecycle::default();
    let task = gate.admit("synthetic").await;
    assert!(gate.try_drain("synthetic").await.is_none());
    drop(task);
    let exclusive = gate.try_drain("synthetic").await.unwrap();
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
    let postgres = PostgresClient::test_database_at(&path).await.unwrap();
    seed(&postgres).await;
    let helix = Arc::new(HelixClient::with_config(&url, None).unwrap());
    let first = service(postgres.clone(), helix.clone());
    first.graph.initialize().await.unwrap();
    let op = first
        .repository
        .accept("DELETE-RESTART".into(), 1, 1)
        .await
        .unwrap();
    drop(first);
    drop(postgres);
    let postgres = PostgresClient::test_database_at(&path).await.unwrap();
    migrations::migrate(&postgres).await.unwrap();
    let second = service(postgres.clone(), helix.clone());
    second.graph.fence_deal(&op.deal_id).await.unwrap();
    second
        .repository
        .snapshot_and_purge(op.clone())
        .await
        .unwrap();
    drop(second);
    drop(postgres);
    let postgres = PostgresClient::test_database_at(&path).await.unwrap();
    migrations::migrate(&postgres).await.unwrap();
    let third = service(postgres, helix);
    third.tick().await.unwrap();
    assert_eq!(
        third.status(op.deletion_id, 1).await.unwrap().status,
        super::super::model::DeletionStatus::Completed
    );
    assert_eq!(
        third
            .repository
            .accept(op.deal_id, 1, 1)
            .await
            .unwrap()
            .status,
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
            let admission = blocking_admission;
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            drop(admission);
            finished_tx.send(()).unwrap();
        })
        .await
        .unwrap();
    });
    started_rx.await.unwrap();
    parent.abort();
    drop(admission);
    assert!(gate.try_drain("blocking-preview").await.is_none());
    release_tx.send(()).unwrap();
    finished_rx.await.unwrap();
    parent.await.unwrap_err();
    assert!(gate.try_drain("blocking-preview").await.is_some());
}

#[tokio::test]
async fn busy_deals_do_not_block_cleanup_on_later_ledger_pages() {
    let postgres = PostgresClient::test_database().await.unwrap();
    seed(&postgres).await;
    let worker = service(
        postgres.clone(),
        Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap()),
    );
    let mut active = Vec::new();
    for index in 0..17 {
        let deal = format!("DELETE-BUSY-{index:02}");
        seed_deal(&postgres, &deal).await;
        active.push(worker.lifecycle.admit(&deal).await);
        let op = worker.repository.accept(deal, 1, 1).await.unwrap();
        postgres
            .with_connection(async |c| {
                c.execute(
                    "UPDATE deal_deletions SET deletion_id=?1 WHERE deletion_id=?2",
                    [format!("busy-{index:02}"), op.deletion_id],
                )
                .await
            })
            .await
            .unwrap();
    }
    let ready = worker
        .repository
        .accept("DELETE-RESTART".into(), 1, 1)
        .await
        .unwrap();
    postgres
        .with_connection(async |c| {
            c.execute(
                "UPDATE deal_deletions SET deletion_id='ready' WHERE deletion_id=?1",
                [&ready.deletion_id],
            )
            .await
        })
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), worker.tick())
        .await
        .expect("busy deals must not stall the ledger scan")
        .unwrap();
    // The unavailable remote is attempted only for the deal whose tasks have drained.
    assert_eq!(
        worker.status("ready".into(), 1).await.unwrap().status,
        super::super::model::DeletionStatus::Retrying
    );
    for index in 0..17 {
        let status = worker.status(format!("busy-{index:02}"), 1).await.unwrap();
        assert_eq!(status.status, super::super::model::DeletionStatus::Pending);
        assert_eq!(status.phase, DeletionPhase::Draining);
    }

    drop(active);
    worker.tick().await.unwrap();
    for index in 0..17 {
        assert_eq!(
            worker
                .status(format!("busy-{index:02}"), 1)
                .await
                .unwrap()
                .status,
            super::super::model::DeletionStatus::Retrying
        );
    }
}

#[tokio::test]
async fn cleanup_limits_active_deals_while_preserving_serialized_helix_writes() {
    let postgres = PostgresClient::test_database().await.unwrap();
    seed(&postgres).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (started, mut requests) = tokio::sync::mpsc::channel(8);
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let permits = release.clone();
    let router = axum::Router::new().route(
        "/v2/query",
        axum::routing::post(move || {
            let started = started.clone();
            let release = permits.clone();
            async move {
                started.send(()).await.unwrap();
                release.acquire().await.unwrap().forget();
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            }
        }),
    );
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
    });
    let worker = service(
        postgres.clone(),
        Arc::new(HelixClient::with_config(&format!("http://{address}"), None).unwrap()),
    );
    for index in 0..8 {
        let deal = format!("DELETE-CONCURRENT-{index}");
        seed_deal(&postgres, &deal).await;
        worker.repository.accept(deal, 1, 1).await.unwrap();
    }
    let task = tokio::spawn(async move { worker.tick().await });
    tokio::time::timeout(Duration::from_secs(3), requests.recv())
        .await
        .unwrap()
        .unwrap();
    // Helix intentionally serializes writes. The other admitted operations should reach
    // their durable fencing checkpoint while that first remote request is held open.
    let fencing_count = || {
        postgres.read_one_with_async(
            crate::adapters::postgres::query::SqlBuilder::select("deal_deletions")
                .count_rows()
                .and_where(crate::adapters::postgres::query::Condition::equal(
                    "phase", "fencing",
                ))
                .build(),
            |r| r.get::<_, i64>(0),
        )
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        while fencing_count().await.unwrap() != Some(4) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let exceeded_limit = tokio::time::timeout(Duration::from_millis(100), requests.recv())
        .await
        .is_ok();
    let admitted = fencing_count().await.unwrap();
    release.add_permits(8);
    let result = tokio::time::timeout(Duration::from_secs(3), task).await;
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    result.unwrap().unwrap().unwrap();
    assert!(!exceeded_limit, "Helix writes must remain serialized");
    assert_eq!(
        admitted,
        Some(4),
        "cleanup must not admit more than four deals"
    );
    let attempts: i64 = postgres
        .with_connection(async |c| {
            c.query_row(
                "SELECT count(*) FROM deal_deletions WHERE status='retrying'",
                Vec::<crate::adapters::postgres::query::SqlValue>::new(),
                |r| r.get(0),
            )
            .await
        })
        .await
        .unwrap();
    assert_eq!(attempts, 8);
}

async fn idle(worker: &Arc<DealDeletionService>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while Arc::strong_count(worker) != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the coordinator must release the service after draining its queue");
}

#[tokio::test]
async fn acceptance_spawns_cleanup_without_startup_and_blocked_work_retires() {
    let postgres = PostgresClient::test_database().await.unwrap();
    seed(&postgres).await;
    let worker = service(
        postgres,
        Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap()),
    );
    assert_eq!(Arc::strong_count(&worker), 1, "construction must not spawn");
    let admission = worker.lifecycle.admit("DELETE-RESTART").await;
    let receipt = worker.accept("DELETE-RESTART".into(), 1, 1).await.unwrap();
    assert_eq!(receipt.phase, DeletionPhase::Draining);
    // Cleanup must survive the accepting request and wait for active work to drain.
    drop(admission);
    tokio::time::timeout(Duration::from_secs(3), async {
        while worker
            .status(receipt.deletion_id.clone(), 1)
            .await
            .unwrap()
            .status
            != super::super::model::DeletionStatus::Retrying
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    worker
        .repository
        .failure(receipt.deletion_id, true)
        .await
        .unwrap();
    idle(&worker).await;
    // An empty recovery scan must also retire without keeping a polling task alive.
    worker.resume_pending();
    idle(&worker).await;
}

#[tokio::test]
async fn recovery_waits_for_a_future_retry_without_another_request() {
    let postgres = PostgresClient::test_database().await.unwrap();
    seed(&postgres).await;
    let worker = service(
        postgres.clone(),
        Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap()),
    );
    let receipt = worker
        .repository
        .accept("DELETE-RESTART".into(), 1, 1)
        .await
        .unwrap();
    worker
        .repository
        .failure(receipt.deletion_id.clone(), false)
        .await
        .unwrap();
    assert!(worker.repository.pending("").await.unwrap().is_empty());
    worker.resume_pending();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let attempts: i64 = postgres
                .with_connection(async |c| {
                    c.query_row(
                        "SELECT attempts FROM deal_deletions WHERE deletion_id=?1",
                        [&receipt.deletion_id],
                        |r| r.get(0),
                    )
                    .await
                })
                .await
                .unwrap();
            if attempts >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("a future durable retry must run without a request");
    worker
        .repository
        .failure(receipt.deletion_id, true)
        .await
        .unwrap();
    idle(&worker).await;
}

#[tokio::test]
async fn cancelled_accepting_request_still_notifies_after_the_receipt_commits() {
    use crate::adapters::postgres::client::test_support::OptionalExtension;
    let postgres = PostgresClient::test_database().await.unwrap();
    seed(&postgres).await;
    let worker = service(
        postgres.clone(),
        Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap()),
    );
    let (locked, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let blocked_sqlite = postgres.clone();
    let blocker = tokio::spawn(async move {
        let mut tx = blocked_sqlite.pool().begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(current_schema(),$1))")
            .bind(crate::adapters::postgres::client::WRITE_LOCK)
            .execute(&mut *tx)
            .await
            .unwrap();
        locked.send(()).unwrap();
        released.await.unwrap();
        tx.commit().await.unwrap();
    });
    ready.await.unwrap();
    {
        let request = worker.accept("DELETE-RESTART".into(), 1, 1);
        tokio::pin!(request);
        assert!(futures_util::poll!(&mut request).is_pending());
        // Drop the HTTP-owned future while acceptance is waiting for storage.
    }
    release.send(()).unwrap();
    blocker.await.unwrap();
    let id = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let id: Option<String> = postgres
                .with_connection(async |c| {
                    c.query_row(
                        "SELECT deletion_id FROM deal_deletions",
                        Vec::<crate::adapters::postgres::query::SqlValue>::new(),
                        |r| r.get(0),
                    )
                    .await
                    .optional()
                })
                .await
                .unwrap();
            if let Some(id) = id {
                break id;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while worker.status(id.clone(), 1).await.unwrap().status
            != super::super::model::DeletionStatus::Retrying
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("committed acceptance must notify despite request cancellation");
    worker.repository.failure(id, true).await.unwrap();
    idle(&worker).await;
}
