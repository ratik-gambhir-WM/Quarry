#[path = "handler/validate.rs"]
mod validate;
use validate::{
    validate_create_index_payload, validate_graph_rag_query_payload, validate_path_segment,
};

use axum::{
    extract::{Path, State},
    Json,
};

use crate::{
    app::http::error::{AppError, AppResult},
    domains::research::{
        files_handler::ResearchHttpState,
        service::{
            CreateIndexPayload, CreateIndexResponse, GraphRagQueryPayload, GraphRagQueryResponse,
            IndexStatusResponse,
        },
    },
};

pub(super) async fn create_index_handler(
    State(state): State<ResearchHttpState>,
    Json(payload): Json<CreateIndexPayload>,
) -> AppResult<Json<CreateIndexResponse>> {
    validate_create_index_payload(&payload)?;
    state
        .research
        .create_index(payload)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn index_status_handler(
    State(state): State<ResearchHttpState>,
    Path(index_id): Path<String>,
) -> AppResult<Json<IndexStatusResponse>> {
    validate_path_segment(&index_id, "indexId")?;
    state
        .research
        .index_status(&index_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn graph_rag_query_handler(
    State(state): State<ResearchHttpState>,
    Json(payload): Json<GraphRagQueryPayload>,
) -> AppResult<Json<GraphRagQueryResponse>> {
    validate_graph_rag_query_payload(&payload)?;
    validate_path_segment(&payload.resource_id, "resourceId")?;
    state
        .research
        .graph_rag_query(payload)
        .await
        .map(Json)
        .map_err(AppError::from)
}
