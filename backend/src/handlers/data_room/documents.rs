use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::{
    handlers::AppError,
    services::data_room_service::{
        build_document_preview, list_deal_data_room, DealDataRoom, DocumentPreview,
    },
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
    list_deal_data_room(&state, deal_id)
        .map(Json)
        .map_err(AppError::bad_request)
}

pub(crate) async fn preview_deal_document_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
    Json(payload): Json<PreviewDocumentPayload>,
) -> crate::errors::AppResult<Json<DocumentPreview>> {
    tokio::task::spawn_blocking(move || {
        build_document_preview(&state, &deal_id, &payload.relative_path)
    })
    .await
    .map_err(|err| AppError::internal(format!("document preview worker failed: {err}")))?
    .map(Json)
    .map_err(AppError::bad_request)
}
