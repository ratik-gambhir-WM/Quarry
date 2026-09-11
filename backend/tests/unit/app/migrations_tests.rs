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
                deal_id, user_id, deal_name, status, start_date, close_date,
                transaction_type, target_company, primary_buyer, deal_sponsor
            ) VALUES (?1, ?2, 'Project Test', ?3, '2026-01-01', '2026-02-01',
                      'Buy-side', 'Target', 'Buyer', 'Test Capital')
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
    seed_deal(&connection, "DEAL-000001", "Active");
    insert_file_aggregate(&connection, "DEAL-000001", "file-1", "version-1");
    run_migrations(&mut connection).unwrap();

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, LATEST_SCHEMA_VERSION);
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
    seed_deal(&connection, "DEAL-CONSTRAINTS", "Active");

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
    seed_deal(&connection, "DEAL-CASCADE", "Active");
    insert_file_aggregate(&connection, "DEAL-CASCADE", "file-1", "version-1");

    connection
        .execute(
            "UPDATE deals SET status = 'Archived' WHERE deal_id = 'DEAL-CASCADE'",
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
    seed_deal(&connection, "DEAL-FILE-CASCADE", "Active");
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

fn create_actual_version_5_database(connection: &Connection) {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE app_metadata (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );

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

            CREATE TABLE reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                reminder TEXT NOT NULL,
                notes TEXT NOT NULL,
                date TEXT NOT NULL,
                link TEXT NOT NULL,
                time TEXT,
                deal TEXT,
                tag TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE deals (
                deal_id TEXT PRIMARY KEY NOT NULL,
                deal_name TEXT NOT NULL,
                status TEXT NOT NULL,
                start_date TEXT NOT NULL,
                close_date TEXT NOT NULL,
                transaction_type TEXT NOT NULL,
                target_company TEXT NOT NULL,
                primary_buyer TEXT NOT NULL,
                deal_sponsor TEXT NOT NULL,
                CHECK (length(trim(deal_id)) > 0),
                CHECK (length(trim(deal_name)) > 0),
                CHECK (length(trim(status)) > 0),
                CHECK (length(trim(start_date)) > 0),
                CHECK (length(trim(close_date)) > 0),
                CHECK (length(trim(transaction_type)) > 0),
                CHECK (length(trim(target_company)) > 0),
                CHECK (length(trim(primary_buyer)) > 0),
                CHECK (length(trim(deal_sponsor)) > 0)
            );

            CREATE INDEX idx_deals_status ON deals(status);
            CREATE INDEX idx_deals_transaction_type ON deals(transaction_type);
            CREATE INDEX idx_deals_close_date ON deals(close_date);

            CREATE TABLE deal_metadata (
                deal_id TEXT PRIMARY KEY NOT NULL,
                user_id INTEGER NOT NULL,
                key_questions_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(key_questions_json)),
                local_path TEXT,
                sharepoint_link TEXT,
                FOREIGN KEY (deal_id) REFERENCES deals(deal_id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE RESTRICT,
                CHECK (local_path IS NULL OR length(trim(local_path)) > 0),
                CHECK (sharepoint_link IS NULL OR length(trim(sharepoint_link)) > 0),
                CHECK (NOT (local_path IS NOT NULL AND sharepoint_link IS NOT NULL))
            );

            CREATE INDEX idx_deal_metadata_user_id ON deal_metadata(user_id);
            PRAGMA user_version = 5;
            "#,
        )
        .unwrap();
}

#[test]
fn migration_recreates_the_complete_version_6_schema_from_an_actual_version_5_database() {
    let mut connection = Connection::open_in_memory().unwrap();
    create_actual_version_5_database(&connection);
    connection
        .execute_batch(
            r#"
            INSERT INTO users (first_name, last_name, email, api_key, role)
            VALUES ('Avery', 'Analyst', 'analyst@example.com', 'key', 'Analyst');
            INSERT INTO deals (
                deal_id, deal_name, status, start_date, close_date,
                transaction_type, target_company, primary_buyer, deal_sponsor
            ) VALUES (
                'DEAL-V5', 'Project V5', 'Active', '2026-01-01', '2026-02-01',
                'Buy-side', 'Target', 'Buyer', 'Test Capital'
            );
            INSERT INTO deal_metadata (
                deal_id, user_id, key_questions_json, local_path, sharepoint_link
            ) VALUES ('DEAL-V5', 1, '["Why?"]', '/tmp/data-room', NULL);
            "#,
        )
        .unwrap();

    run_migrations(&mut connection).unwrap();

    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        table_columns(&connection, "deals"),
        [
            "deal_id",
            "user_id",
            "deal_name",
            "status",
            "start_date",
            "close_date",
            "transaction_type",
            "target_company",
            "primary_buyer",
            "deal_sponsor",
        ]
    );
    assert_eq!(table_count(&connection, "users"), 0);
    assert_eq!(table_count(&connection, "deals"), 0);
    assert_eq!(table_count(&connection, "deal_metadata"), 0);
    let tables = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        tables,
        [
            "app_metadata",
            "deal_metadata",
            "deals",
            "quarry_file_blobs",
            "quarry_file_versions",
            "quarry_files",
            "reminders",
            "users",
        ]
    );
    assert_eq!(
        table_columns(&connection, "quarry_file_blobs"),
        ["version_id", "file_bytes"]
    );
    connection
        .execute_batch(
            r#"
            INSERT INTO users (first_name, last_name, email, api_key, role)
            VALUES ('Avery', 'Analyst', 'analyst@example.com', 'key', 'Analyst');
            INSERT INTO deals (
                deal_id, user_id, deal_name, status, start_date, close_date,
                transaction_type, target_company, primary_buyer, deal_sponsor
            ) VALUES (
                'DEAL-V6', 1, 'Project V6', 'Active', '2026-01-01', '2026-02-01',
                'Buy-side', 'Target', 'Buyer', 'Test Capital'
            )
            "#,
        )
        .unwrap();
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
                seed_deal(connection, "DEAL-PERSISTED", "Active");
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
