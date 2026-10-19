use super::*;

fn seed_deal(connection: &Connection, deal_id: &str, status: &str) {
    connection
        .execute(
            "INSERT INTO users (first_name, last_name, email, api_key, role) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["Avery", "Analyst", format!("{deal_id}@example.com"), "key", "Analyst"],
        )
        .unwrap();
    let user_id = connection.last_insert_rowid();
    connection
        .execute(
            r#"
            INSERT INTO deals (
                deal_id, user_id, deal_name, status, created_at, updated_at, engagement_type, transaction_type
            ) VALUES (?1, ?2, 'Project Test', ?3, '2026-01-01', '2026-02-01',
                      'buy_side', 'unknown')
            "#,
            rusqlite::params![deal_id, user_id, status],
        )
        .unwrap();
}

fn insert_file_aggregate(connection: &Connection, deal_id: &str, file_id: &str, version_id: &str) {
    connection
        .execute(
            r#"
            INSERT INTO quarry_files (
                file_id, deal_id, workspace_id, display_name, metadata_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, 'report.pdf', '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')
            "#,
            rusqlite::params![file_id, deal_id, format!("{deal_id}@example.com")],
        )
        .unwrap();
    connection
        .execute(
            r#"
            INSERT INTO quarry_file_versions (
                version_id, file_id, version_number, original_filename, mime_type,
                content_sha256, byte_size, is_current, created_at
            ) VALUES (?1, ?2, 1, 'report.pdf', 'application/pdf', ?3, 3, 1, '2026-01-01T00:00:00Z')
            "#,
            rusqlite::params![version_id, file_id, "a".repeat(64)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO quarry_file_blobs (version_id, file_bytes) VALUES (?1, ?2)",
            rusqlite::params![version_id, vec![0_u8, 1, 255]],
        )
        .unwrap();
}

fn table_columns(connection: &Connection, table: &str) -> Vec<String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap();
    statement
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn table_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn migrations_create_the_versioned_file_schema_and_are_idempotent() {
    let mut connection = Connection::open_in_memory().unwrap();

    run_migrations(&mut connection).unwrap();
    seed_deal(&connection, "DEAL-000001", "active");
    insert_file_aggregate(&connection, "DEAL-000001", "file-1", "version-1");
    run_migrations(&mut connection).unwrap();

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, LATEST_SCHEMA_VERSION);
    assert_eq!(
        table_columns(&connection, "deal_metadata"),
        [
            "deal_id",
            "key_questions_json",
            "local_path",
            "sharepoint_link",
            "sow_link",
            "fact_sheet_link",
            "rl_link",
        ]
    );
    assert_eq!(
        table_columns(&connection, "quarry_files"),
        [
            "file_id",
            "deal_id",
            "workspace_id",
            "display_name",
            "source_uri",
            "metadata_json",
            "created_at",
            "updated_at",
            "deleted_at",
        ]
    );
    assert_eq!(
        table_columns(&connection, "quarry_file_versions"),
        [
            "version_id",
            "file_id",
            "version_number",
            "original_filename",
            "mime_type",
            "content_sha256",
            "byte_size",
            "is_current",
            "created_at",
        ]
    );
    assert_eq!(
        table_columns(&connection, "quarry_file_blobs"),
        ["version_id", "file_bytes"]
    );
    assert_eq!(table_count(&connection, "quarry_files"), 1);
    assert_eq!(table_count(&connection, "quarry_file_versions"), 1);
    assert_eq!(table_count(&connection, "quarry_file_blobs"), 1);

    let index_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN (\
             'uq_quarry_file_versions_current', 'idx_quarry_files_deal', \
             'idx_quarry_files_workspace_deal', 'idx_quarry_file_versions_file', \
             'idx_quarry_file_versions_hash')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(index_count, 5);
}

#[test]
fn file_schema_enforces_foreign_keys_json_and_one_current_version() {
    let mut connection = Connection::open_in_memory().unwrap();
    run_migrations(&mut connection).unwrap();
    seed_deal(&connection, "DEAL-CONSTRAINTS", "active");

    assert!(connection
        .execute(
            "INSERT INTO quarry_files (file_id, deal_id, workspace_id, display_name, metadata_json, created_at, updated_at) VALUES ('orphan', 'missing', 'owner', 'a.pdf', '{}', 'now', 'now')",
            [],
        )
        .is_err());
    assert!(connection
        .execute(
            "INSERT INTO quarry_files (file_id, deal_id, workspace_id, display_name, metadata_json, created_at, updated_at) VALUES ('invalid-json', 'DEAL-CONSTRAINTS', 'owner', 'a.pdf', 'not-json', 'now', 'now')",
            [],
        )
        .is_err());
    assert!(connection
        .execute(
            "INSERT INTO quarry_file_versions (version_id, file_id, version_number, original_filename, mime_type, content_sha256, byte_size, is_current, created_at) VALUES ('orphan-version', 'missing', 1, 'a.pdf', 'application/pdf', ?1, 1, 1, 'now')",
            ["a".repeat(64)],
        )
        .is_err());
    assert!(connection
        .execute(
            "INSERT INTO quarry_file_blobs (version_id, file_bytes) VALUES ('missing', X'01')",
            [],
        )
        .is_err());

    insert_file_aggregate(
        &connection,
        "DEAL-CONSTRAINTS",
        "file-constraints",
        "version-current",
    );
    assert!(connection
        .execute(
            r#"
            INSERT INTO quarry_file_versions (
                version_id, file_id, version_number, original_filename, mime_type,
                content_sha256, byte_size, is_current, created_at
            ) VALUES ('second-current', 'file-constraints', 2, 'report.pdf',
                      'application/pdf', ?1, 4, 1, 'now')
            "#,
            ["b".repeat(64)],
        )
        .is_err());
}

#[test]
fn deal_archive_retains_files_and_physical_delete_cascades() {
    let mut connection = Connection::open_in_memory().unwrap();
    run_migrations(&mut connection).unwrap();
    seed_deal(&connection, "DEAL-CASCADE", "active");
    insert_file_aggregate(&connection, "DEAL-CASCADE", "file-1", "version-1");

    connection
        .execute(
            "UPDATE deals SET archived_at = '2026-01-01T00:00:00Z' WHERE deal_id = 'DEAL-CASCADE'",
            [],
        )
        .unwrap();
    assert_eq!(table_count(&connection, "quarry_files"), 1);
    assert_eq!(table_count(&connection, "quarry_file_versions"), 1);
    assert_eq!(table_count(&connection, "quarry_file_blobs"), 1);

    connection
        .execute("DELETE FROM deals WHERE deal_id = 'DEAL-CASCADE'", [])
        .unwrap();
    assert_eq!(table_count(&connection, "quarry_files"), 0);
    assert_eq!(table_count(&connection, "quarry_file_versions"), 0);
    assert_eq!(table_count(&connection, "quarry_file_blobs"), 0);
}

#[test]
fn file_and_version_deletes_cascade_to_their_children() {
    let mut connection = Connection::open_in_memory().unwrap();
    run_migrations(&mut connection).unwrap();
    seed_deal(&connection, "DEAL-FILE-CASCADE", "active");
    insert_file_aggregate(
        &connection,
        "DEAL-FILE-CASCADE",
        "file-version-delete",
        "version-delete",
    );

    connection
        .execute(
            "DELETE FROM quarry_file_versions WHERE version_id = 'version-delete'",
            [],
        )
        .unwrap();
    assert_eq!(table_count(&connection, "quarry_files"), 1);
    assert_eq!(table_count(&connection, "quarry_file_blobs"), 0);

    insert_file_aggregate(
        &connection,
        "DEAL-FILE-CASCADE",
        "file-delete",
        "file-delete-version",
    );
    connection
        .execute("DELETE FROM quarry_files WHERE file_id = 'file-delete'", [])
        .unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM quarry_file_versions WHERE file_id = 'file-delete'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM quarry_file_blobs WHERE version_id = 'file-delete-version'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}

#[test]
fn incompatible_layouts_are_rejected_without_erasing_records() {
    for marker in [0, 6, 7, 8, 10] {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE users (name TEXT); INSERT INTO users VALUES ('retained');")
            .unwrap();
        connection
            .pragma_update(None, "user_version", marker)
            .unwrap();
        assert!(run_migrations(&mut connection).is_err());
        assert_eq!(table_count(&connection, "users"), 1);
    }
}
#[test]
fn explicit_reset_discards_records_and_reopen_preserves_new_records() {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrate(&sqlite).unwrap();
    sqlite
        .with_connection(|c| {
            seed_deal(c, "DEAL-OLD", "pipeline");
            Ok(())
        })
        .unwrap();
    reset_development(&sqlite).unwrap();
    sqlite
        .with_connection(|c| {
            assert_eq!(table_count(c, "users"), 0);
            assert_eq!(table_count(c, "deals"), 0);
            assert_eq!(
                c.pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))?,
                1
            );
            seed_deal(c, "DEAL-NEW", "pipeline");
            Ok(())
        })
        .unwrap();
    migrate(&sqlite).unwrap();
    sqlite
        .with_connection(|c| {
            assert_eq!(table_count(c, "deals"), 1);
            Ok(())
        })
        .unwrap();
}
#[test]
fn failed_reset_rolls_back_and_restores_foreign_keys() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE users (name TEXT); INSERT INTO users VALUES ('retained'); CREATE VIEW deals AS SELECT name FROM users;").unwrap();
    assert!(reset_connection(&mut connection).is_err());
    assert_eq!(table_count(&connection, "users"), 1);
    assert_eq!(
        connection
            .pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn final_model_constraints_and_chat_delete_restrictions_hold() {
    let mut c = Connection::open_in_memory().unwrap();
    run_migrations(&mut c).unwrap();
    seed_deal(&c, "DEAL-1", "pipeline");
    let owner = c.last_insert_rowid();
    c.execute("INSERT INTO companies (company_id, user_id, name, created_at, updated_at) VALUES ('c1', ?1, 'Same Name', 'now', 'now'), ('c2', ?1, 'Same Name', 'now', 'now')", [owner]).unwrap();
    c.execute("INSERT INTO deal_parties VALUES ('p1', 'DEAL-1', 'c1', 'buyer', NULL), ('p2', 'DEAL-1', 'c1', 'portfolio_company', NULL), ('p3', 'DEAL-1', 'c2', 'target', NULL)", []).unwrap();
    assert!(c
        .execute(
            "INSERT INTO deal_parties VALUES ('duplicate', 'DEAL-1', 'c1', 'buyer', NULL)",
            []
        )
        .is_err());
    assert!(c
        .execute("DELETE FROM companies WHERE company_id='c1'", [])
        .is_err());
    assert!(c.execute("UPDATE deals SET status='Archived'", []).is_err());
    assert!(c
        .execute("UPDATE deals SET expected_close_date=''", [])
        .is_err());
    assert!(c
        .execute(
            "INSERT INTO deal_scopes VALUES ('DEAL-1', 'unsupported')",
            []
        )
        .is_err());
    assert!(c
        .execute(
            "INSERT INTO deal_metadata (deal_id, key_questions_json) VALUES ('DEAL-1', '{}')",
            []
        )
        .is_err());
    c.execute("INSERT INTO assistant_threads (thread_id, user_id, deal_id, title) VALUES ('linked', ?1, 'DEAL-1', 'Linked'), ('freeform', ?1, NULL, 'Freeform')", [owner]).unwrap();
    assert!(c
        .execute("DELETE FROM deals WHERE deal_id='DEAL-1'", [])
        .is_err());
    c.execute(
        "UPDATE deals SET archived_at='now' WHERE deal_id='DEAL-1'",
        [],
    )
    .unwrap();
    assert_eq!(table_count(&c, "assistant_threads"), 2);
}

#[test]
fn disk_database_preserves_file_ownership_across_reopen() {
    let database_path = std::env::temp_dir().join(format!(
        "quarry-persistence-test-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));

    {
        let database = SqliteClient::open(&database_path).unwrap();
        database.with_connection_result(run_migrations).unwrap();
        database
            .with_connection(|connection| {
                seed_deal(connection, "DEAL-PERSISTED", "active");
                insert_file_aggregate(
                    connection,
                    "DEAL-PERSISTED",
                    "file-persisted",
                    "version-persisted",
                );
                Ok(())
            })
            .unwrap();
    }

    {
        let reopened = SqliteClient::open(&database_path).unwrap();
        reopened.with_connection_result(run_migrations).unwrap();
        let deal_id = reopened
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT deal_id FROM quarry_files WHERE file_id = 'file-persisted'",
                    [],
                    |row| row.get::<_, String>(0),
                )
            })
            .unwrap();
        assert_eq!(deal_id, "DEAL-PERSISTED");
    }

    std::fs::remove_file(database_path).unwrap();
}

#[test]
fn a_current_marker_with_missing_tables_is_rejected_without_recreation() {
    let mut c = Connection::open_in_memory().unwrap();
    run_migrations(&mut c).unwrap();
    c.execute("DROP TABLE deal_metadata", []).unwrap();
    assert!(run_migrations(&mut c).is_err());
    assert!(!table_columns(&c, "users").is_empty());
    assert!(table_columns(&c, "deal_metadata").is_empty());
}
