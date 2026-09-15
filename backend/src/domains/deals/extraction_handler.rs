use axum::{
    extract::{Multipart, Path, State},
    Json,
};

use super::{
    handler::DealsHttpState, service::SaveDealMetadataResponse, upload::collect_deal_metadata_input,
};
use crate::app::http::error::{AppError, AppResult};

pub(super) async fn save_deal_metadata_handler(
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
    multipart: Multipart,
) -> AppResult<Json<SaveDealMetadataResponse>> {
    let input = collect_deal_metadata_input(multipart).await?;
    state
        .deals
        .save_metadata(&deal_id, input)
        .await
        .map(Json)
        .map_err(AppError::from)
}
