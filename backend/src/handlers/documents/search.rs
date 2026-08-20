use axum::{extract::State, Json};
use serde_json::Value;

use crate::{
    core::helix_queries::files::search_quarry_file::{ChunkKeywordSearch, ChunkVectorSearch},
    errors::AppResult,
    handlers::{AppError, AppState},
    services::document_ingestion_service::{search_chunks_by_keyword, search_chunks_by_vector},
};

pub(crate) async fn vector_search_handler(
    State(state): State<AppState>,
    Json(search): Json<ChunkVectorSearch>,
) -> AppResult<Json<Value>> {
    validate_vector_search(&search)?;
    search_chunks_by_vector(&state, search)
        .await
        .map(Json)
        .map_err(AppError::internal)
}

pub(crate) async fn keyword_search_handler(
    State(state): State<AppState>,
    Json(search): Json<ChunkKeywordSearch>,
) -> AppResult<Json<Value>> {
    validate_keyword_search(&search)?;
    search_chunks_by_keyword(&state, search)
        .await
        .map(Json)
        .map_err(AppError::internal)
}

fn validate_vector_search(search: &ChunkVectorSearch) -> AppResult<()> {
    validate_common(&search.user_id, search.limit)?;
    if search.query_embedding.is_empty() {
        return Err(AppError::bad_request("queryEmbedding cannot be empty"));
    }
    Ok(())
}

fn validate_keyword_search(search: &ChunkKeywordSearch) -> AppResult<()> {
    validate_common(&search.user_id, search.limit)?;
    if search.query_text.trim().is_empty() {
        return Err(AppError::bad_request("queryText cannot be empty"));
    }
    Ok(())
}

fn validate_common(user_id: &str, limit: usize) -> AppResult<()> {
    if user_id.trim().is_empty() {
        return Err(AppError::bad_request("userId is required"));
    }
    if limit == 0 {
        return Err(AppError::bad_request("limit must be greater than zero"));
    }
    Ok(())
}
