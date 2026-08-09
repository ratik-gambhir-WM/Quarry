use std::{fs, path::PathBuf, sync::Mutex, time::Duration};

use rusqlite::{Connection, Params, Row};
use tauri::{AppHandle, Manager};

const DATABASE_FILE_NAME: &str = "quarry.sqlite3";

pub struct SqliteClient {
    pub db: Mutex<Connection>,
    pub db_path: PathBuf,
}

impl SqliteClient {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let db_path = database_path(app)?;
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
            db: Mutex::new(connection),
            db_path,
        })
    }

    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self, String> {
        let connection = Connection::open_in_memory()
            .map_err(|err| format!("failed to open in-memory sqlite database: {err}"))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|err| format!("failed to configure sqlite busy timeout: {err}"))?;
        run_migrations(&connection)?;

        Ok(Self {
            db: Mutex::new(connection),
            db_path: PathBuf::from(":memory:"),
        })
    }

    pub fn execute<P>(&self, sql: &str, params: P) -> Result<usize, String>
    where
        P: Params,
    {
        self.with_connection(|db| db.execute(sql, params))
    }

    pub fn query_rows<P, T>(
        &self,
        sql: &str,
        params: P,
        mut map_row: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, String>
    where
        P: Params,
    {
        self.with_connection(|db| {
            let mut statement = db.prepare(sql)?;
            let rows = statement.query_map(params, |row| map_row(row))?;

            rows.collect::<rusqlite::Result<Vec<T>>>()
        })
    }

    pub fn with_connection<T>(
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

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("QUARRY_DATABASE_PATH") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("failed to resolve app data directory: {err}"))?;

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
                email TEXT NOT NULL COLLATE NOCASE UNIQUE,
                api_key TEXT NOT NULL,
                role TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_users_email_nocase
                ON users(email COLLATE NOCASE);

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
                legacy_investment_thesis TEXT,
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
        .map_err(|err| format!("failed to inspect legacy deal metadata: {err}"))?
        && !column_exists(connection, "deal_metadata", "legacy_investment_thesis")
            .map_err(|err| format!("failed to inspect canonical deal metadata: {err}"))?
    {
        connection
            .execute_batch(
                r#"
                ALTER TABLE deal_metadata ADD COLUMN legacy_investment_thesis TEXT;
                UPDATE deal_metadata
                SET legacy_investment_thesis = investment_thesis
                WHERE length(trim(investment_thesis)) > 0;
                "#,
            )
            .map_err(|err| format!("failed to preserve legacy investment thesis data: {err}"))?;
    }

    if user_version < 5 {
        connection
            .pragma_update(None, "user_version", 5)
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
#[path = "../../../tests/core/clients/sqlite_tests.rs"]
mod tests;
