use axum::{
    extract::{Multipart, Path, State},
    Json,
};

use super::{
    handler::{validate_https_link, validate_sharepoint_link, DealsHttpState},
    service::SaveDealMetadataResponse,
    upload::collect_deal_metadata_input,
};
use crate::app::http::error::{AppError, AppResult};

pub(super) async fn save_deal_metadata_handler(
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
    multipart: Multipart,
) -> AppResult<Json<SaveDealMetadataResponse>> {
    let input = collect_deal_metadata_input(multipart).await?;
    validate_sharepoint_link(input.sharepoint_link.as_deref())?;
    validate_https_link("sowLink", input.sow_link.as_deref())?;
    validate_https_link("factSheetLink", input.fact_sheet_link.as_deref())?;
    validate_https_link("rlLink", input.rl_link.as_deref())?;
    state
        .deals
        .save_metadata(&deal_id, input)
        .await
        .map(Json)
        .map_err(AppError::from)
}
