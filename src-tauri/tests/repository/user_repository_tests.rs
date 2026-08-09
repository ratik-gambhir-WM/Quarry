use super::*;

#[test]
fn query_user_by_email_matches_email_case_insensitively() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                first_name TEXT NOT NULL,
                last_name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                api_key TEXT NOT NULL,
                role TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            INSERT INTO users (first_name, last_name, email, api_key, role)
            VALUES ('Sam', 'Example', 'SAM@gmail.com', 'test-key', 'user');
            "#,
        )
        .unwrap();

    let user = query_user_by_email(&connection, "sam@gmail.com")
        .unwrap()
        .expect("case-insensitive email lookup should find the existing user");

    assert_eq!(user.email, "SAM@gmail.com");
}

#[test]
fn serialized_user_masks_the_api_key() {
    let user = User {
        id: 1,
        first_name: "Ada".to_string(),
        last_name: "Lovelace".to_string(),
        email: "ada@example.com".to_string(),
        api_key: "sk-live-super-secret-1234".to_string(),
        role: "Analyst".to_string(),
        created_at: "2026-08-09T00:00:00Z".to_string(),
        updated_at: "2026-08-09T00:00:00Z".to_string(),
    };

    let serialized = serde_json::to_string(&user).expect("user should serialize");
    assert!(!serialized.contains("super-secret"));
    assert!(serialized.contains(r#""apiKey":"sk-...1234""#));
    assert_eq!(user.api_key, "sk-live-super-secret-1234");
}
