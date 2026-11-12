use std::sync::Arc;

use axum::Router;
use thiserror::Error;

use crate::{
    adapters::{
        helix::client::HelixClient,
        office::converter::OfficeConverter,
        openai::client::OpenAiClient,
        postgres::client::{PostgresClient, PostgresError},
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
            store::postgres::DocumentStore,
            viewing::{self, service::StoredDocumentService},
        },
        identity::{
            self, access::ResourceAccess, repository::IdentityRepository, service::IdentityService,
        },
        notebooks::{self, NotebookPipeline, NotebookRepository, NotebookService},
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
    pub templates: Arc<templates::slide_templates::SlideTemplates>,
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("failed to load prompt catalog: {0}")]
    PromptCatalog(PromptCatalogError),
    #[error("failed to connect to PostgreSQL: {0}")]
    Postgres(PostgresError),
    #[error("failed to initialize PostgreSQL schema: {0}")]
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
    let postgres = PostgresClient::connect(&config.postgres)
        .await
        .map_err(BootstrapError::Postgres)?;
    migrations::migrate(&postgres)
        .await
        .map_err(BootstrapError::Migration)?;
    let postgres = postgres
        .with_writer_lease()
        .await
        .map_err(BootstrapError::Postgres)?;

    let helix =
        Arc::new(HelixClient::from_config(&config.helix).map_err(BootstrapError::HelixClient)?);
    if !crate::domains::documents::store::identity::ready(&postgres)
        .await
        .map_err(|e| BootstrapError::HelixIndexes(e.to_string()))?
    {
        return Err(BootstrapError::HelixIndexes(
            "document scope checkpoint incomplete; keep writers drained and restore the matching verified PostgreSQL/Helix backup before startup".into(),
        ));
    }
    DocumentGraphWriter::new(helix.clone(), config.helix.vector_dimension)
        .initialize()
        .await
        .map_err(|error| BootstrapError::HelixIndexes(error.to_string()))?;

    NotebookPipeline::new(
        helix.clone(),
        None,
        config.helix.vector_dimension,
        config
            .openai
            .as_ref()
            .map(|c| c.embedding_model.clone())
            .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.into()),
    )
    .initialize()
    .await
    .map_err(BootstrapError::HelixIndexes)?;
    let http = reqwest::Client::builder()
        .build()
        .map_err(BootstrapError::HttpClient)?;
    let (api, templates) = assemble_api(&config, postgres, helix, http, prompts).await?;
    let bind_address = config.http.bind_address;
    let router = create_router(api, &config.http);
    Ok(Application {
        router,
        bind_address,
        templates,
    })
}

async fn assemble_api(
    config: &AppConfig,
    postgres: PostgresClient,
    helix: Arc<HelixClient>,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
) -> Result<(Router, Arc<templates::slide_templates::SlideTemplates>), BootstrapError> {
    use crate::adapters::slide_templates::preview as native;
    use templates::slide_templates::*;
    let settings = &config.slide_templates;
    let repository = Arc::new(PostgresTemplateRepository::new(postgres.clone()));
    repository.ensure_app(APP_ID, None).await.map_err(|_| {
        BootstrapError::SlideTemplates(
            "shared PostgreSQL template catalog initialization failed".into(),
        )
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
    let classification = settings.classification.as_ref().map(|c| {
        Arc::new(SlideClassificationService::new(
            SlideClassificationServiceOptions {
                classifier: Arc::new(OpenAISlideClassifier::new(OpenAISlideClassifierOptions {
                    api_key: c.key.expose().into(),
                    base_url: None,
                    model: c.model.clone(),
                    timeout: c.timeout,
                    max_output_tokens: c.max_output_tokens,
                })),
                classification_model: c.model.clone(),
                embedder: Arc::new(OpenAISlideEmbedder::new(OpenAISlideEmbedderOptions {
                    api_key: c.key.expose().into(),
                    base_url: None,
                    model: c.embedding_model.clone(),
                    timeout: c.timeout,
                    dimensions: Some(c.dimensions.get()),
                    max_input_bytes: c.max_embedding_text_bytes,
                })),
                embedding_dimensions: Some(c.dimensions.get()),
                embedding_max_text_bytes: c.max_embedding_text_bytes,
                embedding_model: c.embedding_model.clone(),
                input_limits: SlideClassificationInputLimits::default(),
                repository: repository.clone(),
            },
        ))
    });
    let dimensions = settings.embedding_dimensions;
    let graph = Arc::new(graph::TemplateGraph::new(helix.clone(), dimensions));
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
            "Helix template reconciliation failed; canonical PostgreSQL records were preserved"
                .into(),
        )
    })?;
    embedded.resume_pending_jobs();
    let api = assemble_api_routes(
        config,
        postgres,
        helix,
        http,
        prompts,
        Some(embedded.clone()),
    );
    Ok((api, embedded))
}

pub(crate) fn assemble_api_routes(
    config: &AppConfig,
    postgres: PostgresClient,
    document_graph: Arc<HelixClient>,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
    embedded: Option<Arc<templates::slide_templates::SlideTemplates>>,
) -> Router {
    let templates = Arc::new(TemplateService::new(embedded.clone()));
    let identity = Arc::new(IdentityService::new(IdentityRepository::new(
        postgres.clone(),
        UserRepository::new(postgres.clone()),
    )));
    let memberships = Arc::new(
        crate::domains::memberships::service::DealAccessService::new(
            crate::domains::memberships::repository::MembershipRepository::new(postgres.clone()),
        ),
    );
    let access = ResourceAccess::new(postgres.clone(), memberships.clone());
    let users_repository = UserRepository::new(postgres.clone());
    let deals_repository = DealRepository::new(postgres.clone());
    let document_files = DocumentStore::new(postgres.clone());
    let document_graph_reader = DocumentGraphReader::new(document_graph.clone());
    let document_graph_writer =
        DocumentGraphWriter::new(document_graph.clone(), config.helix.vector_dimension);
    let document_graph_search = DocumentGraphSearch::new(document_graph.clone());
    let openai = config
        .openai
        .as_ref()
        .map(|openai_config| Arc::new(OpenAiClient::from_config(openai_config, prompts.clone())));
    let notebooks = NotebookService::new(
        NotebookRepository::new(postgres.clone()),
        Arc::new(NotebookPipeline::new(
            document_graph.clone(),
            openai.clone(),
            config.helix.vector_dimension,
            config
                .openai
                .as_ref()
                .map(|c| c.embedding_model.clone())
                .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.into()),
        )),
    );
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
        CompanyRepository::new(postgres.clone()),
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
    let document_ingestion = Arc::new(
        DocumentIngestionService::new(
            document_files.clone(),
            document_graph_reader,
            document_graph_writer.clone(),
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
        )
        .with_lifecycle(memberships.lifecycle().clone()),
    );
    let document_jobs = Arc::new(DocumentJobService::new(
        document_ingestion.clone(),
        config.documents.completed_job_retention,
    ));
    let search_embeddings = config
        .openai
        .as_ref()
        .zip(openai.as_ref())
        .map(|(settings, client)| {
            crate::domains::documents::search::embedding::QueryEmbeddingCache::openai(
                client.clone(),
                settings.embedding_model.clone(),
                config.helix.vector_dimension.get(),
                config.search.embedding_concurrency,
                config.search.embedding_timeout,
            )
            .map(Arc::new)
        })
        .transpose()
        .unwrap_or_else(|_| {
            tracing::error!(
                "private search cache initialization failed; semantic search unavailable"
            );
            None
        });
    let document_search = Arc::new(
        DocumentSearchService::new(document_graph_search)
            .with_access(access.clone())
            .with_hybrid(config.search.clone(), search_embeddings),
    );
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
            AssistantChatRepository::new(postgres.clone()),
            user_directory,
            prompts,
        )
        .with_deal_context(deal_context)
        .with_documents(stored_documents.clone())
        .with_lifecycle(memberships.lifecycle().clone()),
    );

    let deletions = deals::deletion::service::DealDeletionService::new(
        deals::deletion::DeletionRepository::new(postgres.clone()),
        document_graph_writer,
        memberships.lifecycle().clone(),
        document_jobs.clone(),
        stored_documents.clone(),
    );
    deletions.resume_pending();
    let native_routes = embedded
        .map(templates::slide_templates::route::routes)
        .unwrap_or_default();
    let protected = Router::new()
        .merge(identity::route::protected(identity.clone()))
        .merge(native_routes)
        .merge(deals::route::routes(deals))
        .merge(deals::deletion::route::routes(deletions))
        .merge(crate::domains::memberships::route::routes(memberships))
        .merge(companies::route::routes(companies))
        .merge(notebooks::route::routes(notebooks))
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

#[cfg(test)]
#[path = "../../tests/unit/app/bootstrap_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/integration/app/bootstrap_support.rs"]
pub(crate) mod test_support;
