use std::{fs, path::Path, sync::Arc};

use axum::Router;
use thiserror::Error;

use crate::{
    adapters::{
        diligence_studio::client::DiligenceStudioClient,
        helix::client::HelixClient,
        office::converter::OfficeConverter,
        openai::client::OpenAiClient,
        sqlite::client::{SqliteClient, SqliteClientError},
        wm_ai::client::{FileUploadServiceClient, GraphRagClient, IndexServiceClient},
    },
    app::{
        config::AppConfig,
        http::create_router,
        migrations::{self, MigrationError},
    },
    domains::{
        data_rooms::{self, service::DataRoomService},
        deals::{self, repository::DealRepository, service::DealService, DataRoomSourceReader},
        dev_support,
        documents::{
            index::repository::{DocumentIndexReader, DocumentIndexWriter, DocumentSearchIndex},
            ingestion::{self, job_service::DocumentJobService, service::DocumentIngestionService},
            search::{self, service::DocumentSearchService},
            store::sqlite::DocumentStore,
            viewing::{self, service::StoredDocumentService},
        },
        research::{
            self,
            service::{ResearchService, WmAiClients},
        },
        summaries::{self, service::SummaryService},
        system::{self, service::DatabaseService},
        templates::{self, service::TemplateService},
        users::{self, repository::UserRepository, service::UserService, UserDirectory},
    },
};

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

pub async fn bootstrap(config: AppConfig) -> Result<Application, BootstrapError> {
    let sqlite = open_sqlite(&config.sqlite.path)?;
    migrations::migrate(&sqlite).map_err(BootstrapError::Migration)?;

    let helix =
        Arc::new(HelixClient::from_config(&config.helix).map_err(BootstrapError::HelixClient)?);
    DocumentIndexWriter::new(helix.clone())
        .initialize()
        .await
        .map_err(|error| BootstrapError::HelixIndexes(error.to_string()))?;

    let http = reqwest::Client::builder()
        .build()
        .map_err(BootstrapError::HttpClient)?;
    let api = assemble_api(&config, sqlite, helix, http);
    let bind_address = config.http.bind_address;
    let router = create_router(api, &config.http);
    Ok(Application {
        router,
        bind_address,
    })
}

pub(crate) fn assemble_api(
    config: &AppConfig,
    sqlite: SqliteClient,
    document_index: Arc<HelixClient>,
    http: reqwest::Client,
) -> Router {
    let users_repository = UserRepository::new(sqlite.clone());
    let deals_repository = DealRepository::new(sqlite.clone());
    let document_files = DocumentStore::new(sqlite.clone());
    let document_index_reader = DocumentIndexReader::new(document_index.clone());
    let document_index_writer = DocumentIndexWriter::new(document_index.clone());
    let document_search_index = DocumentSearchIndex::new(document_index);
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
            http.clone(),
            wm.graph_rag_url.clone(),
            wm.graph_rag_api_key.expose().to_string(),
            wm.graph_rag_application_name.clone(),
        ),
    });
    let diligence_studio = config
        .diligence_studio
        .as_ref()
        .map(|studio| Arc::new(DiligenceStudioClient::new(http, studio.base_url.clone())));

    let user_directory = UserDirectory::new(users_repository.clone());
    let users = Arc::new(UserService::new(users_repository));
    let deals = Arc::new(DealService::new(
        user_directory,
        deals_repository.clone(),
        openai.clone(),
        config
            .openai
            .as_ref()
            .map(|config| config.deal_extraction_model.clone())
            .unwrap_or_else(|| DEFAULT_DEAL_EXTRACTION_MODEL.to_string()),
    ));
    let data_rooms = Arc::new(DataRoomService::new(
        DataRoomSourceReader::new(deals_repository),
        config.data_room.clone(),
        office.clone(),
    ));
    let database = Arc::new(DatabaseService::new(sqlite.path().to_path_buf()));
    let document_ingestion = Arc::new(DocumentIngestionService::new(
        document_files.clone(),
        document_index_reader,
        document_index_writer,
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
    let document_search = Arc::new(DocumentSearchService::new(document_search_index));
    let document_summaries = Arc::new(SummaryService::new(
        openai,
        config
            .openai
            .as_ref()
            .map(|config| config.document_summary_model.clone())
            .unwrap_or_else(|| DEFAULT_DOCUMENT_SUMMARY_MODEL.to_string()),
    ));
    let stored_documents = Arc::new(StoredDocumentService::new(document_files, office));
    let research = Arc::new(ResearchService::new(wm_clients));
    let templates = Arc::new(TemplateService::new(diligence_studio));

    Router::new()
        .merge(system::route::routes(database))
        .merge(dev_support::route::routes())
        .merge(users::route::routes(users))
        .merge(deals::route::routes(deals))
        .merge(data_rooms::route::routes(data_rooms))
        .merge(ingestion::route::routes(document_ingestion, document_jobs))
        .merge(viewing::route::routes(stored_documents))
        .merge(search::route::routes(document_search))
        .merge(summaries::route::routes(document_summaries))
        .merge(research::route::routes(research))
        .merge(templates::route::routes(templates))
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
