use axum::{extract::State, Json};

use crate::{
    app::http::error::{AppError, AppResult},
    domains::documents::{
        index::query::{
            FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit, VectorFileChunkHit,
            MAX_FILE_CHUNK_SEARCH_LIMIT,
        },
        search::service::DocumentSearchService,
    },
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct DocumentSearchHttpState {
    pub document_search: Arc<DocumentSearchService>,
}

pub(super) async fn vector_search_handler(
    State(state): State<DocumentSearchHttpState>,
    Json(search): Json<FileChunkVectorSearch>,
) -> AppResult<Json<Vec<VectorFileChunkHit>>> {
    validate_common(&search.workspace_id, search.limit)?;
    if search.query_embedding.is_empty() {
        return Err(AppError::bad_request("queryEmbedding cannot be empty"));
    }
    if search
        .query_embedding
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(AppError::bad_request(
            "queryEmbedding must contain only finite values",
        ));
    }
    state
        .document_search
        .vector(search)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn keyword_search_handler(
    State(state): State<DocumentSearchHttpState>,
    Json(search): Json<FileChunkKeywordSearch>,
) -> AppResult<Json<Vec<KeywordFileChunkHit>>> {
    validate_common(&search.workspace_id, search.limit)?;
    if search.query_text.trim().is_empty() {
        return Err(AppError::bad_request("queryText cannot be empty"));
    }
    state
        .document_search
        .keyword(search)
        .await
        .map(Json)
        .map_err(AppError::from)
}

fn validate_common(workspace_id: &str, limit: usize) -> AppResult<()> {
    if workspace_id.trim().is_empty() {
        return Err(AppError::bad_request("workspaceId is required"));
    }
    if limit == 0 {
        return Err(AppError::bad_request("limit must be greater than zero"));
    }
    if limit > MAX_FILE_CHUNK_SEARCH_LIMIT {
        return Err(AppError::bad_request(format!(
            "limit must not exceed {MAX_FILE_CHUNK_SEARCH_LIMIT}"
        )));
    }
    Ok(())
}
