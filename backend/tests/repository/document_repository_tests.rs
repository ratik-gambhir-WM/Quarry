use super::*;
use std::path::Path;

use crate::{
    bootstrap::migrate,
    core::{clients::sqlite::SqliteClient, models::document::Document},
    services::document_service::build_file_persistence_input,
    utils::{document_id_from_content, file_version_id, sha256_hex},
};

const OWNER: &str = "analyst@example.com";
const DEAL_ID: &str = "DEAL-FILES";

fn seed_user_and_deals(state: &SqliteClient, owner: &str, deals: &[(&str, &str)]) {
    state
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO users (first_name, last_name, email, api_key, role) VALUES ('Avery', 'Analyst', ?1, 'key', 'Analyst')",
                [owner],
            )?;
            let user_id = connection.last_insert_rowid();
            for (deal_id, status) in deals {
                connection.execute(
                    r#"
                    INSERT INTO deals (
                        deal_id, user_id, deal_name, status, start_date, close_date,
                        transaction_type, target_company, primary_buyer, deal_sponsor
                    ) VALUES (?1, ?2, 'Project Test', ?3, '2026-01-01', '2026-02-01',
                              'Buy-side', 'Target', 'Buyer', 'Test Capital')
                    "#,
                    rusqlite::params![deal_id, user_id, status],
                )?;
            }
            Ok(())
        })
        .unwrap();
}

fn test_state() -> SqliteClient {
    let state = empty_sqlite();
    seed_user_and_deals(&state, OWNER, &[(DEAL_ID, "Active")]);
    state
}

fn empty_sqlite() -> SqliteClient {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrate(&sqlite).unwrap();
    sqlite
}

fn document(file_id: &str, owner: &str, filename: &str, bytes: &[u8]) -> Document {
    let content_hash = sha256_hex(bytes);
    Document {
        file_id: file_id.to_string(),
        document_id: document_id_from_content(owner, &content_hash),
        user_id: owner.to_string(),
        file_name: filename.to_string(),
        source_type: Path::new(filename)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase(),
        local_path: Some(format!("/documents/{filename}")),
        file_size_bytes: bytes.len() as u64,
        token_count: 42,
        content_hash,
        rendered_pdf_path: Some(format!("/rendered/{filename}.pdf")),
    }
}

async fn persist_file_blob(
    sqlite: &SqliteClient,
    deal_id: &str,
    document: &Document,
    file_bytes: Vec<u8>,
) -> Result<PersistedFileIdentity, String> {
    let file_persistence = build_file_persistence_input(deal_id, document, file_bytes)?;
    super::persist_file_blob(sqlite, file_persistence).await
}

fn table_counts(state: &SqliteClient) -> (i64, i64, i64) {
    state
        .with_connection(|connection| {
            Ok((
                connection.query_row("SELECT COUNT(*) FROM quarry_files", [], |row| row.get(0))?,
                connection.query_row("SELECT COUNT(*) FROM quarry_file_versions", [], |row| {
                    row.get(0)
                })?,
                connection.query_row("SELECT COUNT(*) FROM quarry_file_blobs", [], |row| {
                    row.get(0)
                })?,
            ))
        })
        .unwrap()
}

#[tokio::test]
async fn persists_a_complete_file_aggregate_and_returns_the_logical_file_id() {
    let state = test_state();
    let file_bytes = b"binary\0file\xffbytes".to_vec();
    let document = document("file-1", OWNER, "report.pdf", &file_bytes);
    let expected_version_id = file_version_id(&document.file_id, &document.content_hash);

    let insert_blob_result = persist_file_blob(&state, DEAL_ID, &document, file_bytes.clone())
        .await
        .unwrap();

    assert_eq!(
        insert_blob_result,
        PersistedFileIdentity {
            file_id: "file-1".to_string(),
            workspace_id: OWNER.to_string(),
            display_name: "report.pdf".to_string(),
            version_id: expected_version_id.clone(),
        }
    );
    assert_eq!(table_counts(&state), (1, 1, 1));
    let stored = state
        .with_connection(|connection| {
            connection.query_row(
                r#"
                SELECT f.deal_id, f.workspace_id, f.display_name, f.source_uri, f.metadata_json,
                       v.version_id, v.version_number, v.mime_type, v.content_sha256,
                       v.byte_size, v.is_current, b.file_bytes
                FROM quarry_files f
                JOIN quarry_file_versions v ON v.file_id = f.file_id
                JOIN quarry_file_blobs b ON b.version_id = v.version_id
                WHERE f.file_id = ?1
                "#,
                [&document.file_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, Vec<u8>>(11)?,
                    ))
                },
            )
        })
        .unwrap();
    assert_eq!(stored.0, DEAL_ID);
    assert_eq!(stored.1, OWNER);
    assert_eq!(stored.2, "report.pdf");
    assert_eq!(stored.3.as_deref(), Some("/documents/report.pdf"));
    let metadata: serde_json::Value = serde_json::from_str(&stored.4).unwrap();
    assert_eq!(metadata["documentId"], document.document_id);
    assert_eq!(metadata["sourceType"], "pdf");
    assert_eq!(metadata["tokenCount"], 42);
    assert_eq!(stored.5, expected_version_id);
    assert_eq!(stored.6, 1);
    assert_eq!(stored.7, "application/pdf");
    assert_eq!(stored.8, document.content_hash);
    assert_eq!(stored.9, file_bytes.len() as i64);
    assert_eq!(stored.10, 1);
    assert_eq!(stored.11, file_bytes);
}

#[tokio::test]
async fn content_hash_lookup_is_scoped_to_the_target_deal_attachment() {
    let state = empty_sqlite();
    seed_user_and_deals(
        &state,
        OWNER,
        &[("DEAL-FIRST", "Active"), ("DEAL-SECOND", "Active")],
    );
    let bytes = b"identical attachment bytes".to_vec();
    let first = document("file-first", OWNER, "report.pdf", &bytes);
    let second = document("file-second", OWNER, "copy.pdf", &bytes);
    persist_file_blob(&state, "DEAL-FIRST", &first, bytes.clone())
        .await
        .unwrap();
    persist_file_blob(&state, "DEAL-SECOND", &second, bytes)
        .await
        .unwrap();

    let first_match =
        find_current_sqlite_file_by_content_hash(&state, "DEAL-FIRST", OWNER, &first.content_hash)
            .await
            .unwrap()
            .unwrap();
    let second_match =
        find_current_sqlite_file_by_content_hash(&state, "DEAL-SECOND", OWNER, &first.content_hash)
            .await
            .unwrap()
            .unwrap();

    assert_eq!(first_match.file_id, "file-first");
    assert_eq!(second_match.file_id, "file-second");
    assert_ne!(first_match.version_id, second_match.version_id);
    assert!(find_current_sqlite_file_by_content_hash(
        &state,
        "DEAL-MISSING",
        OWNER,
        &first.content_hash,
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn deterministic_validation_failures_leave_all_file_tables_unchanged() {
    let state = test_state();
    let bytes = b"valid bytes".to_vec();
    let valid = document("file-valid", OWNER, "report.pdf", &bytes);
    let mut cases = Vec::new();

    cases.push(("", valid.clone(), bytes.clone()));
    let mut empty_file_id = valid.clone();
    empty_file_id.file_id.clear();
    cases.push((DEAL_ID, empty_file_id, bytes.clone()));
    cases.push((DEAL_ID, valid.clone(), Vec::new()));
    let mut wrong_hash = valid.clone();
    wrong_hash.content_hash = "0".repeat(64);
    cases.push((DEAL_ID, wrong_hash, bytes.clone()));
    let mut wrong_document_id = valid.clone();
    wrong_document_id.document_id = "wrong".to_string();
    cases.push((DEAL_ID, wrong_document_id, bytes.clone()));
    let mut wrong_size = valid.clone();
    wrong_size.file_size_bytes += 1;
    cases.push((DEAL_ID, wrong_size, bytes.clone()));
    let unsupported = document("unsupported", OWNER, "report.exe", &bytes);
    cases.push((DEAL_ID, unsupported, bytes.clone()));
    let mut wrong_source_type = valid.clone();
    wrong_source_type.source_type = "docx".to_string();
    cases.push((DEAL_ID, wrong_source_type, bytes.clone()));
    let mut unnormalized_owner = valid.clone();
    unnormalized_owner.user_id = "Analyst@Example.com".to_string();
    cases.push((DEAL_ID, unnormalized_owner, bytes.clone()));

    for (deal_id, document, file_bytes) in cases {
        assert!(persist_file_blob(&state, deal_id, &document, file_bytes)
            .await
            .is_err());
        assert_eq!(table_counts(&state), (0, 0, 0));
    }
}

#[tokio::test]
async fn deal_validation_rejects_missing_archived_and_differently_owned_deals() {
    let state = empty_sqlite();
    seed_user_and_deals(
        &state,
        OWNER,
        &[(DEAL_ID, "Active"), ("DEAL-ARCHIVED", "Archived")],
    );
    seed_user_and_deals(&state, "other@example.com", &[("DEAL-OTHER", "Active")]);
    let bytes = b"deal validation".to_vec();
    let document = document("file-deal", OWNER, "report.pdf", &bytes);

    for deal_id in ["DEAL-MISSING", "DEAL-ARCHIVED", "DEAL-OTHER"] {
        assert!(persist_file_blob(&state, deal_id, &document, bytes.clone())
            .await
            .is_err());
        assert_eq!(table_counts(&state), (0, 0, 0));
    }
}

#[tokio::test]
async fn identical_retry_is_idempotent_and_can_restore_that_version_as_current() {
    let state = test_state();
    let first_bytes = b"first version".to_vec();
    let second_bytes = b"second version".to_vec();
    let first = document("file-versions", OWNER, "report.pdf", &first_bytes);
    let second = document("file-versions", OWNER, "report.pdf", &second_bytes);

    let first_insert_blob_result = persist_file_blob(&state, DEAL_ID, &first, first_bytes.clone())
        .await
        .unwrap();
    let retry_insert_blob_result = persist_file_blob(&state, DEAL_ID, &first, first_bytes.clone())
        .await
        .unwrap();
    assert_eq!(retry_insert_blob_result, first_insert_blob_result);
    assert_eq!(table_counts(&state), (1, 1, 1));

    let second_insert_blob_result = persist_file_blob(&state, DEAL_ID, &second, second_bytes)
        .await
        .unwrap();
    assert_eq!(
        second_insert_blob_result.file_id,
        first_insert_blob_result.file_id
    );
    assert_ne!(
        second_insert_blob_result.version_id,
        first_insert_blob_result.version_id
    );
    assert_eq!(table_counts(&state), (1, 2, 2));
    persist_file_blob(&state, DEAL_ID, &first, first_bytes)
        .await
        .unwrap();
    assert_eq!(table_counts(&state), (1, 2, 2));

    let versions = state
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT version_number, content_sha256, is_current FROM quarry_file_versions WHERE file_id = 'file-versions' ORDER BY version_number",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            let versions = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(versions)
        })
        .unwrap();
    assert_eq!(versions[0], (1, first.content_hash, 1));
    assert_eq!(versions[1], (2, second.content_hash, 0));
}

#[tokio::test]
async fn same_bytes_under_different_logical_files_have_distinct_versions() {
    let state = test_state();
    let bytes = b"shared bytes".to_vec();
    let first = document("logical-1", OWNER, "first.pdf", &bytes);
    let second = document("logical-2", OWNER, "second.pdf", &bytes);

    persist_file_blob(&state, DEAL_ID, &first, bytes.clone())
        .await
        .unwrap();
    persist_file_blob(&state, DEAL_ID, &second, bytes)
        .await
        .unwrap();

    assert_eq!(table_counts(&state), (2, 2, 2));
    assert_ne!(
        file_version_id(&first.file_id, &first.content_hash),
        file_version_id(&second.file_id, &second.content_hash)
    );
}

#[tokio::test]
async fn reusing_a_file_id_cannot_move_or_restore_it() {
    let state = empty_sqlite();
    seed_user_and_deals(
        &state,
        OWNER,
        &[(DEAL_ID, "Active"), ("DEAL-SECOND", "Active")],
    );
    let bytes = b"attached bytes".to_vec();
    let document = document("attached-file", OWNER, "report.pdf", &bytes);
    persist_file_blob(&state, DEAL_ID, &document, bytes.clone())
        .await
        .unwrap();

    assert!(
        persist_file_blob(&state, "DEAL-SECOND", &document, bytes.clone())
            .await
            .is_err()
    );
    let deal_id = state
        .with_connection(|connection| {
            connection.query_row(
                "SELECT deal_id FROM quarry_files WHERE file_id = 'attached-file'",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap();
    assert_eq!(deal_id, DEAL_ID);

    state
        .with_connection(|connection| {
            connection.execute(
                "UPDATE quarry_files SET workspace_id = 'other@example.com' WHERE file_id = 'attached-file'",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(persist_file_blob(&state, DEAL_ID, &document, bytes.clone())
        .await
        .is_err());
    let workspace_id = state
        .with_connection(|connection| {
            connection.query_row(
                "SELECT workspace_id FROM quarry_files WHERE file_id = 'attached-file'",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap();
    assert_eq!(workspace_id, "other@example.com");

    state
        .with_connection(|connection| {
            connection.execute(
                "UPDATE quarry_files SET workspace_id = ?1, deleted_at = '2026-01-01T00:00:00Z' WHERE file_id = 'attached-file'",
                [OWNER],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(persist_file_blob(&state, DEAL_ID, &document, bytes)
        .await
        .is_err());
    assert_eq!(table_counts(&state), (1, 1, 1));
}

#[tokio::test]
async fn file_table_failure_prevents_child_writes() {
    let state = test_state();
    state
        .with_connection(|connection| {
            connection.execute_batch(
                "CREATE TRIGGER reject_file BEFORE INSERT ON quarry_files BEGIN SELECT RAISE(ABORT, 'file rejected'); END;",
            )
        })
        .unwrap();
    let bytes = b"file trigger".to_vec();
    let document = document("trigger-file", OWNER, "report.pdf", &bytes);

    let insert_blob_error = persist_file_blob(&state, DEAL_ID, &document, bytes)
        .await
        .unwrap_err();

    assert!(insert_blob_error.contains("file rejected"));
    assert_eq!(table_counts(&state), (0, 0, 0));
}

#[tokio::test]
async fn version_insert_failure_rolls_back_the_parent_file() {
    let state = test_state();
    state
        .with_connection(|connection| {
            connection.execute_batch(
                "CREATE TRIGGER reject_version BEFORE INSERT ON quarry_file_versions BEGIN SELECT RAISE(ABORT, 'version rejected'); END;",
            )
        })
        .unwrap();
    let bytes = b"version trigger".to_vec();
    let document = document("trigger-version", OWNER, "report.pdf", &bytes);

    let insert_blob_error = persist_file_blob(&state, DEAL_ID, &document, bytes)
        .await
        .unwrap_err();

    assert!(insert_blob_error.contains("version rejected"));
    assert_eq!(table_counts(&state), (0, 0, 0));
}

#[tokio::test]
async fn final_identity_read_back_failure_rolls_back_the_aggregate() {
    let state = test_state();
    state
        .with_connection(|connection| {
            connection.execute_batch(
                "CREATE TRIGGER erase_version_after_blob_insert AFTER INSERT ON quarry_file_blobs BEGIN DELETE FROM quarry_file_versions WHERE version_id = NEW.version_id; END;",
            )
        })
        .unwrap();
    let bytes = b"read-back rollback".to_vec();
    let document = document("read-back-file", OWNER, "report.pdf", &bytes);

    let insert_blob_error = persist_file_blob(&state, DEAL_ID, &document, bytes)
        .await
        .unwrap_err();

    assert!(insert_blob_error.contains("read-back returned 0 rows"));
    assert_eq!(table_counts(&state), (0, 0, 0));
}

#[tokio::test]
async fn blob_insert_failure_restores_the_previous_current_version() {
    let state = test_state();
    let first_bytes = b"committed version".to_vec();
    let first = document("blob-rollback", OWNER, "report.pdf", &first_bytes);
    persist_file_blob(&state, DEAL_ID, &first, first_bytes)
        .await
        .unwrap();
    state
        .with_connection(|connection| {
            connection.execute_batch(
                "CREATE TRIGGER reject_blob BEFORE INSERT ON quarry_file_blobs BEGIN SELECT RAISE(ABORT, 'blob rejected'); END;",
            )
        })
        .unwrap();
    let second_bytes = b"rejected version".to_vec();
    let second = document("blob-rollback", OWNER, "renamed.pdf", &second_bytes);

    let insert_blob_error = persist_file_blob(&state, DEAL_ID, &second, second_bytes)
        .await
        .unwrap_err();

    assert!(insert_blob_error.contains("blob rejected"));
    assert_eq!(table_counts(&state), (1, 1, 1));
    let current = state
        .with_connection(|connection| {
            connection.query_row(
                "SELECT f.display_name, v.content_sha256, v.is_current FROM quarry_files f JOIN quarry_file_versions v ON v.file_id = f.file_id WHERE f.file_id = 'blob-rollback'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
            )
        })
        .unwrap();
    assert_eq!(current, ("report.pdf".to_string(), first.content_hash, 1));
}

#[tokio::test]
async fn corrupted_existing_blob_is_not_treated_as_an_idempotent_retry() {
    let state = test_state();
    let bytes = b"original blob".to_vec();
    let original = document("corrupt-file", OWNER, "report.pdf", &bytes);
    persist_file_blob(&state, DEAL_ID, &original, bytes.clone())
        .await
        .unwrap();
    state
        .with_connection(|connection| {
            connection.execute(
                "UPDATE quarry_file_blobs SET file_bytes = X'00' WHERE version_id = ?1",
                [file_version_id(&original.file_id, &original.content_hash)],
            )?;
            Ok(())
        })
        .unwrap();
    let renamed = document("corrupt-file", OWNER, "renamed.pdf", &bytes);

    let insert_blob_error = persist_file_blob(&state, DEAL_ID, &renamed, bytes)
        .await
        .unwrap_err();

    assert!(insert_blob_error.contains("corrupt or collided"));
    let display_name = state
        .with_connection(|connection| {
            connection.query_row(
                "SELECT display_name FROM quarry_files WHERE file_id = 'corrupt-file'",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap();
    assert_eq!(display_name, "report.pdf");
    assert_eq!(table_counts(&state), (1, 1, 1));
}

#[tokio::test]
async fn concurrent_new_versions_receive_sequential_version_numbers() {
    let state = test_state();
    let initial_bytes = b"initial".to_vec();
    let initial = document("concurrent-file", OWNER, "report.pdf", &initial_bytes);
    persist_file_blob(&state, DEAL_ID, &initial, initial_bytes)
        .await
        .unwrap();

    let mut tasks = Vec::new();
    for bytes in [b"second".to_vec(), b"third".to_vec()] {
        let sqlite = state.clone();
        let document = document("concurrent-file", OWNER, "report.pdf", &bytes);
        tasks.push(tokio::spawn(async move {
            persist_file_blob(&sqlite, DEAL_ID, &document, bytes).await
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }

    let version_numbers = state
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT version_number FROM quarry_file_versions WHERE file_id = 'concurrent-file' ORDER BY version_number",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
            let version_numbers = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(version_numbers)
        })
        .unwrap();
    assert_eq!(version_numbers, vec![1, 2, 3]);
    let current_count = state
        .with_connection(|connection| {
            connection.query_row(
                "SELECT COUNT(*) FROM quarry_file_versions WHERE file_id = 'concurrent-file' AND is_current = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .unwrap();
    assert_eq!(current_count, 1);
}

#[test]
fn maps_an_empty_document_response_to_none() {
    let response: HelixDocumentVersionResponse = serde_json::from_value(serde_json::json!({
        "file": { "properties": [] },
        "version": { "properties": [] }
    }))
    .unwrap();

    assert_eq!(
        map_document_version_response(response, OWNER, None, None, None).unwrap(),
        None
    );
}

#[test]
fn rejects_partial_and_mismatched_document_responses() {
    let partial: HelixDocumentVersionResponse = serde_json::from_value(serde_json::json!({
        "file": { "properties": [{
            "workspace_id": OWNER,
            "file_id": "file-1",
            "display_name": "report.pdf"
        }] },
        "version": { "properties": [] }
    }))
    .unwrap();
    assert!(map_document_version_response(partial, OWNER, None, None, None).is_err());

    let mismatch: HelixDocumentVersionResponse = serde_json::from_value(serde_json::json!({
        "file": { "properties": [{
            "workspace_id": OWNER,
            "file_id": "file-1",
            "display_name": "report.pdf"
        }] },
        "version": { "properties": [{
            "workspace_id": OWNER,
            "file_id": "file-2",
            "version_id": "version-1",
            "mime_type": "application/pdf",
            "content_sha256": "hash",
            "byte_size": 1,
            "index_generation": "version-1",
            "indexed_at": "2026-08-26T00:00:00Z"
        }] }
    }))
    .unwrap();
    assert!(map_document_version_response(mismatch, OWNER, None, None, None).is_err());
}
