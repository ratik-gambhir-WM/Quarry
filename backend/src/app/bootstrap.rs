use std::sync::Arc;

use axum::Router;
use thiserror::Error;

use crate::{
    adapters::{
        office::converter::OfficeConverter,
        openai::{client::OpenAiClient, decisions::DecisionsClient},
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
            classification::{
                self, repository::ClassificationRepository, service::ClassificationService,
            },
            ingestion::{self, job_service::DocumentJobService, service::DocumentIngestionService},
            search::{self, embedding::QueryEmbeddings, service::DocumentSearchService},
            store::repository::DocumentStore,
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
    shared::{
        prompts::{PromptCatalog, PromptCatalogError},
        worker::WorkerHandle,
    },
};

const DEFAULT_DEAL_EXTRACTION_MODEL: &str = "gpt-5.6-luna";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_IMAGE_DESCRIPTION_MODEL: &str = "gpt-5.5";

pub struct Application {
    pub router: Router,
    pub bind_address: std::net::SocketAddr,
    pub templates: Arc<templates::slide_templates::SlideTemplates>,
    pub workers: BackgroundWorkers,
}

/// Background supervisors created once by bootstrap and owned by the application.
#[derive(Clone)]
pub struct BackgroundWorkers {
    documents: WorkerHandle,
}

impl BackgroundWorkers {
    /// Stop document claims, then drain owned tasks within the bounded grace period.
    /// Leases make aborted work recoverable.
    pub async fn stop(&self) {
        self.documents.stop().await;
    }
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
}

/// Typed vector columns for the shared document/notebook family and the template family.
pub fn schema_dimensions(config: &AppConfig) -> migrations::SchemaDimensions {
    migrations::SchemaDimensions {
        shared: config.embeddings.vector_dimension.get(),
        template: config.slide_templates.embedding_dimensions.get(),
    }
}

pub async fn bootstrap(config: AppConfig) -> Result<Application, BootstrapError> {
    let prompts = Arc::new(
        PromptCatalog::load(&config.prompts.directory).map_err(BootstrapError::PromptCatalog)?,
    );
    let postgres = PostgresClient::connect(&config.postgres)
        .await
        .map_err(BootstrapError::Postgres)?;
    migrations::migrate(&postgres, schema_dimensions(&config))
        .await
        .map_err(BootstrapError::Migration)?;
    if !crate::domains::documents::store::identity_repository::ready(&postgres)
        .await
        .map_err(|_| BootstrapError::Migration(MigrationError::Incompatible))?
    {
        return Err(BootstrapError::Migration(MigrationError::Incompatible));
    }
    let postgres = postgres
        .with_writer_lease()
        .await
        .map_err(BootstrapError::Postgres)?;

    let http = reqwest::Client::builder()
        .build()
        .map_err(BootstrapError::HttpClient)?;
    let (api, templates, workers) = assemble_api(&config, postgres, http, prompts).await?;
    let bind_address = config.http.bind_address;
    let router = create_router(api, &config.http);
    Ok(Application {
        router,
        bind_address,
        templates,
        workers,
    })
}

async fn assemble_api(
    config: &AppConfig,
    postgres: PostgresClient,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
) -> Result<
    (
        Router,
        Arc<templates::slide_templates::SlideTemplates>,
        BackgroundWorkers,
    ),
    BootstrapError,
> {
    use crate::adapters::slide_templates::preview as native;
    use templates::slide_templates::*;
    let settings = &config.slide_templates;
    let repository = Arc::new(PostgresTemplateRepository::new(postgres.clone()));
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
    // PostgreSQL is the only template store: startup validated the schema, pair integrity
    // and indexes; no catalog-wide reconciliation or re-embedding runs here.
    let embedded = Arc::new(SlideTemplates::new(
        repository,
        preview,
        classification,
        settings.allow_embedding_inspection,
    ));
    embedded.resume_pending_jobs();
    let (api, workers) =
        assemble_api_components(config, postgres, http, prompts, Some(embedded.clone()));
    Ok((api, embedded, workers))
}

fn assemble_api_components(
    config: &AppConfig,
    postgres: PostgresClient,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
    embedded: Option<Arc<templates::slide_templates::SlideTemplates>>,
) -> (Router, BackgroundWorkers) {
    let templates = Arc::new(TemplateService::new(embedded));
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
    let document_files = DocumentStore::new(postgres.clone()).with_profile(
        config
            .openai
            .as_ref()
            .map(|c| c.embedding_model.clone())
            .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.into()),
        config.embeddings.vector_dimension.get(),
    );
    let document_search_repository =
        crate::domains::documents::store::search_repository::DocumentSearchRepository::new(
            postgres.clone(),
            document_files.profile().clone(),
        );
    let openai = config
        .openai
        .as_ref()
        .map(|openai_config| Arc::new(OpenAiClient::from_config(openai_config, prompts.clone())));
    let notebooks = NotebookService::new(
        NotebookRepository::new(postgres.clone()),
        Arc::new(NotebookPipeline::new(
            openai.clone(),
            config.embeddings.vector_dimension,
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
    let user_directory = UserDirectory::new(users_repository);
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
    // Decisions shares the bootstrap HTTP client and the server-side OpenAI key.
    let classification = Arc::new(ClassificationService::new(
        ClassificationRepository::new(postgres.clone()),
        config
            .openai
            .as_ref()
            .map(|openai| DecisionsClient::new(http.clone(), openai.api_key.expose())),
        prompts.clone(),
        config.documents.classification.enabled,
        config.documents.classification.concurrency,
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
            crate::domains::documents::ingestion::repository::DocumentJobRepository {
                db: postgres.clone(),
                profile: document_files.profile().clone(),
            },
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
        .with_lifecycle(memberships.lifecycle().clone())
        .with_classification(classification.clone()),
    );
    let document_jobs = Arc::new(DocumentJobService::new(document_ingestion.clone()));
    let workers = BackgroundWorkers {
        documents: document_jobs.start_worker(
            config.documents.max_concurrent_background_documents,
            config.documents.completed_job_retention,
        ),
    };
    let search_embeddings =
        config
            .openai
            .as_ref()
            .zip(openai.as_ref())
            .map(|(settings, client)| {
                Arc::new(QueryEmbeddings::openai(
                    client.clone(),
                    settings.embedding_model.clone(),
                    config.embeddings.vector_dimension.get(),
                    config.search.embedding_concurrency,
                    config.search.embedding_timeout,
                ))
            });
    let document_search = Arc::new(
        DocumentSearchService::new(document_search_repository)
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
        deals::deletion::DeletionRepository::new(postgres),
        memberships.lifecycle().clone(),
        stored_documents.clone(),
    );
    deletions.resume_pending();
    let protected = Router::new()
        .merge(identity::route::protected(identity.clone()))
        .merge(deals::route::routes(deals))
        .merge(deals::deletion::route::routes(deletions))
        .merge(crate::domains::memberships::route::routes(memberships))
        .merge(companies::route::routes(companies))
        .merge(notebooks::route::routes(notebooks))
        .merge(data_rooms::route::routes(data_rooms))
        .merge(ingestion::route::routes(document_ingestion, document_jobs))
        .merge(viewing::route::routes(stored_documents))
        .merge(classification::route::routes(classification))
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
    let router = Router::new()
        .merge(protected)
        .merge(system::route::routes())
        .merge(identity::route::public(identity));
    (router, workers)
}

#[cfg(test)]
#[path = "../../tests/unit/app/bootstrap_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/integration/app/bootstrap_support.rs"]
pub(crate) mod test_support;
