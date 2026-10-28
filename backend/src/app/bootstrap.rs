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
        assistant::{
            self,
            chat::{repository::AssistantChatRepository, service::AssistantChatService},
        },
        companies::{self, repository::CompanyRepository, service::CompanyService},
        data_rooms::{self, service::DataRoomService},
        deals::{
            self, repository::DealRepository, service::DealService, DataRoomSourceReader,
            DealContextReader,
        },
        documents::{
            graph::repository::{DocumentGraphReader, DocumentGraphSearch, DocumentGraphWriter},
            ingestion::{self, job_service::DocumentJobService, service::DocumentIngestionService},
            search::{self, service::DocumentSearchService},
            store::sqlite::DocumentStore,
            viewing::{self, service::StoredDocumentService},
        },
        identity::{
            self, access::ResourceAccess, repository::IdentityRepository, service::IdentityService,
        },
        research::{
            self,
            service::{ResearchService, WmAiClients},
        },
        summaries::{self, service::SummaryService},
        system,
        templates::{self, service::TemplateService},
        users::{repository::UserRepository, UserDirectory},
    },
    shared::prompts::{PromptCatalog, PromptCatalogError},
};

const DEFAULT_DEAL_EXTRACTION_MODEL: &str = "gpt-5.6-luna";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_IMAGE_DESCRIPTION_MODEL: &str = "gpt-5.5";

pub struct Application {
    pub router: Router,
    pub bind_address: std::net::SocketAddr,
    pub templates: Option<Arc<templates::slide_templates::SlideTemplates>>,
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("failed to load prompt catalog: {0}")]
    PromptCatalog(PromptCatalogError),
    #[error("failed to create SQLite directory: {0}")]
    SqliteDirectory(std::io::Error),
    #[error("failed to open SQLite database: {0}")]
    SqliteOpen(SqliteClientError),
    #[error("SQLite initialization worker failed: {0}")]
    SqliteWorker(tokio::task::JoinError),
    #[error("failed to migrate SQLite database: {0}")]
    Migration(MigrationError),
    #[error("failed to construct Slide Templates Studio: {0}")]
    SlideTemplates(String),
    #[error("failed to construct shared HTTP client: {0}")]
    HttpClient(reqwest::Error),
    #[error("failed to construct Helix client: {0}")]
    HelixClient(String),
    #[error("failed to initialize Helix document indexes: {0}")]
    HelixIndexes(String),
}

pub async fn bootstrap(config: AppConfig) -> Result<Application, BootstrapError> {
    let prompts = Arc::new(
        PromptCatalog::load(&config.prompts.directory).map_err(BootstrapError::PromptCatalog)?,
    );
    let path = config.sqlite.path.clone();
    let sqlite = tokio::task::spawn_blocking(move || {
        let sqlite = open_sqlite(&path)?;
        migrations::migrate(&sqlite).map_err(BootstrapError::Migration)?;
        Ok::<_, BootstrapError>(sqlite)
    })
    .await
    .map_err(BootstrapError::SqliteWorker)??;

    let helix =
        Arc::new(HelixClient::from_config(&config.helix).map_err(BootstrapError::HelixClient)?);
    DocumentGraphWriter::new(helix.clone())
        .initialize(config.helix.vector_dimension)
        .await
        .map_err(|error| BootstrapError::HelixIndexes(error.to_string()))?;

    let http = reqwest::Client::builder()
        .build()
        .map_err(BootstrapError::HttpClient)?;
    let (templates, embedded) =
        build_templates(&config, sqlite.clone(), helix.clone(), http.clone()).await?;
    let api = assemble_api_with_templates(
        &config,
        sqlite,
        helix,
        http,
        prompts,
        templates,
        embedded.clone(),
    );
    let bind_address = config.http.bind_address;
    let router = create_router(api, &config.http);
    Ok(Application {
        router,
        bind_address,
        templates: embedded,
    })
}

#[allow(clippy::too_many_arguments)]
fn assemble_api_with_templates(
    config: &AppConfig,
    sqlite: SqliteClient,
    document_graph: Arc<HelixClient>,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
    templates: Arc<TemplateService>,
    embedded: Option<Arc<templates::slide_templates::SlideTemplates>>,
) -> Router {
    let identity = Arc::new(IdentityService::new(IdentityRepository::new(
        sqlite.clone(),
        UserRepository::new(sqlite.clone()),
    )));
    let access = ResourceAccess::new(sqlite.clone());
    let users_repository = UserRepository::new(sqlite.clone());
    let deals_repository = DealRepository::new(sqlite.clone());
    let document_files = DocumentStore::new(sqlite.clone());
    let document_graph_reader = DocumentGraphReader::new(document_graph.clone());
    let document_graph_writer = DocumentGraphWriter::new(document_graph.clone());
    let document_graph_search = DocumentGraphSearch::new(document_graph);
    let openai = config.openai.as_ref().map(|openai_config| {
        Arc::new(OpenAiClient::from_config(
            http.clone(),
            openai_config,
            prompts.clone(),
        ))
    });
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
    let user_directory = UserDirectory::new(users_repository.clone());
    let companies = Arc::new(CompanyService::new(
        CompanyRepository::new(sqlite.clone()),
        user_directory.clone(),
    ));
    let deals = Arc::new(DealService::new(
        user_directory.clone(),
        deals_repository.clone(),
        openai.clone(),
        config
            .openai
            .as_ref()
            .map(|config| config.deal_extraction_model.clone())
            .unwrap_or_else(|| DEFAULT_DEAL_EXTRACTION_MODEL.to_string()),
        prompts.clone(),
    ));
    let deal_context = DealContextReader::new(deals_repository.clone());
    let data_rooms = Arc::new(DataRoomService::new(
        DataRoomSourceReader::new(deals_repository),
        config.data_room.clone(),
        office.clone(),
    ));
    let summaries = Arc::new(SummaryService::new(
        openai.clone(),
        config
            .openai
            .as_ref()
            .map(|c| c.document_summary_model.clone())
            .unwrap_or_else(|| "gpt-5.5".into()),
        prompts.clone(),
    ));
    let document_ingestion = Arc::new(DocumentIngestionService::new(
        document_files.clone(),
        document_graph_reader,
        document_graph_writer,
        openai.clone(),
        config
            .openai
            .as_ref()
            .map(|config| config.embedding_model.clone())
            .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.to_string()),
        config
            .openai
            .as_ref()
            .map(|config| config.image_description_model.clone())
            .unwrap_or_else(|| DEFAULT_IMAGE_DESCRIPTION_MODEL.to_string()),
        config.documents.max_concurrent_documents,
    ));
    let document_jobs = Arc::new(DocumentJobService::new(
        document_ingestion.clone(),
        config.documents.completed_job_retention,
    ));
    let document_search =
        Arc::new(DocumentSearchService::new(document_graph_search).with_access(access.clone()));
    let stored_documents = Arc::new(StoredDocumentService::new(document_files, office));
    let research = Arc::new(ResearchService::new(wm_clients));
    let assistant_chat = Arc::new(
        AssistantChatService::with_persistence(
            openai,
            config
                .openai
                .as_ref()
                .map(|config| config.chat_model.clone())
                .unwrap_or_else(|| "gpt-5.5".to_string()),
            AssistantChatRepository::new(sqlite.clone()),
            user_directory,
            prompts,
        )
        .with_deal_context(deal_context),
    );

    let native_routes = embedded
        .map(templates::slide_templates::route::routes)
        .unwrap_or_default();
    let protected = Router::new()
        .merge(identity::route::protected(identity.clone()))
        .merge(native_routes)
        .merge(deals::route::routes(deals))
        .merge(companies::route::routes(companies))
        .merge(data_rooms::route::routes(data_rooms))
        .merge(ingestion::route::routes(document_ingestion, document_jobs))
        .merge(viewing::route::routes(stored_documents))
        .merge(search::route::routes(document_search))
        .merge(summaries::route::routes(summaries))
        .merge(research::route::routes(research))
        .merge(templates::route::routes(templates))
        .merge(assistant::chat::route::routes(assistant_chat))
        .route_layer(axum::middleware::from_fn_with_state(
            crate::app::http::auth::AuthBoundary {
                identity: identity.clone(),
                access,
            },
            crate::app::http::auth::authenticate,
        ));
    Router::new()
        .merge(protected)
        .merge(system::route::routes())
        .merge(identity::route::public(identity))
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

#[cfg(test)]
#[path = "../../tests/unit/app/bootstrap_tests.rs"]
mod tests;

async fn build_templates(
    config: &AppConfig,
    sqlite: SqliteClient,
    helix: Arc<HelixClient>,
    http: reqwest::Client,
) -> Result<
    (
        Arc<TemplateService>,
        Option<Arc<templates::slide_templates::SlideTemplates>>,
    ),
    BootstrapError,
> {
    use crate::adapters::slide_templates::preview as native;
    use crate::app::config::TemplateBackend;
    use templates::slide_templates::*;
    let settings = &config.slide_templates;
    if settings.backend == TemplateBackend::DiligenceStudio {
        let legacy = config
            .diligence_studio
            .as_ref()
            .map(|c| Arc::new(DiligenceStudioClient::new(http, c.base_url.clone())));
        return Ok((Arc::new(TemplateService::new(legacy)), None));
    }
    let repository = tokio::task::spawn_blocking(move || seed_templates(sqlite))
        .await
        .map_err(|_| BootstrapError::SlideTemplates("template store worker failed".into()))?
        .map_err(|_| {
            BootstrapError::SlideTemplates("shared SQLite template catalog seeding failed".into())
        })?;
    let native_preview: Arc<dyn native::TemplatePreviewGenerator> = match &settings.chromium_path {
        Some(path) => Arc::new(native::HeadlessTemplatePreviewGenerator::new(
            path.clone(),
            settings.render_url.clone(),
            settings.preview_max_bytes,
            10_000,
            settings.preview_render_size,
            settings.preview_timeout,
        )),
        None => Arc::new(native::DisabledTemplatePreviewGenerator),
    };
    let preview = Arc::new(NativePreview(native_preview));
    let provider_http = reqwest::Client::builder()
        .retry(reqwest::retry::never())
        .build()
        .map_err(BootstrapError::HttpClient)?;
    let classification = settings.classification.as_ref().map(|c| {
        Arc::new(SlideClassificationService::new(
            SlideClassificationServiceOptions {
                classifier: Arc::new(OpenAISlideClassifier::new(
                    provider_http.clone(),
                    OpenAISlideClassifierOptions {
                        api_key: c.key.expose().into(),
                        base_url: None,
                        model: c.model.clone(),
                        timeout: c.timeout,
                        max_output_tokens: c.max_output_tokens,
                    },
                )),
                classification_model: c.model.clone(),
                embedder: Arc::new(OpenAISlideEmbedder::new(
                    provider_http.clone(),
                    OpenAISlideEmbedderOptions {
                        api_key: c.key.expose().into(),
                        base_url: None,
                        model: c.embedding_model.clone(),
                        timeout: c.timeout,
                        dimensions: Some(c.dimensions.get()),
                        max_input_bytes: c.max_embedding_text_bytes,
                    },
                )),
                embedding_dimensions: Some(c.dimensions.get()),
                embedding_max_text_bytes: c.max_embedding_text_bytes,
                embedding_model: c.embedding_model.clone(),
                input_limits: SlideClassificationInputLimits::default(),
                repository: repository.clone(),
            },
        ))
    });
    let dimensions = settings.embedding_dimensions;
    let graph = Arc::new(graph::TemplateGraph::new(helix, dimensions));
    graph.initialize().await.map_err(|_| {
        BootstrapError::SlideTemplates("Helix template indexes failed to initialize".into())
    })?;
    let embedded = Arc::new(SlideTemplates::new(
        repository,
        preview,
        classification,
        Some(graph),
        settings.allow_embedding_inspection,
    ));
    embedded.reconcile().await.map_err(|_| {
        BootstrapError::SlideTemplates(
            "Helix template reconciliation failed; canonical SQLite records were preserved".into(),
        )
    })?;
    Ok((
        Arc::new(TemplateService::embedded(embedded.clone())),
        Some(embedded),
    ))
}
/// The repository shares the migrated product connection; it never opens or initializes a store.
fn seed_templates(
    sqlite: SqliteClient,
) -> anyhow::Result<Arc<dyn templates::slide_templates::TemplateRepository>> {
    use templates::slide_templates::{catalog, SqliteTemplateRepository, APP_ID};
    let repository = Arc::new(SqliteTemplateRepository::new(sqlite));
    repository.ensure_app(APP_ID, None)?;
    catalog::seed(repository.as_ref())?;
    Ok(repository)
}

#[cfg(test)]
pub(crate) fn assemble_api(
    config: &AppConfig,
    sqlite: SqliteClient,
    document_graph: Arc<HelixClient>,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
) -> Router {
    let legacy = config
        .diligence_studio
        .as_ref()
        .map(|c| Arc::new(DiligenceStudioClient::new(http.clone(), c.base_url.clone())));
    assemble_api_with_templates(
        config,
        sqlite,
        document_graph,
        http,
        prompts,
        Arc::new(TemplateService::new(legacy)),
        None,
    )
}
