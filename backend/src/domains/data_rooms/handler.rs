use crate::domains::deals::deletion::lifecycle::DealAdmission;
mod validate;

use axum::{
    extract::{Path, State},
    Extension, Json,
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
    admission: Option<Extension<DealAdmission>>,
) -> AppResult<Json<DealDataRoom>> {
    state
        .data_rooms
        .list(deal_id, admission.map(|Extension(guard)| guard))
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn preview_deal_document_handler(
    State(state): State<DataRoomsHttpState>,
    Path(deal_id): Path<String>,
    admission: Option<Extension<DealAdmission>>,
    Json(payload): Json<PreviewDocumentPayload>,
) -> AppResult<Json<DocumentPreview>> {
    validate::preview_path(&payload.relative_path)?;
    state
        .data_rooms
        .preview(
            &deal_id,
            &payload.relative_path,
            admission.map(|Extension(guard)| guard),
        )
        .await
        .map(Json)
        .map_err(AppError::from)
}
