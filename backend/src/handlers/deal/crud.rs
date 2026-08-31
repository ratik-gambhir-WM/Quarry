use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    handlers::{AppError, AppState},
    services::deal_service::{Deal, DealWithMetadata, SaveDealInput, SaveDealResponse},
};

pub(crate) async fn list_deals_handler(
    State(state): State<AppState>,
) -> crate::errors::AppResult<Json<Vec<DealWithMetadata>>> {
    state.deals.list().await.map(Json).map_err(AppError::from)
}

pub(crate) async fn get_deal_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
) -> crate::errors::AppResult<Json<DealWithMetadata>> {
    state
        .deals
        .get(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(crate) async fn create_deal_handler(
    State(state): State<AppState>,
    Json(input): Json<SaveDealInput>,
) -> crate::errors::AppResult<(StatusCode, Json<SaveDealResponse>)> {
    state
        .deals
        .create(input)
        .await
        .map(|response| (StatusCode::CREATED, Json(response)))
        .map_err(AppError::from)
}

pub(crate) async fn archive_deal_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
) -> crate::errors::AppResult<Json<Deal>> {
    state
        .deals
        .archive(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}
