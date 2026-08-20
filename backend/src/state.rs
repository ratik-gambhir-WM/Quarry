use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Weak},
};

use rusqlite::Connection;
use tokio::sync::{watch, Mutex as AsyncMutex, OwnedMutexGuard, RwLock};

use crate::{
    core::clients::{helix::HelixClient, sqlite::SqliteClient},
    document_jobs::DocumentJobEvent,
};

const DATABASE_FILE_NAME: &str = "pathfinder.sqlite3";

#[derive(Clone)]
pub struct AppState {
    db: SqliteClient,
    document_jobs: Arc<RwLock<HashMap<String, watch::Sender<DocumentJobEvent>>>>,
    document_processing_locks: Arc<AsyncMutex<HashMap<String, Weak<AsyncMutex<()>>>>>,
    helix: Arc<HelixClient>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let db_path = database_path()?;
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create database directory: {err}"))?;
        }

        let db = SqliteClient::open(&db_path)
            .map_err(|err| format!("failed to open sqlite database: {err}"))?;
        db.with_connection(|connection| run_migrations(connection))
            .map_err(|err| format!("failed to initialize sqlite database: {err}"))?;

        Ok(Self {
            db,
            document_jobs: Arc::new(RwLock::new(HashMap::new())),
            document_processing_locks: Arc::new(AsyncMutex::new(HashMap::new())),
            helix: Arc::new(HelixClient::new()?),
        })
    }

    pub fn db_path(&self) -> &Path {
        self.db.path()
    }

    pub fn sqlite(&self) -> &SqliteClient {
        &self.db
    }

    pub fn helix(&self) -> &HelixClient {
        self.helix.as_ref()
    }

    pub async fn create_document_job(&self, event: DocumentJobEvent) {
        let job_id = event.job_id.clone();
        let (sender, _) = watch::channel(event);
        self.document_jobs.write().await.insert(job_id, sender);
    }

    pub async fn subscribe_to_document_job(
        &self,
        job_id: &str,
    ) -> Option<watch::Receiver<DocumentJobEvent>> {
        self.document_jobs
            .read()
            .await
            .get(job_id)
            .map(watch::Sender::subscribe)
    }

    pub async fn update_document_job(&self, job_id: &str, event: DocumentJobEvent) {
        if let Some(sender) = self.document_jobs.read().await.get(job_id) {
            sender.send_replace(event);
        }
    }

    pub async fn remove_document_job(&self, job_id: &str) {
        self.document_jobs.write().await.remove(job_id);
    }

    pub async fn lock_document_processing(&self, document_id: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.document_processing_locks.lock().await;
            locks.retain(|_, lock| lock.strong_count() > 0);
            match locks.get(document_id).and_then(Weak::upgrade) {
                Some(lock) => lock,
                None => {
                    let lock = Arc::new(AsyncMutex::new(()));
                    locks.insert(document_id.to_string(), Arc::downgrade(&lock));
                    lock
                }
            }
        };

        lock.lock_owned().await
    }

    pub fn with_db<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, String> {
        self.db
            .with_connection(|connection| f(connection))
            .map_err(|err| format!("sqlite error: {err}"))
    }
}

fn database_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("PATHFINDER_DATABASE_PATH") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    if let Ok(path) = env::var("PATHFINDER_DATA_DIR") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path).join(DATABASE_FILE_NAME));
        }
    }

    let app_data_dir = env::current_dir()
        .map_err(|err| format!("failed to resolve current directory: {err}"))?
        .join("data");

    Ok(app_data_dir.join(DATABASE_FILE_NAME))
}

fn run_migrations(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS app_metadata (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                first_name TEXT NOT NULL,
                last_name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                api_key TEXT NOT NULL,
                role TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS reminders (
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

            "#,
    )?;

    if column_exists(connection, "deals", "id")? {
        if !column_exists(connection, "deals", "status")? {
            connection.execute_batch(
                "ALTER TABLE deals ADD COLUMN status TEXT NOT NULL DEFAULT 'active';",
            )?;
        }
        migrate_legacy_deals(connection)?;
    }

    create_deal_tables(connection)?;
    connection.pragma_update(None, "user_version", 5)?;

    Ok(())
}

fn create_deal_tables(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS deals (
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

        CREATE INDEX IF NOT EXISTS idx_deals_status ON deals(status);
        CREATE INDEX IF NOT EXISTS idx_deals_transaction_type ON deals(transaction_type);
        CREATE INDEX IF NOT EXISTS idx_deals_close_date ON deals(close_date);

        CREATE TABLE IF NOT EXISTS deal_metadata (
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

        CREATE INDEX IF NOT EXISTS idx_deal_metadata_user_id ON deal_metadata(user_id);
        "#,
    )
}

fn migrate_legacy_deals(connection: &Connection) -> rusqlite::Result<()> {
    let has_legacy_metadata = table_exists(connection, "deal_metadata")?;
    connection.execute_batch("PRAGMA foreign_keys = OFF; BEGIN IMMEDIATE;")?;
    let migration = (|| {
        connection.execute_batch("ALTER TABLE deals RENAME TO deals_legacy_v4;")?;
        if has_legacy_metadata {
            connection
                .execute_batch("ALTER TABLE deal_metadata RENAME TO deal_metadata_legacy_v4;")?;
        }
        create_deal_tables(connection)?;
        connection.execute_batch(
            r#"
            INSERT INTO deals (
                deal_id, deal_name, status, start_date, close_date,
                transaction_type, target_company, primary_buyer, deal_sponsor
            )
            SELECT
                printf('DEAL-%06d', id),
                deal_name,
                CASE WHEN lower(status) = 'archived' THEN 'Archived' ELSE 'Active' END,
                substr(created_at, 1, 10),
                substr(updated_at, 1, 10),
                deal_type,
                coalesce(target_company, carve_out_business, deal_name),
                coalesce(buyer_or_platform_company, parent_or_seller_company, pe_firm),
                pe_firm
            FROM deals_legacy_v4;
            "#,
        )?;
        if has_legacy_metadata {
            connection.execute_batch(
                r#"
                INSERT INTO deal_metadata (
                    deal_id, user_id, key_questions_json, local_path, sharepoint_link
                )
                SELECT
                    printf('DEAL-%06d', metadata.deal_id),
                    (SELECT id FROM users ORDER BY id LIMIT 1),
                    metadata.key_questions_json,
                    CASE
                        WHEN deals.main_data_room_folder NOT LIKE 'browser-upload://%'
                        THEN deals.main_data_room_folder
                        ELSE NULL
                    END,
                    NULL
                FROM deal_metadata_legacy_v4 metadata
                JOIN deals_legacy_v4 deals ON deals.id = metadata.deal_id
                WHERE EXISTS (SELECT 1 FROM users);
                DROP TABLE deal_metadata_legacy_v4;
                "#,
            )?;
        }
        connection.execute_batch("DROP TABLE deals_legacy_v4; COMMIT;")
    })();

    if migration.is_err() {
        let _ = connection.execute_batch("ROLLBACK;");
    }
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    migration
}

fn table_exists(connection: &Connection, table_name: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table_name],
        |row| row.get(0),
    )
}

fn column_exists(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> rusqlite::Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;

    for column in columns {
        if column? == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
#[path = "../tests/state_tests.rs"]
mod tests;
