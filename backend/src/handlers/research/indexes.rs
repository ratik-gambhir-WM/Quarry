use axum::{extract::Path, Json};

use crate::{
    core::clients::wm_ai_services::{
        validate_create_index_payload, validate_graph_rag_query_payload, CreateIndexPayload,
        CreateIndexResponse, GraphRagClient, GraphRagQueryPayload, GraphRagQueryResponse,
        IndexServiceClient, IndexStatusResponse,
    },
    handlers::AppError,
};

pub(crate) async fn create_index_handler(
    Json(payload): Json<CreateIndexPayload>,
) -> crate::errors::AppResult<Json<CreateIndexResponse>> {
    validate_create_index_payload(&payload).map_err(AppError::bad_request)?;
    let client = IndexServiceClient::from_env().map_err(AppError::internal)?;

    client
        .create_index(payload)
        .await
        .map(Json)
        .map_err(AppError::bad_request)
}

pub(crate) async fn index_status_handler(
    Path(index_id): Path<String>,
) -> crate::errors::AppResult<Json<IndexStatusResponse>> {
    let client = IndexServiceClient::from_env().map_err(AppError::internal)?;

    client
        .status(&index_id)
        .await
        .map(Json)
        .map_err(AppError::bad_request)
}

pub(crate) async fn graph_rag_query_handler(
    Json(payload): Json<GraphRagQueryPayload>,
) -> crate::errors::AppResult<Json<GraphRagQueryResponse>> {
    validate_graph_rag_query_payload(&payload).map_err(AppError::bad_request)?;
    let client = GraphRagClient::from_env().map_err(AppError::internal)?;

    client
        .query(payload)
        .await
        .map(Json)
        .map_err(AppError::bad_request)
}
