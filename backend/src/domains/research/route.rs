use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::{
    files_handler::{extract_files_handler, ResearchHttpState},
    indexes_handler::{create_index_handler, graph_rag_query_handler, index_status_handler},
    service::ResearchService,
};

pub(crate) fn routes(research: Arc<ResearchService>) -> Router {
    Router::new()
        .route("/files/extract", post(extract_files_handler))
        .route("/indexes", post(create_index_handler))
        .route("/indexes/{index_id}/status", get(index_status_handler))
        .route("/graphrag/query", post(graph_rag_query_handler))
        .with_state(ResearchHttpState { research })
}
