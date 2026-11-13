use super::*;
use crate::{
    adapters::postgres::client::PostgresClient,
    app::migrations::migrate,
    domains::{
        deals::repository::DealRepository,
        users::{
            repository::UserRepository,
            service::{AddUserInput, UserService},
            UserDirectory,
        },
    },
};

fn input() -> SaveDealInput {
    SaveDealInput {
        fields: serde_json::from_value(serde_json::json!({
            "dealName": "Acme acquisition of WidgetCo", "dealCode": null,
            "engagementType": "buy_side", "transactionType": "unknown", "status": "pipeline",
            "startDate": null, "expectedCloseDate": null, "actualCloseDate": null, "engagementCompletedDate": null,
            "parties": [], "scopes": []
        })).unwrap(),
        user_email: "analyst@example.com".to_string(),
        local_path: Some("/tmp/data-room".to_string()),
        sharepoint_link: None,
    }
}

#[tokio::test]
async fn saves_core_deal_and_empty_metadata_in_the_first_call() {
    let postgres = PostgresClient::test_database().await.unwrap();
    migrate(&postgres).await.unwrap();
    let users_repository = UserRepository::new(postgres.clone());
    let users = UserService::new(users_repository.clone());
    let deals = DealService::new(
        UserDirectory::new(users_repository),
        DealRepository::new(postgres),
        None,
        "test-model".to_string(),
        crate::integration_support::prompt_catalog(),
    );
    let user = users
        .create(AddUserInput {
            first_name: "Avery".to_string(),
            last_name: "Analyst".to_string(),
            email: "analyst@example.com".to_string(),

            role: "Analyst".to_string(),
        })
        .await
        .unwrap();

    let response = deals.create(input()).await.unwrap();

    assert_eq!(
        uuid::Uuid::parse_str(&response.deal.deal_id)
            .unwrap()
            .get_version_num(),
        4
    );
    assert_eq!(response.metadata.deal_id, response.deal.deal_id);
    let second = deals.create(input()).await.unwrap();
    assert_ne!(second.deal.deal_id, response.deal.deal_id);
    assert_eq!(response.deal.user_id, user.id);
    assert_eq!(response.metadata.key_questions_json, "[]");
    assert_eq!(
        response.metadata.local_path.as_deref(),
        Some("/tmp/data-room")
    );
    assert!(response.metadata.sharepoint_link.is_none());
    assert!(response.metadata.sow_link.is_none());
    assert!(response.metadata.fact_sheet_link.is_none());
    assert!(response.metadata.rl_link.is_none());
}

#[tokio::test]
async fn failed_extraction_preserves_metadata_revision_and_allows_retry() {
    let postgres = PostgresClient::test_database().await.unwrap();
    migrate(&postgres).await.unwrap();
    let users_repository = UserRepository::new(postgres.clone());
    let users = UserService::new(users_repository.clone());
    let deals_repository = DealRepository::new(postgres);
    let deals = DealService::new(
        UserDirectory::new(users_repository),
        deals_repository.clone(),
        None,
        "test-model".to_string(),
        crate::integration_support::prompt_catalog(),
    );
    users
        .create(AddUserInput {
            first_name: "Avery".to_string(),
            last_name: "Analyst".to_string(),
            email: "analyst@example.com".to_string(),

            role: "Analyst".to_string(),
        })
        .await
        .unwrap();
    let mut deal_input = input();
    deal_input.local_path = None;
    let created = deals.create(deal_input).await.unwrap();
    let deal_id = created.deal.deal_id;

    let attempt = SaveDealMetadataInput {
        expected_revision: created.metadata.revision,
        uploaded_files: vec![UploadedDealFile {
            relative_path: "Acme SOW.txt".into(),
            filename: "Acme SOW.txt".into(),
            mime_type: "text/plain".into(),
            bytes: b"scope".to_vec(),
        }],
        sow_link: Some("https://example.com/sow".into()),
        ..Default::default()
    };
    for _ in 0..2 {
        let error = deals
            .save_metadata(&deal_id, attempt.clone(), 1)
            .await
            .unwrap_err();
        assert_eq!(
            error,
            ServiceError::unavailable("OpenAI capability is not configured")
        );
        let metadata = deals_repository
            .metadata(deal_id.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(metadata.revision, created.metadata.revision);
        assert!(metadata.sow_link.is_none());
        assert_eq!(metadata.key_questions_json, "[]");
    }
    // The same draft can also be saved without the failed optional extraction.
    let saved = deals
        .save_metadata(
            &deal_id,
            SaveDealMetadataInput {
                uploaded_files: vec![],
                ..attempt.clone()
            },
            1,
        )
        .await
        .unwrap();
    assert_eq!(saved.metadata.revision, created.metadata.revision + 1);
    assert_eq!(saved.metadata.sow_link, attempt.sow_link);
    // A genuine newer write still fences an old draft before provider lookup.
    assert!(matches!(
        deals.save_metadata(&deal_id, attempt, 1).await,
        Err(ServiceError::Conflict(_))
    ));
}

#[test]
fn parses_fenced_extraction_json() {
    let extraction = parse_deal_extraction("```json\n{\"keyQuestions\":[\"Why?\"]}\n```").unwrap();
    assert_eq!(extraction.key_questions, vec!["Why?"]);
}

#[tokio::test]
async fn extraction_retry_commits_once_and_rejects_edits_made_during_provider_work() {
    use axum::{routing::post, Json, Router};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let postgres = PostgresClient::test_database().await.unwrap();
    migrate(&postgres).await.unwrap();
    let users_repository = UserRepository::new(postgres.clone());
    UserService::new(users_repository.clone())
        .create(AddUserInput {
            first_name: "Test".into(),
            last_name: "User".into(),
            email: "analyst@example.com".into(),
            role: "Analyst".into(),
        })
        .await
        .unwrap();
    let repository = DealRepository::new(postgres);
    let mut service = DealService::new(
        UserDirectory::new(users_repository),
        repository.clone(),
        None,
        "test-model".into(),
        crate::integration_support::prompt_catalog(),
    );
    let created = service.create(input()).await.unwrap();
    let deal_id = created.deal.deal_id;
    let calls = Arc::new(AtomicUsize::new(0));
    let requests = calls.clone();
    let current = repository.clone();
    let target = deal_id.clone();
    let router = Router::new().route("/responses", post(move || {
        let current = current.clone();
        let target = target.clone();
        let attempt = requests.fetch_add(1, Ordering::Relaxed);
        async move {
            // Mutate canonical state while the third extraction is in flight.
            if attempt == 2 {
                let metadata = current.metadata(target.clone()).await.unwrap().unwrap();
                current.upsert_metadata(UpsertDealMetadataRecord {
                    expected_revision: metadata.revision, deal_id: target,
                    key_questions_json: metadata.key_questions_json,
                    local_path: metadata.local_path,
                    sharepoint_link: metadata.sharepoint_link,
                    sow_link: Some("https://example.com/newer".into()),
                    fact_sheet_link: metadata.fact_sheet_link, rl_link: metadata.rl_link,
                }, 1).await.unwrap();
            }
            let text = if attempt == 0 { "invalid extraction" } else { r#"{"keyQuestions":["Why?"]}"# };
            Json(serde_json::json!({"object":"response","output":[{
                "type":"message","role":"assistant","content":[{"type":"output_text","text":text}]
            }]}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
    });
    service.openai = Some(Arc::new(OpenAiClient::with_responses_url(
        "test-key",
        format!("http://{address}/responses"),
        crate::integration_support::prompt_catalog(),
    )));
    let mut attempt = SaveDealMetadataInput {
        expected_revision: 1,
        uploaded_files: vec![UploadedDealFile {
            relative_path: "scope.txt".into(),
            filename: "scope.txt".into(),
            mime_type: "text/plain".into(),
            bytes: b"scope".to_vec(),
        }],
        sow_link: Some("https://example.com/submitted".into()),
        ..Default::default()
    };
    assert!(matches!(
        service.save_metadata(&deal_id, attempt.clone(), 1).await,
        Err(ServiceError::Validation(_))
    ));
    assert_eq!(
        repository
            .metadata(deal_id.clone())
            .await
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    let saved = service
        .save_metadata(&deal_id, attempt.clone(), 1)
        .await
        .unwrap();
    assert_eq!(saved.metadata.revision, 2);
    assert_eq!(saved.metadata.sow_link, attempt.sow_link);
    assert_eq!(saved.extraction.key_questions, ["Why?"]);
    assert_eq!(saved.metadata.key_questions_json, r#"["Why?"]"#);
    attempt.expected_revision = 2;
    let conflict = service.save_metadata(&deal_id, attempt, 1).await;
    stop.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(conflict, Err(ServiceError::Conflict(_))));
    let metadata = repository.metadata(deal_id).await.unwrap().unwrap();
    assert_eq!(metadata.revision, 3);
    assert_eq!(
        metadata.sow_link.as_deref(),
        Some("https://example.com/newer")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}
