use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    app::http::error::{AppError, AppResult},
    domains::data_rooms::service::{DataRoomService, DealDataRoom, DocumentPreview},
};

#[derive(Clone)]
pub(super) struct DataRoomsHttpState {
    pub data_rooms: Arc<DataRoomService>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewDocumentPayload {
    pub relative_path: String,
}

pub(super) async fn list_deal_data_room_handler(
    State(state): State<DataRoomsHttpState>,
    Path(deal_id): Path<String>,
) -> AppResult<Json<DealDataRoom>> {
    state
        .data_rooms
        .list(deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn preview_deal_document_handler(
    State(state): State<DataRoomsHttpState>,
    Path(deal_id): Path<String>,
    Json(payload): Json<PreviewDocumentPayload>,
) -> AppResult<Json<DocumentPreview>> {
    state
        .data_rooms
        .preview(&deal_id, &payload.relative_path)
        .await
        .map(Json)
        .map_err(AppError::from)
}
