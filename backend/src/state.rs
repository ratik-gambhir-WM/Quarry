use std::sync::Arc;

use crate::services::{
    data_room_service::DataRoomService, database_service::DatabaseService,
    deal_service::DealService, document_ingestion_service::DocumentIngestionService,
    document_job_service::DocumentJobService, document_search_service::DocumentSearchService,
    document_service::DocumentSummaryService, research_service::ResearchService,
    stored_document_service::StoredDocumentService, user_service::UserService,
};

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<UserService>,
    pub deals: Arc<DealService>,
    pub data_rooms: Arc<DataRoomService>,
    pub database: Arc<DatabaseService>,
    pub document_ingestion: Arc<DocumentIngestionService>,
    pub document_jobs: Arc<DocumentJobService>,
    pub document_search: Arc<DocumentSearchService>,
    pub document_summaries: Arc<DocumentSummaryService>,
    pub stored_documents: Arc<StoredDocumentService>,
    pub research: Arc<ResearchService>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        users: Arc<UserService>,
        deals: Arc<DealService>,
        data_rooms: Arc<DataRoomService>,
        database: Arc<DatabaseService>,
        document_ingestion: Arc<DocumentIngestionService>,
        document_jobs: Arc<DocumentJobService>,
        document_search: Arc<DocumentSearchService>,
        document_summaries: Arc<DocumentSummaryService>,
        stored_documents: Arc<StoredDocumentService>,
        research: Arc<ResearchService>,
    ) -> Self {
        Self {
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
        }
    }
}
