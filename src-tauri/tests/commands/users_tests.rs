use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use super::*;

#[test]
fn helix_user_responses_redact_nested_api_keys() {
    let sanitized = redact_user_secrets(serde_json::json!({
        "user": { "api_key": "sk-secret", "email": "analyst@example.com" },
        "items": [{ "apiKey": "another-secret" }]
    }));

    assert_eq!(sanitized["user"]["api_key"], "[REDACTED]");
    assert_eq!(sanitized["items"][0]["apiKey"], "[REDACTED]");
    assert_eq!(sanitized["user"]["email"], "analyst@example.com");
}
use tauri::{
    test::{mock_builder, mock_context, noop_assets},
    Manager,
};

#[tokio::test]
#[ignore = "integration test; requires a running Helix database"]
async fn create_wm_user_creates_a_user_in_helix() {
    let input = unique_user("create");

    let response = invoke_create_wm_user(input.clone()).await;
    let persisted_user = find_user_by_email(&response, &input.email).unwrap_or_else(|| {
        panic!(
            "create_wm_user response did not contain the created user `{}`: {response}",
            input.email
        )
    });

    assert_user_matches(persisted_user, &input);
}

#[tokio::test]
#[ignore = "integration test; requires a running Helix database"]
async fn create_wm_user_updates_an_existing_user_by_email_in_helix() {
    let original = unique_user("update");
    invoke_create_wm_user(original.clone()).await;

    let updated = UserNode {
        first_name: "Updated".to_string(),
        last_name: "User".to_string(),
        api_key: "updated-test-api-key".to_string(),
        role: "updated-test-role".to_string(),
        updated_at: "2026-08-05 13:30:00".to_string(),
        ..original
    };

    invoke_create_wm_user(updated.clone()).await;

    let response = invoke_get_wm_user_by_email(updated.email.clone()).await;
    let persisted_user = find_user_by_email(&response, &updated.email).unwrap_or_else(|| {
        panic!(
            "get_wm_user_by_email response did not contain the upserted user `{}`: {response}",
            updated.email
        )
    });

    assert_user_matches(persisted_user, &updated);
}

#[tokio::test]
#[ignore = "integration test; requires a running Helix database"]
async fn get_wm_user_by_email_returns_the_user_from_helix() {
    let input = unique_user("get-by-email");
    invoke_create_wm_user(input.clone()).await;

    let response = invoke_get_wm_user_by_email(input.email.clone()).await;
    let persisted_user = find_user_by_email(&response, &input.email).unwrap_or_else(|| {
        panic!(
            "Helix lookup response did not contain user `{}`: {response}",
            input.email
        )
    });

    assert_user_matches(persisted_user, &input);
}

async fn invoke_create_wm_user(input: UserNode) -> Value {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock Tauri app should build");
    let state = AppState::new_for_test().expect(
        "AppState should initialize; set HELIX_URL and HELIX_API_KEY when Helix is not local",
    );
    assert!(app.manage(state), "AppState should only be managed once");

    create_wm_user(app.state::<AppState>(), input)
        .await
        .expect("create_wm_user should persist the user in Helix")
}

async fn invoke_get_wm_user_by_email(email: String) -> Value {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock Tauri app should build");
    let state = AppState::new_for_test().expect(
        "AppState should initialize; set HELIX_URL and HELIX_API_KEY when Helix is not local",
    );
    assert!(app.manage(state), "AppState should only be managed once");

    get_wm_user_by_email(app.state::<AppState>(), email)
        .await
        .expect("get_wm_user_by_email should fetch the user from Helix")
}

fn unique_user(test_name: &str) -> UserNode {
    static NEXT_TEST_ID: AtomicI64 = AtomicI64::new(0);

    let unique_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the Unix epoch")
        .as_secs() as i64
        + NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);

    UserNode {
        id: unique_id,
        first_name: "Helix".to_string(),
        last_name: "Integration Test".to_string(),
        email: format!("quarry-{test_name}-{unique_id}@example.com"),
        api_key: "integration-test-api-key".to_string(),
        role: "integration-test-role".to_string(),
        created_at: "2026-08-05 13:00:00".to_string(),
        updated_at: "2026-08-05 13:00:00".to_string(),
    }
}

fn find_user_by_email<'a>(value: &'a Value, email: &str) -> Option<&'a Value> {
    match value {
        Value::Object(object) => {
            if object.get("email").and_then(Value::as_str) == Some(email) {
                return Some(value);
            }

            object
                .values()
                .find_map(|value| find_user_by_email(value, email))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|value| find_user_by_email(value, email)),
        _ => None,
    }
}

fn assert_user_matches(actual: &Value, expected: &UserNode) {
    assert_eq!(actual["id"].as_i64(), Some(expected.id));
    assert_eq!(
        actual["first_name"].as_str(),
        Some(expected.first_name.as_str())
    );
    assert_eq!(
        actual["last_name"].as_str(),
        Some(expected.last_name.as_str())
    );
    assert_eq!(actual["email"].as_str(), Some(expected.email.as_str()));
    assert_eq!(actual["api_key"].as_str(), Some(expected.api_key.as_str()));
    assert_eq!(actual["role"].as_str(), Some(expected.role.as_str()));
    assert_eq!(
        actual["created_at"].as_str(),
        Some(expected.created_at.as_str())
    );
    assert_eq!(
        actual["updated_at"].as_str(),
        Some(expected.updated_at.as_str())
    );
}
