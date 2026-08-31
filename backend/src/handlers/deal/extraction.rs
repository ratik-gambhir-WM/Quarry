use axum::{
    extract::{Multipart, Path, State},
    Json,
};

use super::upload_support::collect_selected_deal_uploads;
use crate::{
    handlers::{AppError, AppState},
    services::deal_service::SaveDealMetadataResponse,
};

pub(crate) async fn save_deal_metadata_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
    multipart: Multipart,
) -> crate::errors::AppResult<Json<SaveDealMetadataResponse>> {
    let files = collect_selected_deal_uploads(multipart).await?;
    state
        .deals
        .save_metadata(&deal_id, files)
        .await
        .map(Json)
        .map_err(AppError::from)
}
