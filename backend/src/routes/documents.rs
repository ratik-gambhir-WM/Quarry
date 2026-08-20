use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};

use crate::{
    handlers::documents::{
        keyword_search_handler, process_document_job_events_handler, process_documents_handler,
        start_process_file_handler, vector_search_handler,
    },
    services::document_service::MAX_TOTAL_REQUEST_FILE_BYTES,
    state::AppState,
};

const MULTIPART_OVERHEAD_BYTES: usize = 1024 * 1024;
const DOCUMENT_UPLOAD_BODY_BYTES: usize = MAX_TOTAL_REQUEST_FILE_BYTES + MULTIPART_OVERHEAD_BYTES;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/documents/process",
            post(process_documents_handler)
                .layer(DefaultBodyLimit::max(DOCUMENT_UPLOAD_BODY_BYTES)),
        )
        .route(
            "/documents/process_file",
            post(start_process_file_handler)
                .layer(DefaultBodyLimit::max(DOCUMENT_UPLOAD_BODY_BYTES)),
        )
        .route(
            "/documents/process_file/{job_id}/events",
            get(process_document_job_events_handler),
        )
        .route("/documents/search/vector", post(vector_search_handler))
        .route("/documents/search/keyword", post(keyword_search_handler))
}
