use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::{
    handler::{
        process_document_job_events_handler, process_documents_handler, start_process_file_handler,
        DocumentIngestionHttpState,
    },
    job_service::DocumentJobService,
    service::DocumentIngestionService,
};
use crate::shared::file_policy::MAX_TOTAL_REQUEST_FILE_BYTES;

const MULTIPART_OVERHEAD_BYTES: usize = 1024 * 1024;
const DOCUMENT_UPLOAD_BODY_BYTES: usize = MAX_TOTAL_REQUEST_FILE_BYTES + MULTIPART_OVERHEAD_BYTES;

pub(crate) fn routes(
    document_ingestion: Arc<DocumentIngestionService>,
    document_jobs: Arc<DocumentJobService>,
) -> Router {
    Router::new()
        .route(
            "/deals/{deal_id}/documents/process",
            post(process_documents_handler)
                .layer(DefaultBodyLimit::max(DOCUMENT_UPLOAD_BODY_BYTES)),
        )
        .route(
            "/deals/{deal_id}/documents/process_file",
            post(start_process_file_handler)
                .layer(DefaultBodyLimit::max(DOCUMENT_UPLOAD_BODY_BYTES)),
        )
        .route(
            "/documents/process_file/{job_id}/events",
            get(process_document_job_events_handler),
        )
        .with_state(DocumentIngestionHttpState {
            document_ingestion,
            document_jobs,
        })
}
