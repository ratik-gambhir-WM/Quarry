use axum::{extract::State, Json};

use crate::{
    core::helix_queries::files::search_quarry_file::{
        FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit, VectorFileChunkHit,
        MAX_FILE_CHUNK_SEARCH_LIMIT,
    },
    errors::AppResult,
    handlers::{AppError, AppState},
    services::document_ingestion_service::{search_chunks_by_keyword, search_chunks_by_vector},
};

pub(crate) async fn vector_search_handler(
    State(state): State<AppState>,
    Json(search): Json<FileChunkVectorSearch>,
) -> AppResult<Json<Vec<VectorFileChunkHit>>> {
    validate_vector_search(&search)?;
    search_chunks_by_vector(&state, search)
        .await
        .map(Json)
        .map_err(AppError::internal)
}

pub(crate) async fn keyword_search_handler(
    State(state): State<AppState>,
    Json(search): Json<FileChunkKeywordSearch>,
) -> AppResult<Json<Vec<KeywordFileChunkHit>>> {
    validate_keyword_search(&search)?;
    search_chunks_by_keyword(&state, search)
        .await
        .map(Json)
        .map_err(AppError::internal)
}

fn validate_vector_search(search: &FileChunkVectorSearch) -> AppResult<()> {
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
    Ok(())
}

fn validate_keyword_search(search: &FileChunkKeywordSearch) -> AppResult<()> {
    validate_common(&search.workspace_id, search.limit)?;
    if search.query_text.trim().is_empty() {
        return Err(AppError::bad_request("queryText cannot be empty"));
    }
    Ok(())
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
