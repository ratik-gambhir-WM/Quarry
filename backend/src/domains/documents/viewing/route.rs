use axum::{routing::get, Router};
use std::sync::Arc;

use super::{
    handler::{
        get_deal_document_pdf_handler, get_deal_document_text_handler, list_deal_documents_handler,
        DocumentViewingHttpState,
    },
    service::StoredDocumentService,
};

pub(crate) fn routes(stored_documents: Arc<StoredDocumentService>) -> Router {
    Router::new()
        .route(
            "/deals/{deal_id}/documents",
            get(list_deal_documents_handler),
        )
        .route(
            "/deals/{deal_id}/documents/{file_id}/pdf",
            get(get_deal_document_pdf_handler),
        )
        .route(
            "/deals/{deal_id}/documents/{file_id}/text",
            get(get_deal_document_text_handler),
        )
        .with_state(DocumentViewingHttpState { stored_documents })
}
