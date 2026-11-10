use super::*;
use crate::{
    adapters::{helix::client::HelixClient, sqlite::client::SqliteClient},
    app::migrations::migrate,
    domains::{
        deals::{
            model::DealFields,
            repository::{CreateDealRecord, DealRepository},
        },
        documents::{
            graph::repository::{DocumentGraphReader, DocumentGraphWriter},
            store::sqlite::DocumentStore,
        },
        memberships::{model::DealRole, repository::MembershipRepository},
    },
};

#[tokio::test]
async fn current_members_can_observe_shared_jobs_and_revocation_stops_event_delivery() {
    let db = SqliteClient::open_in_memory().unwrap();
    migrate(&db).unwrap();
    db.with_connection(|connection| {
        connection.execute(
            "INSERT INTO users(id,username,email,first_name,last_name,password_hash) VALUES(1,'owner','owner@example.test','Test','Owner','unusable'),(2,'viewer','viewer@example.test','Test','Viewer','unusable'),(3,'outsider','outsider@example.test','Test','Outsider','unusable')",
            [],
        )?;
        Ok(())
    }).unwrap();
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
    let helix = Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap());
    let ingestion = Arc::new(DocumentIngestionService::new(
        DocumentStore::new(db.clone()),
        DocumentGraphReader::new(helix.clone()),
        DocumentGraphWriter::new(helix, std::num::NonZeroUsize::new(2).unwrap()),
        None,
        "test".into(),
        "test".into(),
        1,
    ));
    let jobs = Arc::new(DocumentJobService::new(
        ingestion.clone(),
        Duration::from_secs(60),
    ));
    let (sender, _) = watch::channel(DocumentJobEvent::processing(
        "job-test".into(),
        "shared.pdf".into(),
    ));
    jobs.jobs
        .write()
        .await
        .insert("job-test".into(), ("DEAL-JOBS".into(), 1, sender.clone()));
    assert!(jobs.subscribe_owned("job-test", 2).await.is_ok());
    assert!(jobs.subscribe_owned("job-test", 3).await.is_err());
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
        axum::extract::Path("job-test".into()),
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
        .remove_member("DEAL-JOBS".into(), 1, 2)
        .await
        .unwrap();
    sender.send_replace(DocumentJobEvent::completed(
        "job-test".into(),
        "shared.pdf".into(),
        Some("content".into()),
        1,
    ));
    assert!(tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .is_none());
    assert!(jobs.authorize_job("job-test", 2).await.is_err());
}

impl DocumentJobService {
    pub async fn subscribe(
        &self,
        job_id: &str,
    ) -> ServiceResult<watch::Receiver<DocumentJobEvent>> {
        self.jobs
            .read()
            .await
            .get(job_id)
            .map(|(_, _, sender)| sender.subscribe())
            .ok_or_else(|| {
                ServiceError::not_found(format!("document job `{job_id}` was not found"))
            })
    }
}
