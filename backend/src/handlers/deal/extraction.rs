use axum::{
    extract::{Multipart, Path, State},
    Json,
};

use super::upload_support::collect_selected_deal_uploads;
use crate::{
    handlers::{AppError, AppState},
    services::deal_service::{save_deal_metadata, SaveDealMetadataResponse},
};

pub(crate) async fn save_deal_metadata_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
    multipart: Multipart,
) -> crate::errors::AppResult<Json<SaveDealMetadataResponse>> {
    let files = collect_selected_deal_uploads(multipart).await?;
    save_deal_metadata(&state, &deal_id, files)
        .await
        .map(Json)
        .map_err(map_deal_error)
}

fn map_deal_error(message: String) -> AppError {
    if message.starts_with("deal not found") {
        AppError::not_found(message)
    } else {
        AppError::bad_request(message)
    }
}
