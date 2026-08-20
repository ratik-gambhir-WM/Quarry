use super::*;
use crate::services::user_service::{save_sqlite_user, AddUserInput};

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

#[test]
fn validates_dates_and_optional_data_room_location() {
    assert!(validate_deal_input(&input()).is_ok());

    let mut no_location = input();
    no_location.local_path = None;
    assert!(validate_deal_input(&no_location).is_ok());

    let mut invalid_dates = input();
    invalid_dates.close_date = "2026-01-01".to_string();
    assert_eq!(
        validate_deal_input(&invalid_dates).unwrap_err(),
        "closeDate cannot be before startDate"
    );

    let mut both_locations = input();
    both_locations.sharepoint_link =
        Some("https://company.sharepoint.com/sites/deal-room".to_string());
    assert_eq!(
        validate_deal_input(&both_locations).unwrap_err(),
        "localPath and sharepointLink cannot both be provided"
    );

    let mut invalid_sharepoint_link = no_location;
    invalid_sharepoint_link.sharepoint_link = Some("http://example.com/deal-room".to_string());
    assert_eq!(
        validate_deal_input(&invalid_sharepoint_link).unwrap_err(),
        "sharepointLink must be an HTTPS SharePoint URL"
    );
}

#[test]
fn saves_core_deal_and_empty_metadata_in_the_first_call() {
    let state = AppState::in_memory().unwrap();
    let user = save_sqlite_user(
        &state,
        AddUserInput {
            first_name: "Avery".to_string(),
            last_name: "Analyst".to_string(),
            email: "analyst@example.com".to_string(),
            api_key: "test-key".to_string(),
            role: "Analyst".to_string(),
        },
    )
    .unwrap();

    let response = save_deal(&state, input()).unwrap();

    assert_eq!(response.deal.deal_id, "DEAL-000184");
    assert_eq!(response.metadata.user_id, user.id);
    assert_eq!(response.metadata.key_questions_json, "[]");
    assert_eq!(
        response.metadata.local_path.as_deref(),
        Some("/tmp/data-room")
    );
    assert!(response.metadata.sharepoint_link.is_none());
}

#[test]
fn parses_fenced_extraction_json() {
    let extraction = parse_deal_extraction("```json\n{\"keyQuestions\":[\"Why?\"]}\n```").unwrap();
    assert_eq!(extraction.key_questions, vec!["Why?"]);
}
