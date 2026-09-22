use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    app::http::error::{AppError, AppResult},
    domains::documents::viewing::service::{
        DealDocumentSummary, StoredDocumentService, StoredDocumentText,
    },
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct DocumentViewingHttpState {
    pub stored_documents: Arc<StoredDocumentService>,
}

pub(super) async fn list_deal_documents_handler(
    State(state): State<DocumentViewingHttpState>,
    Path(deal_id): Path<String>,
) -> AppResult<Json<Vec<DealDocumentSummary>>> {
    validate_identifier("dealId", &deal_id)?;
    state
        .stored_documents
        .list(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn get_deal_document_text_handler(
    State(state): State<DocumentViewingHttpState>,
    Path((deal_id, file_id)): Path<(String, String)>,
) -> AppResult<Json<StoredDocumentText>> {
    validate_identifier("dealId", &deal_id)?;
    validate_identifier("fileId", &file_id)?;
    let document = state
        .stored_documents
        .load(&deal_id, &file_id)
        .await
        .map_err(AppError::from)?;
    state
        .stored_documents
        .render_text(document)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn get_deal_document_pdf_handler(
    State(state): State<DocumentViewingHttpState>,
    Path((deal_id, file_id)): Path<(String, String)>,
) -> AppResult<Response> {
    validate_identifier("dealId", &deal_id)?;
    validate_identifier("fileId", &file_id)?;
    let document = state
        .stored_documents
        .load(&deal_id, &file_id)
        .await
        .map_err(AppError::from)?;
    let pdf_bytes = state
        .stored_documents
        .render_pdf(document)
        .await
        .map_err(AppError::from)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok((headers, pdf_bytes).into_response())
}

fn validate_identifier(field: &str, value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::bad_request(format!("{field} is required")));
    }
    Ok(())
}
