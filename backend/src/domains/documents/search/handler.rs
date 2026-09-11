use axum::{extract::State, Json};

use crate::{
    app::http::error::{AppError, AppResult},
    domains::documents::{
        index::query::{
            FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit, VectorFileChunkHit,
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
    state
        .document_search
        .keyword(search)
        .await
        .map(Json)
        .map_err(AppError::from)
}
