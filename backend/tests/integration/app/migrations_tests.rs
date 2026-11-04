use super::*;

fn seed_deal(connection: &Connection, deal_id: &str, status: &str) {
    connection
        .execute(
            "INSERT INTO users (first_name, last_name, email, username, password_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["Avery", "Analyst", format!("{deal_id}@example.com"), deal_id, "unusable-test-hash"],
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
    if table_columns(connection, "quarry_files")
        .iter()
        .all(|c| c != "workspace_id")
    {
        connection.execute("INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES(?1,?2,'owner','now','now')",rusqlite::params![deal_id,user_id]).unwrap();
        connection
            .execute(
                "INSERT INTO deal_access_revisions(deal_id) VALUES(?1)",
                [deal_id],
            )
            .unwrap();
    }
}

fn insert_file_aggregate(connection: &Connection, deal_id: &str, file_id: &str, version_id: &str) {
    if table_columns(connection, "quarry_files")
        .iter()
        .any(|c| c == "workspace_id")
    {
        connection.execute("INSERT INTO quarry_files(file_id,deal_id,workspace_id,display_name,metadata_json,created_at,updated_at) VALUES(?1,?2,'1','report.pdf','{}','now','now')",rusqlite::params![file_id,deal_id]).unwrap();
    } else {
        connection.execute("INSERT INTO quarry_files(file_id,deal_id,display_name,metadata_json,created_at,updated_at) VALUES(?1,?2,'report.pdf','{}','now','now')",rusqlite::params![file_id,deal_id]).unwrap();
    }
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
    seed_deal(&connection, "DEAL-000001", "prepare");
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
            "uploaded_by_user_id",
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
    assert_eq!(index_count, 4);
}

#[test]
fn file_schema_enforces_foreign_keys_json_and_one_current_version() {
    let mut connection = Connection::open_in_memory().unwrap();
    run_migrations(&mut connection).unwrap();
    seed_deal(&connection, "DEAL-CONSTRAINTS", "prepare");

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
    seed_deal(&connection, "DEAL-CASCADE", "prepare");
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
    seed_deal(&connection, "DEAL-FILE-CASCADE", "prepare");
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
    for marker in [0, 5, 6, 7, 8, 10, 11] {
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
            insert_file_aggregate(c, "DEAL-OLD", "old-file", "old-version");
            c.pragma_update(None, "user_version", 11)?;
            Ok(())
        })
        .unwrap();
    reset_development(&sqlite).unwrap();
    sqlite
        .with_connection(|c| {
            assert_eq!(table_count(c, "users"), 0);
            assert_eq!(table_count(c, "deals"), 0);
            assert_eq!(table_count(c, "quarry_files"), 0);
            assert_eq!(table_count(c, "quarry_file_versions"), 0);
            assert_eq!(table_count(c, "quarry_file_blobs"), 0);
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
                seed_deal(connection, "DEAL-PERSISTED", "prepare");
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

fn schema_nine() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    connection
        .execute_batch(include_str!("../../fixtures/schema-v9.sql"))
        .unwrap();
    connection
}

fn product_snapshot(connection: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    // All columns, including timestamps and binary bytes, must remain identical.
    [
        "app_metadata",
        "users",
        "companies",
        "deals",
        "deal_parties",
        "deal_scopes",
        "deal_metadata",
        "assistant_threads",
        "assistant_messages",
        "reminders",
        "quarry_files",
        "quarry_file_versions",
        "quarry_file_blobs",
    ]
    .into_iter()
    .map(|table| {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        let columns = statement.column_count();
        statement
            .query_map([], |row| {
                (0..columns)
                    .map(|index| row.get(index))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    })
    .collect()
}

#[test]
fn schema_nine_requires_explicit_reset_and_retains_records_on_rejection() {
    let mut connection = schema_nine();
    connection.execute_batch("INSERT INTO users (first_name,last_name,email,api_key,role) VALUES ('Keep','Profile','retained@example.com','legacy-unused-key','Analyst'); INSERT INTO app_metadata VALUES ('retained','value');").unwrap();
    let before = product_snapshot(&connection);
    assert!(matches!(
        run_migrations(&mut connection),
        Err(MigrationError::UnsupportedSchemaVersion {
            found: 9,
            supported: 16
        })
    ));
    assert_eq!(product_snapshot(&connection), before);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        9
    );
}

#[test]
fn failed_schema_nine_upgrade_rolls_back_partial_ddl_and_marker() {
    let mut connection = schema_nine();

    // This collision occurs after several new tables have already been created.
    connection.execute_batch("CREATE TABLE template_previews (value TEXT); INSERT INTO template_previews VALUES ('retained');").unwrap();
    let before = product_snapshot(&connection);
    for _ in 0..2 {
        assert!(run_migrations(&mut connection).is_err());
        assert_eq!(product_snapshot(&connection), before);
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            9
        );
        assert!(table_columns(&connection, "templates").is_empty());
        assert!(table_columns(&connection, "template_assets").is_empty());
        assert_eq!(
            connection
                .query_row("SELECT value FROM template_previews", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "retained"
        );
        assert_eq!(
            connection
                .pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}

#[test]
fn current_schema_with_missing_template_table_is_rejected_without_recreation() {
    let mut connection = Connection::open_in_memory().unwrap();
    run_migrations(&mut connection).unwrap();
    seed_deal(&connection, "DEAL-UPGRADE", "prepare");
    let before = product_snapshot(&connection);
    connection
        .execute("DROP TABLE template_assets", [])
        .unwrap();
    assert!(run_migrations(&mut connection).is_err());
    assert_eq!(product_snapshot(&connection), before);
    assert!(table_columns(&connection, "template_assets").is_empty());
}

#[test]
fn all_diligence_stages_roundtrip_and_retired_codes_fail_sqlite() {
    let mut connection = Connection::open_in_memory().unwrap();
    run_migrations(&mut connection).unwrap();
    seed_deal(&connection, "DEAL-STAGES", "pipeline");
    for code in [
        "pipeline",
        "prepare",
        "review_plan_discovery",
        "interviews_site_visit",
        "debrief_flash_report",
        "follow_ups_analysis",
        "qa_readout_closeout",
        "closed",
        "pencils_down",
    ] {
        connection
            .execute(
                "UPDATE deals SET status = ?1 WHERE deal_id = 'DEAL-STAGES'",
                [code],
            )
            .unwrap();
        run_migrations(&mut connection).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT status FROM deals WHERE deal_id = 'DEAL-STAGES'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            code
        );
    }
    for code in ["active", "on_hold", "completed", "cancelled", "unknown"] {
        assert!(connection
            .execute(
                "UPDATE deals SET status = ?1 WHERE deal_id = 'DEAL-STAGES'",
                [code]
            )
            .is_err());
    }
}

#[test]
fn forged_current_marker_with_retired_status_constraint_is_rejected() {
    let mut connection = Connection::open_in_memory().unwrap();
    let old = SCHEMA.replace("'pipeline', 'prepare', 'review_plan_discovery', 'interviews_site_visit', 'debrief_flash_report', 'follow_ups_analysis', 'qa_readout_closeout', 'closed', 'pencils_down'", "'pipeline', 'active', 'completed', 'on_hold', 'cancelled'");
    connection.execute_batch(&old).unwrap();
    connection.execute_batch(TEMPLATE_SCHEMA).unwrap();
    seed_deal(&connection, "DEAL-OLD", "active");
    connection
        .pragma_update(None, "user_version", LATEST_SCHEMA_VERSION)
        .unwrap();
    assert!(run_migrations(&mut connection).is_err());
    assert_eq!(table_count(&connection, "deals"), 1);
}

fn notebook_schema_twelve() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection.execute_batch(TEMPLATE_SCHEMA).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    connection.pragma_update(None, "user_version", 12).unwrap();
    seed_deal(&connection, "DEAL-PRESERVED", "prepare");
    insert_file_aggregate(
        &connection,
        "DEAL-PRESERVED",
        "preserved-file",
        "preserved-version",
    );
    connection
}
#[test]
fn notebook_schema_upgrade_preserves_existing_product_records_and_reopens() {
    let mut connection = notebook_schema_twelve();
    let before = table_count(&connection, "quarry_file_blobs");
    run_migrations(&mut connection).unwrap();
    run_migrations(&mut connection).unwrap();
    assert_eq!(table_count(&connection, "quarry_file_blobs"), before);
    assert_eq!(table_count(&connection, "users"), 1);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        LATEST_SCHEMA_VERSION
    );
    check_foreign_keys(&connection).unwrap();
}
#[test]
fn notebook_schema_upgrade_ddl_and_marker_roll_back_on_failure() {
    let mut connection = notebook_schema_twelve();
    connection
        .execute_batch("CREATE TABLE notebook_notes (wrong TEXT)")
        .unwrap();
    assert!(run_migrations(&mut connection).is_err());
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        12
    );
    assert_eq!(table_count(&connection, "users"), 1);
    assert!(connection.prepare("SELECT * FROM notebooks").is_err());
}
#[test]
fn notebook_schema_rejects_forged_current_constraints_without_repairing() {
    let mut connection = notebook_schema_twelve();
    run_migrations(&mut connection).unwrap();
    connection
        .execute_batch("DROP INDEX idx_notebook_one_active_save")
        .unwrap();
    assert!(run_migrations(&mut connection).is_err());
    assert_eq!(table_count(&connection, "users"), 1);
}
#[test]
fn notebook_schema_composite_fk_and_deferred_cascade_protect_relationships() {
    let mut connection = notebook_schema_twelve();
    run_migrations(&mut connection).unwrap();
    connection.execute_batch("INSERT INTO notebooks VALUES('b1',1,NULL,'One','now','now'); INSERT INTO notebooks VALUES('b2',1,NULL,'Two','now','now'); INSERT INTO notebook_folders VALUES('f1','b1','Folder','now','now'); INSERT INTO notebook_folders VALUES('f2','b1','Folder','now','now'); INSERT INTO notebook_notes(note_id,notebook_id,folder_id,created_at,updated_at) VALUES('n1','b1','f1','now','now'),('n2','b1','f2','now','now'),('n3','b1',NULL,'now','now');").unwrap();
    assert!(connection.execute("INSERT INTO notebook_notes(note_id,notebook_id,folder_id,created_at,updated_at) VALUES('bad','b2','f1','now','now')",[]).is_err());
    assert!(connection
        .execute("DELETE FROM notebook_folders WHERE folder_id='f1'", [])
        .is_err());
    connection
        .execute("DELETE FROM notebooks WHERE notebook_id='b1'", [])
        .unwrap();
    assert_eq!(table_count(&connection, "notebook_notes"), 0);
    assert_eq!(table_count(&connection, "notebooks"), 1);
    check_foreign_keys(&connection).unwrap();
}

#[test]
fn notebook_upgrade_rejects_forged_marker_twelve_indexes_without_altering_data() {
    let mut connection = notebook_schema_twelve();
    connection
        .execute_batch("DROP INDEX idx_auth_sessions_owner")
        .unwrap();
    assert!(run_migrations(&mut connection).is_err());
    assert_eq!(table_count(&connection, "users"), 1);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        12
    );
    assert!(connection.prepare("SELECT * FROM notebooks").is_err());
}

fn membership_schema_thirteen() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection.execute_batch(TEMPLATE_SCHEMA).unwrap();
    connection.execute_batch(NOTEBOOK_SCHEMA).unwrap();
    connection.pragma_update(None, "user_version", 13).unwrap();
    connection
}
#[test]
fn shared_upgrade_preserves_ids_blobs_and_unknown_attribution_and_backfills_exactly_one_owner() {
    let mut c = membership_schema_thirteen();
    seed_deal(&c, "DEAL-SHARED-LEGACY", "pipeline");
    insert_file_aggregate(
        &c,
        "DEAL-SHARED-LEGACY",
        "unchanged-file",
        "unchanged-version",
    );
    c.execute("UPDATE deals SET archived_at='archived'", [])
        .unwrap();
    let before: Vec<u8> = c
        .query_row("SELECT file_bytes FROM quarry_file_blobs", [], |r| r.get(0))
        .unwrap();
    run_migrations(&mut c).unwrap();
    run_migrations(&mut c).unwrap();
    assert!(!table_columns(&c, "quarry_files").contains(&"workspace_id".into()));
    assert_eq!(
        c.query_row("SELECT file_id FROM quarry_files", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "unchanged-file"
    );
    assert_eq!(
        c.query_row("SELECT version_id FROM quarry_file_versions", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "unchanged-version"
    );
    assert_eq!(
        c.query_row(
            "SELECT uploaded_by_user_id FROM quarry_file_versions",
            [],
            |r| r.get::<_, Option<i64>>(0)
        )
        .unwrap(),
        None
    );
    assert_eq!(
        c.query_row("SELECT file_bytes FROM quarry_file_blobs", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        before
    );
    assert_eq!(table_count(&c, "deal_memberships"), 1);
    assert_eq!(table_count(&c, "deal_activity_events"), 1);
    assert_eq!(
        c.query_row("SELECT role FROM deal_memberships", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "owner"
    );
    assert_eq!(
        c.query_row("SELECT actor_user_id FROM deal_activity_events", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .unwrap(),
        None
    );
    assert_eq!(table_count(&c, "document_identity_manifests"), 0);
    assert_eq!(
        c.pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn shared_upgrade_failure_rolls_back_and_restart_rejects_missing_owner_without_repair() {
    let mut old = membership_schema_thirteen();
    seed_deal(&old, "DEAL-RETAIN", "pipeline");
    old.execute_batch("CREATE VIEW deal_teams AS SELECT deal_id FROM deals;")
        .unwrap();
    assert!(run_migrations(&mut old).is_err());
    assert_eq!(table_count(&old, "deals"), 1);
    assert_eq!(
        old.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        13
    );
    assert!(old.prepare("SELECT * FROM deal_memberships").is_err());
    assert!(table_columns(&old, "quarry_files").contains(&"workspace_id".into()));
    assert_eq!(
        old.pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    let mut current = Connection::open_in_memory().unwrap();
    run_migrations(&mut current).unwrap();
    seed_deal(&current, "DEAL-BROKEN", "pipeline");
    current.execute("DELETE FROM deal_memberships", []).unwrap();
    assert!(run_migrations(&mut current).is_err());
    assert_eq!(table_count(&current, "deal_memberships"), 0);
}
#[test]
fn shared_fresh_and_upgraded_owned_layouts_are_identical() {
    let mut fresh = Connection::open_in_memory().unwrap();
    run_migrations(&mut fresh).unwrap();
    let mut upgraded = membership_schema_thirteen();
    run_migrations(&mut upgraded).unwrap();
    fn ddl(c: &Connection) -> Vec<(String, String)> {
        c.prepare("SELECT name,sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().map(Result::unwrap).collect()
    }
    assert_eq!(ddl(&fresh), ddl(&upgraded));
}

fn image_schema_fourteen() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    for schema in [
        SCHEMA,
        TEMPLATE_SCHEMA,
        NOTEBOOK_SCHEMA,
        MEMBERSHIP_SCHEMA,
        DOCUMENT_SCOPE_UPGRADE,
    ] {
        connection.execute_batch(schema).unwrap();
    }
    connection.pragma_update(None, "user_version", 14).unwrap();
    seed_deal(&connection, "DEAL-IMAGE", "prepare");
    insert_file_aggregate(&connection, "DEAL-IMAGE", "image-file", "image-version");
    connection
}

#[test]
fn image_recovery_upgrade_preserves_schema_fourteen_records_and_reopens() {
    let mut connection = image_schema_fourteen();
    let before = product_snapshot(&connection);
    run_migrations(&mut connection).unwrap();
    run_migrations(&mut connection).unwrap();
    assert_eq!(product_snapshot(&connection), before);
    assert_eq!(table_count(&connection, "document_image_recovery"), 0);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        LATEST_SCHEMA_VERSION
    );
    check_foreign_keys(&connection).unwrap();
}

#[test]
fn image_recovery_upgrade_rejects_forged_layout_without_changing_records_or_marker() {
    let mut connection = image_schema_fourteen();
    connection
        .execute_batch("DROP INDEX uq_quarry_file_versions_current;")
        .unwrap();
    let before = product_snapshot(&connection);
    assert!(run_migrations(&mut connection).is_err());
    assert_eq!(product_snapshot(&connection), before);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        14
    );
    assert!(table_columns(&connection, "document_image_recovery").is_empty());
}

#[test]
fn image_recovery_upgrade_rolls_back_new_table_and_marker_when_owner_validation_fails() {
    let mut connection = image_schema_fourteen();
    connection
        .execute("DELETE FROM deal_memberships", [])
        .unwrap();
    let before = product_snapshot(&connection);
    assert!(run_migrations(&mut connection).is_err());
    assert_eq!(product_snapshot(&connection), before);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        14
    );
    assert!(table_columns(&connection, "document_image_recovery").is_empty());
    assert_eq!(
        connection
            .pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

fn classification_schema_fifteen() -> Connection {
    let connection = image_schema_fourteen();
    connection.execute_batch(IMAGE_RECOVERY_SCHEMA).unwrap();
    connection.pragma_update(None, "user_version", 15).unwrap();
    connection
        .execute("INSERT INTO templates VALUES('retained','{}')", [])
        .unwrap();
    connection.execute("INSERT INTO slide_classifications(template_id,status,input_fingerprint,prompt_version,schema_version,attempt_count,subject_embedding,embedding_dimensions) VALUES('retained','failed','fingerprint','prompt',2,2,X'01020304',1)", []).unwrap();
    connection
}
#[test]
fn classification_status_upgrade_preserves_artifacts_and_is_idempotent() {
    let mut connection = classification_schema_fifteen();
    let before = product_snapshot(&connection);
    run_migrations(&mut connection).unwrap();
    run_migrations(&mut connection).unwrap();
    assert_eq!(product_snapshot(&connection), before);
    assert_eq!(connection.query_row("SELECT status,attempt_count,subject_embedding FROM template_classification_status WHERE template_id='retained'", [], |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,Vec<u8>>(2)?))).unwrap(), ("failed".into(),2,vec![1,2,3,4]));
    assert!(table_columns(&connection, "slide_classifications").is_empty());
    check_foreign_keys(&connection).unwrap();
}
#[test]
fn classification_status_upgrade_recovers_old_unleased_processing_rows() {
    let mut connection = classification_schema_fifteen();
    connection
        .execute(
            "UPDATE slide_classifications SET status='processing',attempt_count=1",
            [],
        )
        .unwrap();
    run_migrations(&mut connection).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT status,attempt_count,last_error_code FROM template_classification_status",
                [],
                |r| Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?
                ))
            )
            .unwrap(),
        ("pending".into(), 0, "classification_interrupted".into())
    );
}
#[test]
fn classification_status_upgrade_rejects_damaged_layout_and_rolls_back_rename() {
    for damage in [false, true] {
        let mut connection = classification_schema_fifteen();
        if damage {
            connection
                .execute(
                    "DROP INDEX slide_classifications_classification_jobs_idx",
                    [],
                )
                .unwrap();
        } else {
            connection
                .execute("DELETE FROM deal_memberships", [])
                .unwrap();
        }
        let before = product_snapshot(&connection);
        assert!(run_migrations(&mut connection).is_err());
        assert_eq!(product_snapshot(&connection), before);
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            15
        );
        assert_eq!(table_count(&connection, "slide_classifications"), 1);
        assert!(table_columns(&connection, "template_classification_status").is_empty());
    }
}
