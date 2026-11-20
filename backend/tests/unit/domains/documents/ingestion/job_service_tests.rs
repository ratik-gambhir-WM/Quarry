use super::*;
use crate::{
    adapters::postgres::client::PostgresClient,
    app::migrations::migrate,
    domains::{
        deals::{
            model::DealFields,
            repository::{CreateDealRecord, DealRepository},
        },
        documents::store::repository::DocumentStore,
        memberships::{model::DealRole, repository::MembershipRepository},
    },
};

#[tokio::test]
async fn current_members_can_observe_shared_jobs_and_revocation_stops_event_delivery() {
    let db = PostgresClient::test_database().await.unwrap();
    migrate(&db, Default::default()).await.unwrap();
    db.with_connection(async |connection| {
        connection.execute(
            "INSERT INTO users(id,username,email,first_name,last_name,password_hash) VALUES(1,'owner','owner@example.test','Test','Owner','unusable'),(2,'viewer','viewer@example.test','Test','Viewer','unusable'),(3,'outsider','outsider@example.test','Test','Outsider','unusable')", Vec::<crate::adapters::postgres::query::SqlValue>::new(),
        ).await?;
        Ok(())
     }).await.unwrap();
    let fields: DealFields = serde_json::from_value(serde_json::json!({
        "dealName": "Jobs",
        "dealCode": null,
        "engagementType": "buy_side",
        "transactionType": "unknown",
        "status": "pipeline",
        "startDate": null,
        "expectedCloseDate": null,
        "actualCloseDate": null,
        "engagementCompletedDate": null,
        "parties": [],
        "scopes": []
    }))
    .unwrap();
    DealRepository::new(db.clone())
        .create(CreateDealRecord {
            deal_id: "DEAL-JOBS".into(),
            user_id: 1,
            fields,
            local_path: None,
            sharepoint_link: None,
        })
        .await
        .unwrap();
    let memberships = MembershipRepository::new(db.clone());
    memberships
        .add_member(
            "DEAL-JOBS".into(),
            1,
            "viewer@example.test".into(),
            DealRole::Viewer,
        )
        .await
        .unwrap();
    let ingestion = Arc::new(DocumentIngestionService::new(
        DocumentStore::new(db.clone()),
        super::super::repository::DocumentJobRepository {
            db: db.clone(),
            profile: crate::domains::documents::store::search_repository::EmbeddingProfile::new(
                "text-embedding-3-small".into(),
                1536,
            ),
        },
        None,
        "test".into(),
        "test".into(),
        1,
    ));
    let jobs = Arc::new(DocumentJobService::new(ingestion.clone()));
    let (id, _) = jobs
        .queue
        .admit(
            "DEAL-JOBS".into(),
            1,
            UploadedDocument {
                filename: "shared.pdf".into(),
                bytes: b"accepted input".to_vec(),
            },
            false,
        )
        .await
        .unwrap();
    assert!(jobs.subscribe_owned(&id, 2).await.is_ok());
    assert!(jobs.subscribe_owned(&id, 3).await.is_err());
    assert!(jobs.subscribe_owned("foreign-job", 2).await.is_err());
    let actor = crate::domains::identity::model::AuthenticatedUser {
        user: crate::domains::users::repository::UserRepository::new(db)
            .by_id(2)
            .await
            .unwrap()
            .unwrap(),
        token_hash: "synthetic".into(),
        expires_at: 4102444800,
    };
    let response = super::super::handler::process_document_job_events_handler(
        axum::Extension(actor),
        axum::extract::State(super::super::handler::DocumentIngestionHttpState {
            document_ingestion: ingestion,
            document_jobs: jobs.clone(),
        }),
        axum::extract::Path(id.clone()),
    )
    .await
    .unwrap();
    use axum::response::IntoResponse;
    use futures_util::StreamExt;
    let mut stream = response.into_response().into_body().into_data_stream();
    assert!(
        String::from_utf8(stream.next().await.unwrap().unwrap().to_vec())
            .unwrap()
            .contains("event: processing")
    );
    memberships
        .remove_member("DEAL-JOBS".into(), 1, 2, 1)
        .await
        .unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .is_none());
    assert!(jobs.authorize_job(&id, 2).await.is_err());
}

mod supervisor {
    use super::*;
    use std::sync::Mutex;
    use tokio::sync::{mpsc, Semaphore};

    async fn queue_with_jobs(count: usize) -> (PostgresClient, DocumentJobRepository) {
        let db = crate::domains::documents::test_support::database().await;
        let queue = DocumentJobRepository {
            db: db.clone(),
            profile: crate::domains::documents::store::search_repository::EmbeddingProfile::new(
                "text-embedding-3-small".into(),
                1536,
            ),
        };
        for index in 0..count {
            // Actor 1 may hold at most eight active jobs; spread across two actors' deals.
            let (deal, actor) = if index < 8 {
                ("DEAL-SQL", 1)
            } else {
                ("DEAL-FOREIGN", 2)
            };
            queue
                .admit(
                    deal.into(),
                    actor,
                    UploadedDocument {
                        filename: format!("document-{index}.pdf"),
                        bytes: format!("independent document {index}").into_bytes(),
                    },
                    true,
                )
                .await
                .unwrap();
        }
        (db, queue)
    }

    async fn processing(db: &PostgresClient) -> (i64, i64) {
        sqlx::query_as(
            "SELECT count(*) FILTER (WHERE status='processing'), count(*) FILTER (WHERE status='queued') FROM document_jobs",
        )
        .fetch_one(db.pool())
        .await
        .unwrap()
    }

    /// Each document reports that it started, then waits for a gate permit.
    fn gated(
        started: mpsc::UnboundedSender<String>,
        gate: Arc<Semaphore>,
        panic_on: Option<String>,
    ) -> super::super::ProcessDocument {
        Arc::new(move |claim: super::super::super::repository::Claimed| {
            let started = started.clone();
            let gate = gate.clone();
            let panic_on = panic_on.clone();
            Box::pin(async move {
                let filename = claim.attempt.filename.clone();
                started.send(filename.clone()).unwrap();
                if panic_on.as_deref() == Some(filename.as_str()) {
                    panic!("injected document task panic");
                }
                gate.acquire().await.unwrap().forget();
                Ok(())
            })
        })
    }

    async fn next(started: &mut mpsc::UnboundedReceiver<String>) -> String {
        tokio::time::timeout(Duration::from_secs(5), started.recv())
            .await
            .expect("document did not start")
            .unwrap()
    }

    #[tokio::test]
    async fn five_documents_prepare_concurrently_and_a_sixth_waits_for_a_free_slot() {
        let (db, queue) = queue_with_jobs(7).await;
        let (sender, mut started) = mpsc::unbounded_channel();
        let gate = Arc::new(Semaphore::new(0));
        let worker = super::super::start_supervisor(
            queue,
            5,
            Duration::from_secs(600),
            gated(sender, gate.clone(), None),
        );
        let mut seen = Vec::new();
        for _ in 0..5 {
            seen.push(next(&mut started).await);
        }
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(
            started.try_recv().is_err(),
            "a sixth document was preclaimed"
        );
        assert_eq!(processing(&db).await, (5, 2));
        // Freeing one slot immediately starts exactly one more document.
        gate.add_permits(1);
        seen.push(next(&mut started).await);
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(started.try_recv().is_err());
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 6);
        // Stop aborts blocked owned tasks within the grace period; no new claims follow.
        tokio::time::timeout(Duration::from_secs(10), worker.stop())
            .await
            .unwrap();
        let (_, queued) = processing(&db).await;
        assert_eq!(queued, 1);
        assert!(started.try_recv().is_err());
    }

    #[tokio::test]
    async fn one_panicking_document_neither_stops_the_supervisor_nor_cancels_siblings() {
        let (db, queue) = queue_with_jobs(3).await;
        let (sender, mut started) = mpsc::unbounded_channel();
        let gate = Arc::new(Semaphore::new(0));
        let worker = super::super::start_supervisor(
            queue,
            1,
            Duration::from_secs(600),
            gated(sender, gate.clone(), Some("document-0.pdf".into())),
        );
        // Claim order among same-second jobs is arbitrary; with one slot every document
        // still starts, including those queued behind the panicking one.
        let mut seen = std::collections::BTreeSet::new();
        while seen.len() < 3 {
            let filename = next(&mut started).await;
            if filename != "document-0.pdf" {
                gate.add_permits(1);
            }
            assert!(seen.insert(filename), "a document started twice");
        }
        worker.stop().await;
        let statuses: Vec<(String, String)> =
            sqlx::query_as("SELECT filename,status FROM document_jobs ORDER BY filename")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(statuses[0], ("document-0.pdf".into(), "processing".into()));
    }

    #[tokio::test]
    async fn a_stopped_supervisor_claims_nothing() {
        let (db, queue) = queue_with_jobs(2).await;
        let calls = Arc::new(Mutex::new(0));
        let counted = calls.clone();
        let process: super::super::ProcessDocument = Arc::new(move |_| {
            *counted.lock().unwrap() += 1;
            Box::pin(async { Ok(()) })
        });
        let worker = super::super::start_supervisor(queue, 5, Duration::from_secs(600), process);
        worker.request_stop();
        worker.stop().await;
        let claimed = *calls.lock().unwrap();
        let (processing, queued) = processing(&db).await;
        // Any claim completed before the stop was processed as owned work, never dropped.
        assert_eq!(processing, i64::from(claimed));
        assert_eq!(queued + processing, 2);
    }
}
