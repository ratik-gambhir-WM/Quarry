use super::*;
use std::time::Duration;

impl AppState {
    pub(crate) fn in_memory() -> Result<Self, String> {
        let db = SqliteClient::open_in_memory()
            .map_err(|err| format!("failed to open in-memory sqlite database: {err}"))?;
        db.with_connection(|connection| run_migrations(connection))
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
fn migrations_replace_the_legacy_deal_schema_and_preserve_records() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            first_name TEXT NOT NULL, last_name TEXT NOT NULL, email TEXT NOT NULL UNIQUE,
            api_key TEXT NOT NULL, role TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        INSERT INTO users (first_name, last_name, email, api_key, role)
        VALUES ('Avery', 'Analyst', 'analyst@example.com', 'key', 'Analyst');
        CREATE TABLE deals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            deal_name TEXT NOT NULL, main_data_room_folder TEXT NOT NULL,
            deal_type TEXT NOT NULL, pe_firm TEXT NOT NULL,
            target_company TEXT, buyer_or_platform_company TEXT,
            parent_or_seller_company TEXT, carve_out_business TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        INSERT INTO deals (
            deal_name, main_data_room_folder, deal_type, pe_firm,
            target_company, buyer_or_platform_company
        ) VALUES ('Project Test', '/tmp/data-room', 'Buy-side', 'Test Capital', 'Target', 'Buyer');
        CREATE TABLE deal_metadata (
            id INTEGER PRIMARY KEY AUTOINCREMENT, deal_id INTEGER NOT NULL,
            key_questions_json TEXT NOT NULL DEFAULT '[]',
            document_count INTEGER NOT NULL DEFAULT 0,
            data_room_size_bytes INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        INSERT INTO deal_metadata (deal_id, key_questions_json) VALUES (1, '["Why?"]');
        PRAGMA user_version = 4;
    "#,
        )
        .unwrap();

    run_migrations(&connection).unwrap();

    assert!(column_exists(&connection, "deals", "deal_id").unwrap());
    assert!(!column_exists(&connection, "deals", "main_data_room_folder").unwrap());
    assert!(column_exists(&connection, "deal_metadata", "user_id").unwrap());
    assert!(column_exists(&connection, "deal_metadata", "local_path").unwrap());
    assert!(column_exists(&connection, "deal_metadata", "sharepoint_link").unwrap());
    let migrated: (String, String, String) = connection
        .query_row(
            "SELECT deal_id, transaction_type, primary_buyer FROM deals",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        migrated,
        (
            "DEAL-000001".to_string(),
            "Buy-side".to_string(),
            "Buyer".to_string()
        )
    );
    let metadata: (i64, String, Option<String>) = connection
        .query_row(
            "SELECT user_id, key_questions_json, local_path FROM deal_metadata",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        metadata,
        (
            1,
            "[\"Why?\"]".to_string(),
            Some("/tmp/data-room".to_string())
        )
    );
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        5
    );
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
