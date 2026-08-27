use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    errors::AppResult,
    handlers::{AppError, AppState},
    repository::document_repository::DealDocumentSummary,
    services::stored_document_service::{
        list_deal_documents, load_deal_document, render_stored_document_as_pdf,
        render_stored_document_as_text, StoredDocumentText,
    },
    utils::require_non_empty,
};

pub(crate) async fn list_deal_documents_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
) -> AppResult<Json<Vec<DealDocumentSummary>>> {
    require_non_empty(&deal_id, "dealId").map_err(AppError::bad_request)?;
    list_deal_documents(&state, &deal_id)
        .await
        .map(Json)
        .map_err(AppError::internal)
}

pub(crate) async fn get_deal_document_text_handler(
    State(state): State<AppState>,
    Path((deal_id, file_id)): Path<(String, String)>,
) -> AppResult<Json<StoredDocumentText>> {
    require_non_empty(&deal_id, "dealId").map_err(AppError::bad_request)?;
    require_non_empty(&file_id, "fileId").map_err(AppError::bad_request)?;

    let requested_file_id = file_id.clone();
    let document = load_deal_document(&state, &deal_id, &file_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| {
            AppError::not_found(format!(
                "file `{requested_file_id}` was not found for deal `{deal_id}`"
            ))
        })?;
    render_stored_document_as_text(document)
        .await
        .map(Json)
        .map_err(AppError::bad_request)
}

pub(crate) async fn get_deal_document_pdf_handler(
    State(state): State<AppState>,
    Path((deal_id, file_id)): Path<(String, String)>,
) -> AppResult<Response> {
    require_non_empty(&deal_id, "dealId").map_err(AppError::bad_request)?;
    require_non_empty(&file_id, "fileId").map_err(AppError::bad_request)?;

    let requested_file_id = file_id.clone();
    let document = load_deal_document(&state, &deal_id, &file_id)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| {
            AppError::not_found(format!(
                "file `{requested_file_id}` was not found for deal `{deal_id}`"
            ))
        })?;
    let pdf_bytes = render_stored_document_as_pdf(document)
        .await
        .map_err(AppError::bad_request)?;

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
