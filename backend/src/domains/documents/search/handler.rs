use validate::validate_common;
mod validate;
use crate::domains::identity::model::AuthenticatedUser;
use axum::Extension;
use axum::{extract::State, Json};

use crate::{
    app::http::error::{AppError, AppResult},
    domains::documents::{
        graph::query::{
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
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<DocumentSearchHttpState>,
    path: Option<axum::extract::Path<String>>,
    Json(mut search): Json<FileChunkVectorSearch>,
) -> AppResult<Json<Vec<VectorFileChunkHit>>> {
    search.deal_id = path.map(|p| p.0).unwrap_or_default();
    validate_common(search.limit)?;
    if search.query_embedding.is_empty() || search.query_embedding.len() > 4096 {
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
        .vector_owned(search, actor.user.id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn keyword_search_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<DocumentSearchHttpState>,
    path: Option<axum::extract::Path<String>>,
    Json(mut search): Json<FileChunkKeywordSearch>,
) -> AppResult<Json<Vec<KeywordFileChunkHit>>> {
    search.deal_id = path.map(|p| p.0).unwrap_or_default();
    validate_common(search.limit)?;
    if search.query_text.trim().is_empty() || search.query_text.len() > 2000 {
        return Err(AppError::bad_request("queryText cannot be empty"));
    }
    state
        .document_search
        .keyword_owned(search, actor.user.id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn hybrid_search_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<DocumentSearchHttpState>,
    path: Option<axum::extract::Path<String>>,
    input: Result<Json<super::model::HybridSearch>, axum::extract::rejection::JsonRejection>,
) -> AppResult<Json<super::model::HybridResponse>> {
    let Json(mut search) =
        input.map_err(|_| AppError::bad_request("invalid hybrid search input"))?;
    let deal = path.map(|p| p.0).unwrap_or_default();
    if !deal.is_empty() && !crate::shared::ids::is_deal_id(&deal) {
        return Err(AppError::bad_request("invalid dealId"));
    }
    validate_common(search.limit)?;
    search.query_text = search.query_text.trim().to_owned();
    if search.query_text.is_empty() || search.query_text.len() > 2000 {
        return Err(AppError::bad_request(
            "queryText must contain 1 to 2000 bytes",
        ));
    }
    state
        .document_search
        .hybrid_owned(search, actor.user.id, &deal)
        .await
        .map(Json)
        .map_err(AppError::from)
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/search/handler_tests.rs"]
mod tests;
