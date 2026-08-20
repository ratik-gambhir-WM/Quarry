use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use rusqlite::Connection;
use tokio::sync::{watch, Mutex as AsyncMutex, OwnedMutexGuard, RwLock};

use crate::{core::clients::helix::HelixClient, document_jobs::DocumentJobEvent};

const DATABASE_FILE_NAME: &str = "pathfinder.sqlite3";

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<Connection>>,
    db_path: Arc<PathBuf>,
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

        let connection = Connection::open(&db_path)
            .map_err(|err| format!("failed to open sqlite database: {err}"))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|err| format!("failed to configure sqlite busy timeout: {err}"))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|err| format!("failed to configure sqlite journal mode: {err}"))?;

        run_migrations(&connection)?;

        Ok(Self {
            db: Arc::new(Mutex::new(connection)),
            db_path: Arc::new(db_path),
            document_jobs: Arc::new(RwLock::new(HashMap::new())),
            document_processing_locks: Arc::new(AsyncMutex::new(HashMap::new())),
            helix: Arc::new(HelixClient::new()?),
        })
    }

    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
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
        let db = self
            .db
            .lock()
            .map_err(|_| "sqlite connection lock was poisoned".to_string())?;

        f(&db).map_err(|err| format!("sqlite error: {err}"))
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

fn run_migrations(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
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

            CREATE TABLE IF NOT EXISTS deals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                deal_name TEXT NOT NULL,
                main_data_room_folder TEXT NOT NULL,
                deal_type TEXT NOT NULL CHECK (
                    deal_type IN (
                        'Buy-side',
                        'Sell-side',
                        'Carve-out',
                        'Add-on',
                        'Recapitalization',
                        'Growth equity'
                    )
                ),
                pe_firm TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
                target_company TEXT,
                buyer_or_platform_company TEXT,
                parent_or_seller_company TEXT,
                carve_out_business TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                CHECK (length(trim(deal_name)) > 0),
                CHECK (length(trim(main_data_room_folder)) > 0),
                CHECK (length(trim(pe_firm)) > 0),
                CHECK (
                    (deal_type = 'Sell-side' AND target_company IS NOT NULL AND length(trim(target_company)) > 0)
                    OR (
                        deal_type = 'Buy-side'
                        AND buyer_or_platform_company IS NOT NULL
                        AND length(trim(buyer_or_platform_company)) > 0
                        AND target_company IS NOT NULL
                        AND length(trim(target_company)) > 0
                    )
                    OR (
                        deal_type = 'Carve-out'
                        AND parent_or_seller_company IS NOT NULL
                        AND length(trim(parent_or_seller_company)) > 0
                        AND carve_out_business IS NOT NULL
                        AND length(trim(carve_out_business)) > 0
                    )
                    OR (
                        deal_type = 'Add-on'
                        AND buyer_or_platform_company IS NOT NULL
                        AND length(trim(buyer_or_platform_company)) > 0
                        AND target_company IS NOT NULL
                        AND length(trim(target_company)) > 0
                    )
                    OR (
                        deal_type IN ('Recapitalization', 'Growth equity')
                        AND target_company IS NOT NULL
                        AND length(trim(target_company)) > 0
                    )
                )
            );

            CREATE INDEX IF NOT EXISTS idx_deals_deal_type ON deals(deal_type);
            CREATE INDEX IF NOT EXISTS idx_deals_pe_firm ON deals(pe_firm);
            CREATE INDEX IF NOT EXISTS idx_deals_updated_at ON deals(updated_at);

            CREATE TABLE IF NOT EXISTS deal_metadata (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                deal_id INTEGER NOT NULL,
                key_questions_json TEXT NOT NULL DEFAULT '[]',
                document_count INTEGER NOT NULL DEFAULT 0 CHECK (document_count >= 0),
                data_room_size_bytes INTEGER NOT NULL DEFAULT 0 CHECK (data_room_size_bytes >= 0),
                portco_summary TEXT,
                buyer_summary TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE CASCADE
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_deal_metadata_deal_id
                ON deal_metadata(deal_id);
            CREATE INDEX IF NOT EXISTS idx_deal_metadata_updated_at
                ON deal_metadata(updated_at);

            "#,
        )
        .map_err(|err| format!("failed to initialize sqlite database: {err}"))?;

    let user_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("failed to read sqlite schema version: {err}"))?;

    if !column_exists(connection, "deals", "status")
        .map_err(|err| format!("failed to inspect deals schema: {err}"))?
    {
        connection
            .execute_batch(
                r#"
                ALTER TABLE deals
                    ADD COLUMN status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived'));
                "#,
            )
            .map_err(|err| format!("failed to add deal status column: {err}"))?;
    }

    connection
        .execute_batch("CREATE INDEX IF NOT EXISTS idx_deals_status ON deals(status);")
        .map_err(|err| format!("failed to initialize deal status index: {err}"))?;

    if column_exists(connection, "deal_metadata", "investment_thesis")
        .map_err(|err| format!("failed to inspect deal metadata schema: {err}"))?
    {
        connection
            .execute_batch("ALTER TABLE deal_metadata DROP COLUMN investment_thesis;")
            .map_err(|err| format!("failed to remove deal investment thesis column: {err}"))?;
    }

    if user_version < 4 {
        connection
            .pragma_update(None, "user_version", 4)
            .map_err(|err| format!("failed to set sqlite schema version: {err}"))?;
    }

    Ok(())
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
