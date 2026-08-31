use std::{fs, path::Path, sync::Arc};

use axum::Router;
use rusqlite::{Connection, TransactionBehavior};
use thiserror::Error;

use crate::{
    config::AppConfig,
    core::clients::{
        helix::HelixClient,
        office_converter::OfficeConverter,
        openai::OpenAiClient,
        sqlite::{SqliteClient, SqliteClientError},
        wm_ai_services::{FileUploadServiceClient, GraphRagClient, IndexServiceClient},
    },
    create_router,
    repository::{
        deal_repository::DealRepository,
        document_repository::{DocumentFileRepository, DocumentIndexRepository},
        user_repository::UserRepository,
    },
    services::{
        data_room_service::DataRoomService,
        database_service::DatabaseService,
        deal_service::DealService,
        document_ingestion_service::DocumentIngestionService,
        document_job_service::DocumentJobService,
        document_search_service::DocumentSearchService,
        document_service::DocumentSummaryService,
        research_service::{ResearchService, WmAiClients},
        stored_document_service::StoredDocumentService,
        user_service::UserService,
    },
    state::AppState,
};

const LATEST_SCHEMA_VERSION: i64 = 6;
const DEFAULT_DEAL_EXTRACTION_MODEL: &str = "gpt-5.6-luna";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_DOCUMENT_SUMMARY_MODEL: &str = "gpt-5.5";

pub struct Application {
    pub router: Router,
    pub bind_address: std::net::SocketAddr,
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("failed to create SQLite directory: {0}")]
    SqliteDirectory(std::io::Error),
    #[error("failed to open SQLite database: {0}")]
    SqliteOpen(SqliteClientError),
    #[error("failed to migrate SQLite database: {0}")]
    Migration(MigrationError),
    #[error("failed to construct shared HTTP client: {0}")]
    HttpClient(reqwest::Error),
    #[error("failed to construct Helix client: {0}")]
    HelixClient(String),
    #[error("failed to initialize Helix document indexes: {0}")]
    HelixIndexes(String),
}

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Client(#[from] SqliteClientError),
    #[error("database schema version {found} is newer than supported version {supported}")]
    UnsupportedSchemaVersion { found: i64, supported: i64 },
}

pub async fn bootstrap(config: AppConfig) -> Result<Application, BootstrapError> {
    let sqlite = open_sqlite(&config.sqlite.path)?;
    migrate(&sqlite)?;

    let helix =
        Arc::new(HelixClient::from_config(&config.helix).map_err(BootstrapError::HelixClient)?);
    let index_repository = DocumentIndexRepository::new(helix.clone());
    index_repository
        .initialize()
        .await
        .map_err(|error| BootstrapError::HelixIndexes(error.to_string()))?;

    let http = reqwest::Client::builder()
        .build()
        .map_err(BootstrapError::HttpClient)?;
    let state = assemble_state(&config, sqlite, index_repository, http);
    let bind_address = config.http.bind_address;
    let router = create_router(state, &config.http);
    Ok(Application {
        router,
        bind_address,
    })
}

fn assemble_state(
    config: &AppConfig,
    sqlite: SqliteClient,
    document_index: DocumentIndexRepository,
    http: reqwest::Client,
) -> AppState {
    let users_repository = UserRepository::new(sqlite.clone());
    let deals_repository = DealRepository::new(sqlite.clone());
    let document_files = DocumentFileRepository::new(sqlite.clone());
    let openai = config
        .openai
        .as_ref()
        .map(|openai_config| Arc::new(OpenAiClient::from_config(http.clone(), openai_config)));
    let office = OfficeConverter::new(config.data_room.office_executable.clone());
    let wm_clients = config.wm_ai.as_ref().map(|wm| WmAiClients {
        files: FileUploadServiceClient::new(
            http.clone(),
            wm.file_upload_url.clone(),
            wm.file_upload_api_key.expose().to_string(),
        ),
        indexes: IndexServiceClient::new(
            http.clone(),
            wm.index_url.clone(),
            wm.index_api_key.expose().to_string(),
        ),
        graph_rag: GraphRagClient::new(
            http,
            wm.graph_rag_url.clone(),
            wm.graph_rag_api_key.expose().to_string(),
            wm.graph_rag_application_name.clone(),
        ),
    });

    let users = Arc::new(UserService::new(users_repository.clone()));
    let deals = Arc::new(DealService::new(
        users_repository,
        deals_repository.clone(),
        openai.clone(),
        config
            .openai
            .as_ref()
            .map(|config| config.deal_extraction_model.clone())
            .unwrap_or_else(|| DEFAULT_DEAL_EXTRACTION_MODEL.to_string()),
    ));
    let data_rooms = Arc::new(DataRoomService::new(
        deals_repository,
        config.data_room.clone(),
        office.clone(),
    ));
    let database = Arc::new(DatabaseService::new(sqlite.path().to_path_buf()));
    let document_ingestion = Arc::new(DocumentIngestionService::new(
        document_files.clone(),
        document_index.clone(),
        openai.clone(),
        config
            .openai
            .as_ref()
            .map(|config| config.embedding_model.clone())
            .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.to_string()),
        config.documents.max_concurrent_documents,
    ));
    let document_jobs = Arc::new(DocumentJobService::new(
        document_ingestion.clone(),
        config.documents.completed_job_retention,
    ));
    let document_search = Arc::new(DocumentSearchService::new(document_index));
    let document_summaries = Arc::new(DocumentSummaryService::new(
        openai,
        config
            .openai
            .as_ref()
            .map(|config| config.document_summary_model.clone())
            .unwrap_or_else(|| DEFAULT_DOCUMENT_SUMMARY_MODEL.to_string()),
    ));
    let stored_documents = Arc::new(StoredDocumentService::new(document_files, office));
    let research = Arc::new(ResearchService::new(wm_clients));

    AppState::new(
        users,
        deals,
        data_rooms,
        database,
        document_ingestion,
        document_jobs,
        document_search,
        document_summaries,
        stored_documents,
        research,
    )
}

fn open_sqlite(path: &Path) -> Result<SqliteClient, BootstrapError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(BootstrapError::SqliteDirectory)?;
    }
    SqliteClient::open(path).map_err(BootstrapError::SqliteOpen)
}

pub(crate) fn migrate(sqlite: &SqliteClient) -> Result<(), BootstrapError> {
    sqlite
        .with_connection_result(run_migrations)
        .map_err(BootstrapError::Migration)
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
pub(crate) struct TestApplication {
    pub state: AppState,
    pub sqlite: SqliteClient,
}

#[cfg(test)]
pub(crate) fn test_application() -> Result<TestApplication, BootstrapError> {
    let config = AppConfig::default();
    let sqlite = SqliteClient::open_in_memory().map_err(BootstrapError::SqliteOpen)?;
    migrate(&sqlite)?;
    let helix =
        Arc::new(HelixClient::from_config(&config.helix).map_err(BootstrapError::HelixClient)?);
    let index = DocumentIndexRepository::new(helix);
    let state = assemble_state(&config, sqlite.clone(), index, reqwest::Client::new());
    Ok(TestApplication { state, sqlite })
}

#[cfg(test)]
#[path = "../tests/state_tests.rs"]
mod tests;
