use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::{
    app::http::error::{AppError, AppResult},
    domains::deals::service::{
        Deal, DealService, DealWithMetadata, SaveDealInput, SaveDealResponse,
    },
};

#[derive(Clone)]
pub(super) struct DealsHttpState {
    pub deals: Arc<DealService>,
}

pub(super) async fn list_deals_handler(
    State(state): State<DealsHttpState>,
) -> AppResult<Json<Vec<DealWithMetadata>>> {
    state.deals.list().await.map(Json).map_err(AppError::from)
}

pub(super) async fn get_deal_handler(
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
) -> crate::app::http::error::AppResult<Json<DealWithMetadata>> {
    state
        .deals
        .get(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn create_deal_handler(
    State(state): State<DealsHttpState>,
    Json(input): Json<SaveDealInput>,
) -> crate::app::http::error::AppResult<(StatusCode, Json<SaveDealResponse>)> {
    state
        .deals
        .create(input)
        .await
        .map(|response| (StatusCode::CREATED, Json(response)))
        .map_err(AppError::from)
}

pub(super) async fn archive_deal_handler(
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
) -> crate::app::http::error::AppResult<Json<Deal>> {
    state
        .deals
        .archive(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}
