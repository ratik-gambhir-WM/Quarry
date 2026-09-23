use super::*;
use crate::{
    adapters::sqlite::client::SqliteClient,
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
        deal_id: "DEAL-000184".to_string(),
        deal_name: "Acme acquisition of WidgetCo".to_string(),
        status: "Active".to_string(),
        start_date: "2026-02-14".to_string(),
        close_date: "2026-05-01".to_string(),
        transaction_type: "Acquisition".to_string(),
        target_company: "WidgetCo".to_string(),
        primary_buyer: "CVS".to_string(),
        deal_sponsor: "Thoma Bravo".to_string(),
        user_email: "analyst@example.com".to_string(),
        local_path: Some("/tmp/data-room".to_string()),
        sharepoint_link: None,
    }
}

#[tokio::test]
async fn saves_core_deal_and_empty_metadata_in_the_first_call() {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrate(&sqlite).unwrap();
    let users_repository = UserRepository::new(sqlite.clone());
    let users = UserService::new(users_repository.clone());
    let deals = DealService::new(
        UserDirectory::new(users_repository),
        DealRepository::new(sqlite),
        None,
        "test-model".to_string(),
    );
    let user = users
        .create(AddUserInput {
            first_name: "Avery".to_string(),
            last_name: "Analyst".to_string(),
            email: "analyst@example.com".to_string(),
            api_key: "test-key".to_string(),
            role: "Analyst".to_string(),
        })
        .await
        .unwrap();

    let response = deals.create(input()).await.unwrap();

    assert_eq!(response.deal.deal_id, "DEAL-000184");
    assert_eq!(response.deal.user_id, user.id);
    assert_eq!(response.metadata.user_id, user.id);
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
async fn saves_submitted_links_before_optional_extraction_fails() {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrate(&sqlite).unwrap();
    let users_repository = UserRepository::new(sqlite.clone());
    let users = UserService::new(users_repository.clone());
    let deals_repository = DealRepository::new(sqlite);
    let deals = DealService::new(
        UserDirectory::new(users_repository),
        deals_repository.clone(),
        None,
        "test-model".to_string(),
    );
    users
        .create(AddUserInput {
            first_name: "Avery".to_string(),
            last_name: "Analyst".to_string(),
            email: "analyst@example.com".to_string(),
            api_key: "test-key".to_string(),
            role: "Analyst".to_string(),
        })
        .await
        .unwrap();
    let mut deal_input = input();
    deal_input.local_path = None;
    deals.create(deal_input).await.unwrap();

    let error = deals
        .save_metadata(
            "DEAL-000184",
            SaveDealMetadataInput {
                uploaded_files: vec![UploadedDealFile {
                    relative_path: "Acme SOW.txt".to_string(),
                    filename: "Acme SOW.txt".to_string(),
                    mime_type: "text/plain".to_string(),
                    bytes: b"scope".to_vec(),
                }],
                sharepoint_link: Some("https://northwind.sharepoint.com/sites/acme".to_string()),
                sow_link: Some("https://example.com/sow".to_string()),
                fact_sheet_link: Some("https://example.com/fact-sheet".to_string()),
                rl_link: Some("https://example.com/request-list".to_string()),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ServiceError::unavailable("OpenAI capability is not configured")
    );
    let metadata = deals_repository
        .metadata("DEAL-000184".to_string())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        metadata.sharepoint_link.as_deref(),
        Some("https://northwind.sharepoint.com/sites/acme")
    );
    assert_eq!(
        metadata.sow_link.as_deref(),
        Some("https://example.com/sow")
    );
    assert_eq!(
        metadata.fact_sheet_link.as_deref(),
        Some("https://example.com/fact-sheet")
    );
    assert_eq!(
        metadata.rl_link.as_deref(),
        Some("https://example.com/request-list")
    );
}

#[test]
fn parses_fenced_extraction_json() {
    let extraction = parse_deal_extraction("```json\n{\"keyQuestions\":[\"Why?\"]}\n```").unwrap();
    assert_eq!(extraction.key_questions, vec!["Why?"]);
}
