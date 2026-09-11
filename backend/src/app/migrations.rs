use rusqlite::{Connection, TransactionBehavior};
use thiserror::Error;

use crate::adapters::sqlite::client::{SqliteClient, SqliteClientError};

const LATEST_SCHEMA_VERSION: i64 = 6;

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Client(#[from] SqliteClientError),
    #[error("database schema version {found} is newer than supported version {supported}")]
    UnsupportedSchemaVersion { found: i64, supported: i64 },
}

pub(crate) fn migrate(sqlite: &SqliteClient) -> Result<(), MigrationError> {
    sqlite.with_connection_result(run_migrations)
}

pub(crate) fn run_migrations(connection: &mut Connection) -> Result<(), MigrationError> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > LATEST_SCHEMA_VERSION {
        return Err(MigrationError::UnsupportedSchemaVersion {
            found: version,
            supported: LATEST_SCHEMA_VERSION,
        });
    }
    if version < LATEST_SCHEMA_VERSION {
        recreate_version_6_schema(connection)?;
    }
    Ok(())
}

fn recreate_version_6_schema(connection: &mut Connection) -> Result<(), MigrationError> {
    connection.pragma_update(None, "foreign_keys", "OFF")?;
    let migration = (|| -> Result<(), MigrationError> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            r#"
            DROP TABLE IF EXISTS quarry_file_blobs;
            DROP TABLE IF EXISTS quarry_file_versions;
            DROP TABLE IF EXISTS quarry_files;
            DROP TABLE IF EXISTS deal_metadata;
            DROP TABLE IF EXISTS deals;
            DROP TABLE IF EXISTS reminders;
            DROP TABLE IF EXISTS users;
            DROP TABLE IF EXISTS app_metadata;

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
                user_id INTEGER NOT NULL,
                deal_name TEXT NOT NULL,
                status TEXT NOT NULL,
                start_date TEXT NOT NULL,
                close_date TEXT NOT NULL,
                transaction_type TEXT NOT NULL,
                target_company TEXT NOT NULL,
                primary_buyer TEXT NOT NULL,
                deal_sponsor TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE RESTRICT,
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

            CREATE INDEX idx_deals_user_id ON deals(user_id);
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

            CREATE TABLE quarry_files (
                file_id       TEXT PRIMARY KEY NOT NULL,
                deal_id       TEXT NOT NULL REFERENCES deals(deal_id) ON DELETE CASCADE,
                workspace_id  TEXT NOT NULL,
                display_name  TEXT NOT NULL,
                source_uri    TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL,
                deleted_at    TEXT,
                CHECK (length(trim(file_id)) > 0),
                CHECK (length(trim(deal_id)) > 0),
                CHECK (length(trim(workspace_id)) > 0),
                CHECK (length(trim(display_name)) > 0),
                CHECK (source_uri IS NULL OR length(trim(source_uri)) > 0)
            );

            CREATE TABLE quarry_file_versions (
                version_id        TEXT PRIMARY KEY NOT NULL,
                file_id           TEXT NOT NULL REFERENCES quarry_files(file_id) ON DELETE CASCADE,
                version_number    INTEGER NOT NULL CHECK (version_number > 0),
                original_filename TEXT NOT NULL,
                mime_type         TEXT NOT NULL,
                content_sha256    TEXT NOT NULL,
                byte_size         INTEGER NOT NULL CHECK (byte_size >= 0),
                is_current        INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0, 1)),
                created_at        TEXT NOT NULL,
                UNIQUE (file_id, version_number),
                UNIQUE (file_id, content_sha256),
                CHECK (length(trim(version_id)) > 0),
                CHECK (length(trim(original_filename)) > 0),
                CHECK (length(content_sha256) = 64)
            );

            CREATE UNIQUE INDEX uq_quarry_file_versions_current
                ON quarry_file_versions(file_id)
                WHERE is_current = 1;
            CREATE INDEX idx_quarry_files_deal
                ON quarry_files(deal_id, deleted_at);
            CREATE INDEX idx_quarry_files_workspace_deal
                ON quarry_files(workspace_id, deal_id, deleted_at);
            CREATE INDEX idx_quarry_file_versions_file
                ON quarry_file_versions(file_id, version_number DESC);
            CREATE INDEX idx_quarry_file_versions_hash
                ON quarry_file_versions(content_sha256);

            CREATE TABLE quarry_file_blobs (
                version_id  TEXT PRIMARY KEY NOT NULL
                    REFERENCES quarry_file_versions(version_id) ON DELETE CASCADE,
                file_bytes  BLOB NOT NULL
            );
            "#,
        )?;
        transaction.pragma_update(None, "user_version", LATEST_SCHEMA_VERSION)?;
        transaction.commit()?;
        Ok(())
    })();
    let restore_foreign_keys = connection.pragma_update(None, "foreign_keys", "ON");
    migration?;
    restore_foreign_keys?;
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/app/migrations_tests.rs"]
mod tests;
