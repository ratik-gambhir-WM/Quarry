use super::*;

#[test]
fn new_schema_rejects_case_variant_email_duplicates() {
    let connection = Connection::open_in_memory().unwrap();
    run_migrations(&connection).unwrap();

    connection
        .execute(
            "INSERT INTO users (first_name, last_name, email, api_key, role) VALUES (?1, ?2, ?3, ?4, ?5)",
            ["Sam", "Example", "SAM@gmail.com", "test-key", "user"],
        )
        .unwrap();

    let duplicate = connection.execute(
        "INSERT INTO users (first_name, last_name, email, api_key, role) VALUES (?1, ?2, ?3, ?4, ?5)",
        ["Sam", "Example", "sam@gmail.com", "other-key", "user"],
    );

    assert!(duplicate.is_err());
}

#[test]
fn migrations_add_case_insensitive_email_index_to_legacy_schema() {
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
            "#,
        )
        .unwrap();

    run_migrations(&connection).unwrap();

    let query_plan = connection
        .prepare("EXPLAIN QUERY PLAN SELECT id FROM users WHERE email = ?1 COLLATE NOCASE")
        .unwrap()
        .query_map(["sam@gmail.com"], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let user_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .unwrap();

    assert!(
        query_plan
            .iter()
            .any(|detail| detail.contains("idx_users_email_nocase")),
        "query plan did not use the NOCASE email index: {query_plan:?}"
    );
    assert_eq!(user_version, 5);
}

#[test]
fn migration_preserves_legacy_investment_thesis_as_optional_metadata() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            r#"
            CREATE TABLE deal_metadata (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                deal_id INTEGER NOT NULL,
                key_questions_json TEXT NOT NULL DEFAULT '[]',
                investment_thesis TEXT NOT NULL DEFAULT '',
                document_count INTEGER NOT NULL DEFAULT 0,
                data_room_size_bytes INTEGER NOT NULL DEFAULT 0,
                portco_summary TEXT,
                buyer_summary TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO deal_metadata (deal_id, investment_thesis)
            VALUES (7, 'Preserve this desktop briefing');
            PRAGMA user_version = 4;
            "#,
        )
        .unwrap();

    run_migrations(&connection).unwrap();

    let preserved = connection
        .query_row(
            "SELECT legacy_investment_thesis FROM deal_metadata WHERE deal_id = 7",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap();
    assert_eq!(preserved.as_deref(), Some("Preserve this desktop briefing"));
    assert!(column_exists(&connection, "deal_metadata", "investment_thesis").unwrap());
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        5
    );
}
