use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    errors::AppResult,
    handlers::{AppError, AppState},
    services::stored_document_service::{DealDocumentSummary, StoredDocumentText},
};

pub(crate) async fn list_deal_documents_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
) -> AppResult<Json<Vec<DealDocumentSummary>>> {
    state
        .stored_documents
        .list(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(crate) async fn get_deal_document_text_handler(
    State(state): State<AppState>,
    Path((deal_id, file_id)): Path<(String, String)>,
) -> AppResult<Json<StoredDocumentText>> {
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

pub(crate) async fn get_deal_document_pdf_handler(
    State(state): State<AppState>,
    Path((deal_id, file_id)): Path<(String, String)>,
) -> AppResult<Response> {
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
