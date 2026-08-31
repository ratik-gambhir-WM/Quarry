use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::{
    handlers::AppError,
    services::data_room_service::{DealDataRoom, DocumentPreview},
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewDocumentPayload {
    pub relative_path: String,
}

pub(crate) async fn list_deal_data_room_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
) -> crate::errors::AppResult<Json<DealDataRoom>> {
    state
        .data_rooms
        .list(deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(crate) async fn preview_deal_document_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
    Json(payload): Json<PreviewDocumentPayload>,
) -> crate::errors::AppResult<Json<DocumentPreview>> {
    state
        .data_rooms
        .preview(&deal_id, &payload.relative_path)
        .await
        .map(Json)
        .map_err(AppError::from)
}
