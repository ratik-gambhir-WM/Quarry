use axum::{
    extract::{Multipart, Path, State},
    Json,
};

use super::{
    handler::DealsHttpState, service::SaveDealMetadataResponse,
    upload::collect_selected_deal_uploads,
};
use crate::app::http::error::{AppError, AppResult};

pub(super) async fn save_deal_metadata_handler(
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
    multipart: Multipart,
) -> AppResult<Json<SaveDealMetadataResponse>> {
    let files = collect_selected_deal_uploads(multipart).await?;
    state
        .deals
        .save_metadata(&deal_id, files)
        .await
        .map(Json)
        .map_err(AppError::from)
}
