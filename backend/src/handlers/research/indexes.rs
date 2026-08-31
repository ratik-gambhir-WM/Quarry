use axum::{
    extract::{Path, State},
    Json,
};

use crate::{
    core::clients::wm_ai_services::{
        CreateIndexPayload, CreateIndexResponse, GraphRagQueryPayload, GraphRagQueryResponse,
        IndexStatusResponse,
    },
    handlers::{AppError, AppState},
};

pub(crate) async fn create_index_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateIndexPayload>,
) -> crate::errors::AppResult<Json<CreateIndexResponse>> {
    state
        .research
        .create_index(payload)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(crate) async fn index_status_handler(
    State(state): State<AppState>,
    Path(index_id): Path<String>,
) -> crate::errors::AppResult<Json<IndexStatusResponse>> {
    state
        .research
        .index_status(&index_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(crate) async fn graph_rag_query_handler(
    State(state): State<AppState>,
    Json(payload): Json<GraphRagQueryPayload>,
) -> crate::errors::AppResult<Json<GraphRagQueryResponse>> {
    state
        .research
        .graph_rag_query(payload)
        .await
        .map(Json)
        .map_err(AppError::from)
}
