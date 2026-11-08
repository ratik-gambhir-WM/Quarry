use axum::{
    extract::{Multipart, Path, State},
    Json,
};

use super::{
    handler::upload::collect_deal_metadata_input,
    handler::{validate_https_link, validate_sharepoint_link, DealsHttpState},
    service::SaveDealMetadataResponse,
};
use crate::app::http::error::{AppError, AppResult};

pub(super) async fn save_deal_metadata_handler(
    axum::Extension(actor): axum::Extension<crate::domains::identity::model::AuthenticatedUser>,
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
        .save_metadata(&deal_id, input, actor.user.id)
        .await
        .map(Json)
        .map_err(AppError::from)
}
