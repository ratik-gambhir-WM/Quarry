use super::*;

fn user(email: &str) -> UserNode {
    UserNode {
        id: 1,
        first_name: "Ada".to_string(),
        last_name: "Lovelace".to_string(),
        email: email.to_string(),
        api_key: "test-key".to_string(),
        role: "Analyst".to_string(),
        created_at: "2026-08-05T00:00:00Z".to_string(),
        updated_at: "2026-08-05T00:00:00Z".to_string(),
    }
}

#[test]
fn save_user_builds_a_query_for_a_valid_user() {
    assert!(save_user(user("ada@example.com")).is_ok());
}

#[test]
fn save_user_rejects_an_empty_email() {
    assert_eq!(
        save_user(user("  ")).unwrap_err(),
        "user email cannot be empty"
    );
}

#[test]
fn get_user_by_email_validates_the_lookup_key() {
    assert!(get_user_by_email("ada@example.com".to_string()).is_ok());
    assert_eq!(
        get_user_by_email(" ".to_string()).unwrap_err(),
        "user email cannot be empty"
    );
}
