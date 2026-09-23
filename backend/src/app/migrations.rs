use rusqlite::{Connection, TransactionBehavior};
use thiserror::Error;

use crate::adapters::sqlite::client::{SqliteClient, SqliteClientError};

const LATEST_SCHEMA_VERSION: i64 = 8;

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
    if version < 6 {
        recreate_version_8_schema(connection)?;
    } else if version == 6 {
        migrate_version_6_to_7(connection)?;
        migrate_version_7_to_8(connection)?;
    } else if version < LATEST_SCHEMA_VERSION {
        migrate_version_7_to_8(connection)?;
    }
    Ok(())
}

fn recreate_version_8_schema(connection: &mut Connection) -> Result<(), MigrationError> {
    connection.pragma_update(None, "foreign_keys", "OFF")?;
    let migration = (|| -> Result<(), MigrationError> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            r#"
            DROP TABLE IF EXISTS assistant_messages;
            DROP TABLE IF EXISTS assistant_threads;
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

            CREATE TABLE assistant_threads (
                thread_id TEXT PRIMARY KEY NOT NULL,
                user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'regular'
                    CHECK (status IN ('regular', 'archived')),
                metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                last_message_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                CHECK (length(trim(thread_id)) > 0),
                CHECK (length(trim(title)) > 0)
            );

            CREATE INDEX idx_assistant_threads_user_status_recent
                ON assistant_threads(user_id, status, last_message_at DESC, thread_id);

            CREATE TABLE assistant_messages (
                message_id TEXT PRIMARY KEY NOT NULL,
                thread_id TEXT NOT NULL REFERENCES assistant_threads(thread_id) ON DELETE CASCADE,
                parent_message_id TEXT REFERENCES assistant_messages(message_id),
                sequence INTEGER NOT NULL CHECK (sequence >= 0),
                role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
                content TEXT NOT NULL,
                status TEXT NOT NULL
                    CHECK (status IN ('streaming', 'completed', 'failed', 'cancelled')),
                model TEXT,
                request_id TEXT,
                error_code TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                completed_at TEXT,
                UNIQUE (thread_id, sequence),
                UNIQUE (thread_id, request_id),
                CHECK (length(trim(message_id)) > 0),
                CHECK (request_id IS NULL OR length(trim(request_id)) > 0)
            );

            CREATE INDEX idx_assistant_messages_thread_sequence
                ON assistant_messages(thread_id, sequence);
            CREATE INDEX idx_assistant_messages_thread_parent
                ON assistant_messages(thread_id, parent_message_id);

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
                sow_link TEXT,
                fact_sheet_link TEXT,
                rl_link TEXT,
                FOREIGN KEY (deal_id) REFERENCES deals(deal_id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE RESTRICT,
                CHECK (local_path IS NULL OR length(trim(local_path)) > 0),
                CHECK (sharepoint_link IS NULL OR length(trim(sharepoint_link)) > 0),
                CHECK (sow_link IS NULL OR length(trim(sow_link)) > 0),
                CHECK (fact_sheet_link IS NULL OR length(trim(fact_sheet_link)) > 0),
                CHECK (rl_link IS NULL OR length(trim(rl_link)) > 0),
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

fn migrate_version_6_to_7(connection: &mut Connection) -> Result<(), MigrationError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        r#"
        ALTER TABLE deal_metadata
            ADD COLUMN sow_link TEXT
            CHECK (sow_link IS NULL OR length(trim(sow_link)) > 0);
        ALTER TABLE deal_metadata
            ADD COLUMN fact_sheet_link TEXT
            CHECK (fact_sheet_link IS NULL OR length(trim(fact_sheet_link)) > 0);
        ALTER TABLE deal_metadata
            ADD COLUMN rl_link TEXT
            CHECK (rl_link IS NULL OR length(trim(rl_link)) > 0);
        "#,
    )?;
    transaction.pragma_update(None, "user_version", 7)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_version_7_to_8(connection: &mut Connection) -> Result<(), MigrationError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS assistant_threads (
            thread_id TEXT PRIMARY KEY NOT NULL,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'regular'
                CHECK (status IN ('regular', 'archived')),
            metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_message_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            CHECK (length(trim(thread_id)) > 0),
            CHECK (length(trim(title)) > 0)
        );
        CREATE INDEX IF NOT EXISTS idx_assistant_threads_user_status_recent
            ON assistant_threads(user_id, status, last_message_at DESC, thread_id);

        CREATE TABLE IF NOT EXISTS assistant_messages (
            message_id TEXT PRIMARY KEY NOT NULL,
            thread_id TEXT NOT NULL REFERENCES assistant_threads(thread_id) ON DELETE CASCADE,
            parent_message_id TEXT REFERENCES assistant_messages(message_id),
            sequence INTEGER NOT NULL CHECK (sequence >= 0),
            role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
            content TEXT NOT NULL,
            status TEXT NOT NULL
                CHECK (status IN ('streaming', 'completed', 'failed', 'cancelled')),
            model TEXT,
            request_id TEXT,
            error_code TEXT,
            metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT,
            UNIQUE (thread_id, sequence),
            UNIQUE (thread_id, request_id),
            CHECK (length(trim(message_id)) > 0),
            CHECK (request_id IS NULL OR length(trim(request_id)) > 0)
        );
        CREATE INDEX IF NOT EXISTS idx_assistant_messages_thread_sequence
            ON assistant_messages(thread_id, sequence);
        CREATE INDEX IF NOT EXISTS idx_assistant_messages_thread_parent
            ON assistant_messages(thread_id, parent_message_id);
        "#,
    )?;
    transaction.pragma_update(None, "user_version", LATEST_SCHEMA_VERSION)?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/app/migrations_tests.rs"]
mod tests;
