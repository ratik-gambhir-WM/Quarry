use axum::{extract::Path, extract::State, Json};
use serde_json::Value;

use crate::{
    core::nodes::deal_node::DealNode,
    errors::AppResult,
    handlers::{AppError, AppState},
    services::deal_service::{get_helix_deal, save_helix_deal},
};

pub(crate) async fn save_helix_deal_handler(
    State(state): State<AppState>,
    Json(deal): Json<DealNode>,
) -> AppResult<Json<Value>> {
    save_helix_deal(&state, deal)
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
