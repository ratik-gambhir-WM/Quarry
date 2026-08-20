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
fn migrations_remove_legacy_investment_thesis_column() {
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
                PRAGMA user_version = 3;
                "#,
        )
        .unwrap();

    run_migrations(&connection).unwrap();

    assert!(!column_exists(&connection, "deal_metadata", "investment_thesis").unwrap());
    let version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .unwrap();
    assert_eq!(version, 4);
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
