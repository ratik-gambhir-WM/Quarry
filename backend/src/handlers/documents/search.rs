use axum::{extract::State, Json};

use crate::{
    core::helix_queries::files::search_quarry_file::{
        FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit, VectorFileChunkHit,
    },
    errors::AppResult,
    handlers::{AppError, AppState},
};

pub(crate) async fn vector_search_handler(
    State(state): State<AppState>,
    Json(search): Json<FileChunkVectorSearch>,
) -> AppResult<Json<Vec<VectorFileChunkHit>>> {
    state
        .document_search
        .vector(search)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(crate) async fn keyword_search_handler(
    State(state): State<AppState>,
    Json(search): Json<FileChunkKeywordSearch>,
) -> AppResult<Json<Vec<KeywordFileChunkHit>>> {
    state
        .document_search
        .keyword(search)
        .await
        .map(Json)
        .map_err(AppError::from)
}
