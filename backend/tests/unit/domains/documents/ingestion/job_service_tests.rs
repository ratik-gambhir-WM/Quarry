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
    let jobs = Arc::new(DocumentJobService::new(
        ingestion.clone(),
        Duration::from_secs(60),
    ));
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
