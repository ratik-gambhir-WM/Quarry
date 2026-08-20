use axum::{extract::Path, extract::State, Json};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    core::nodes::deal_node::DealNode,
    errors::AppResult,
    handlers::{AppError, AppState},
    services::deal_service::{get_helix_deal, save_helix_deal},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveHelixDealPayload {
    pub user_id: i64,
    pub deal: DealNode,
}

pub(crate) async fn save_helix_deal_handler(
    State(state): State<AppState>,
    Json(payload): Json<SaveHelixDealPayload>,
) -> AppResult<Json<Value>> {
    if payload.user_id <= 0 {
        return Err(AppError::bad_request("userId must be greater than zero"));
    }
    save_helix_deal(&state, payload.deal, payload.user_id)
        .await
        .map(Json)
        .map_err(AppError::internal)
}

pub(crate) async fn get_helix_deal_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
) -> AppResult<Json<Value>> {
    if !deal_id.starts_with("DEAL-") {
        return Err(AppError::bad_request("dealId must start with DEAL-"));
    }
    get_helix_deal(&state, &deal_id)
        .await
        .map(Json)
        .map_err(AppError::internal)
}
