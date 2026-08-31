use crate::{
    core::clients::wm_ai_services::{
        CreateIndexPayload, CreateIndexResponse, FileExtractResponse, FileUploadServiceClient,
        GraphRagClient, GraphRagQueryPayload, GraphRagQueryResponse, IndexServiceClient,
        IndexStatusResponse, WmUploadedFile,
    },
    services::error::{ServiceError, ServiceResult},
};

#[derive(Clone)]
pub struct WmAiClients {
    pub files: FileUploadServiceClient,
    pub indexes: IndexServiceClient,
    pub graph_rag: GraphRagClient,
}

#[derive(Clone)]
pub struct ResearchService {
    clients: Option<WmAiClients>,
}

impl ResearchService {
    pub fn new(clients: Option<WmAiClients>) -> Self {
        Self { clients }
    }

    pub async fn extract_files(
        &self,
        files: Vec<WmUploadedFile>,
    ) -> ServiceResult<FileExtractResponse> {
        self.clients()?
            .files
            .extract_files(files)
            .await
            .map_err(ServiceError::validation)
    }

    pub async fn create_index(
        &self,
        payload: CreateIndexPayload,
    ) -> ServiceResult<CreateIndexResponse> {
        self.clients()?
            .indexes
            .create_index(payload)
            .await
            .map_err(ServiceError::validation)
    }

    pub async fn index_status(&self, index_id: &str) -> ServiceResult<IndexStatusResponse> {
        self.clients()?
            .indexes
            .status(index_id)
            .await
            .map_err(ServiceError::validation)
    }

    pub async fn graph_rag_query(
        &self,
        payload: GraphRagQueryPayload,
    ) -> ServiceResult<GraphRagQueryResponse> {
        self.clients()?
            .graph_rag
            .query(payload)
            .await
            .map_err(ServiceError::validation)
    }

    fn clients(&self) -> ServiceResult<&WmAiClients> {
        self.clients
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("WM AI capability is not configured"))
    }
}
