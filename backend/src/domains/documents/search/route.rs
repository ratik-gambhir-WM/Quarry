use axum::{routing::post, Router};
use std::sync::Arc;

use super::{
    handler::{keyword_search_handler, vector_search_handler, DocumentSearchHttpState},
    service::DocumentSearchService,
};

pub(crate) fn routes(document_search: Arc<DocumentSearchService>) -> Router {
    Router::new()
        .route("/documents/search/vector", post(vector_search_handler))
        .route("/documents/search/keyword", post(keyword_search_handler))
        .with_state(DocumentSearchHttpState { document_search })
}
