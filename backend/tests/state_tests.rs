use super::*;
use std::time::Duration;

impl AppState {
    pub(crate) fn in_memory() -> Result<Self, String> {
        let db = SqliteClient::open_in_memory()
            .map_err(|err| format!("failed to open in-memory sqlite database: {err}"))?;
        db.with_connection(initialize_schema)
            .map_err(|err| format!("failed to initialize in-memory sqlite database: {err}"))?;
        Ok(Self {
            db,
            document_jobs: Arc::new(RwLock::new(HashMap::new())),
            document_processing_locks: Arc::new(AsyncMutex::new(HashMap::new())),
            helix: Arc::new(HelixClient::new()?),
        })
    }
}

#[test]
fn schema_initialization_creates_user_owned_deals_and_is_idempotent() {
    let mut connection = Connection::open_in_memory().unwrap();

    initialize_schema(&mut connection).unwrap();
    initialize_schema(&mut connection).unwrap();
    connection
        .execute_batch(
            r#"
            INSERT INTO users (first_name, last_name, email, api_key, role)
            VALUES ('Avery', 'Analyst', 'analyst@example.com', 'key', 'Analyst');
            INSERT INTO deals (
                deal_id, user_id, deal_name, status, start_date, close_date,
                transaction_type, target_company, primary_buyer, deal_sponsor
            ) VALUES (
                'DEAL-000001', 1, 'Project Test', 'Active', '2026-01-01', '2026-02-01',
                'Buy-side', 'Target', 'Buyer', 'Test Capital'
            );
            "#,
        )
        .unwrap();

    let owner_id = connection
        .query_row(
            "SELECT user_id FROM deals WHERE deal_id = 'DEAL-000001'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();

    assert_eq!(owner_id, 1);
    assert!(connection
        .execute(
            r#"
            INSERT INTO deals (
                deal_id, user_id, deal_name, status, start_date, close_date,
                transaction_type, target_company, primary_buyer, deal_sponsor
            ) VALUES (
                'DEAL-INVALID', 999, 'Invalid', 'Active', '2026-01-01', '2026-02-01',
                'Buy-side', 'Target', 'Buyer', 'Sponsor'
            )
            "#,
            [],
        )
        .is_err());
}

#[test]
fn schema_initialization_creates_file_blob_storage_and_preserves_binary_bytes() {
    let mut connection = Connection::open_in_memory().unwrap();

    initialize_schema(&mut connection).unwrap();

    let file_bytes = vec![0, 1, 2, 127, 128, 254, 255];
    connection
        .execute(
            "INSERT INTO quarry_file_blobs (file_id, file_bytes) VALUES (?1, ?2)",
            rusqlite::params!["file-1", &file_bytes],
        )
        .unwrap();

    let stored_bytes: Vec<u8> = connection
        .query_row(
            "SELECT file_bytes FROM quarry_file_blobs WHERE file_id = ?1",
            ["file-1"],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(stored_bytes, file_bytes);
}

#[test]
fn disk_database_preserves_deal_ownership_across_reopen() {
    let database_path = std::env::temp_dir().join(format!(
        "quarry-persistence-test-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));

    {
        let database = SqliteClient::open(&database_path).unwrap();
        database.with_connection(initialize_schema).unwrap();
        database
            .with_connection(|connection| {
                connection.execute_batch(
                    r#"
                    INSERT INTO users (first_name, last_name, email, api_key, role)
                    VALUES ('Avery', 'Analyst', 'analyst@example.com', 'key', 'Analyst');
                    INSERT INTO deals (
                        deal_id, user_id, deal_name, status, start_date, close_date,
                        transaction_type, target_company, primary_buyer, deal_sponsor
                    ) VALUES (
                        'DEAL-PERSISTED', 1, 'Persisted Deal', 'Active',
                        '2026-01-01', '2026-02-01', 'Buy-side', 'Target', 'Buyer', 'Sponsor'
                    );
                    "#,
                )
            })
            .unwrap();
    }

    {
        let reopened = SqliteClient::open(&database_path).unwrap();
        reopened.with_connection(initialize_schema).unwrap();
        let owner_id = reopened
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT user_id FROM deals WHERE deal_id = 'DEAL-PERSISTED'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(owner_id, 1);
    }

    std::fs::remove_file(database_path).unwrap();
}

#[tokio::test]
async fn document_processing_lock_serializes_the_same_identity() {
    let state = AppState::in_memory().unwrap();
    let first_guard = state.lock_document_processing("document-1").await;
    let waiting_state = state.clone();
    let mut waiting = tokio::spawn(async move {
        let _guard = waiting_state.lock_document_processing("document-1").await;
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut waiting)
            .await
            .is_err()
    );
    drop(first_guard);
    tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap();
}
