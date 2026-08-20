use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    handlers::{run_blocking, AppError, AppState},
    repository::deal_repository::{
        archive_deal, get_deal_with_metadata, list_deals, Deal, DealWithMetadata,
    },
    services::deal_service::{save_deal, SaveDealInput, SaveDealResponse},
};

pub(crate) async fn list_deals_handler(
    State(state): State<AppState>,
) -> crate::errors::AppResult<Json<Vec<DealWithMetadata>>> {
    run_blocking(move || list_deals(&state))
        .await
        .map(Json)
        .map_err(AppError::internal)
}

pub(crate) async fn get_deal_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
) -> crate::errors::AppResult<Json<DealWithMetadata>> {
    let requested_deal_id = deal_id.clone();
    run_blocking(move || get_deal_with_metadata(&state, &deal_id))
        .await
        .map_err(AppError::internal)?
        .map(Json)
        .ok_or_else(|| AppError::not_found(format!("deal not found for id `{requested_deal_id}`")))
}

pub(crate) async fn create_deal_handler(
    State(state): State<AppState>,
    Json(input): Json<SaveDealInput>,
) -> crate::errors::AppResult<(StatusCode, Json<SaveDealResponse>)> {
    run_blocking(move || save_deal(&state, input))
        .await
        .map(|response| (StatusCode::CREATED, Json(response)))
        .map_err(AppError::bad_request)
}

pub(crate) async fn archive_deal_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
) -> crate::errors::AppResult<Json<Deal>> {
    let requested_deal_id = deal_id.clone();
    run_blocking(move || archive_deal(&state, &deal_id))
        .await
        .map_err(AppError::internal)?
        .map(Json)
        .ok_or_else(|| AppError::not_found(format!("deal not found for id `{requested_deal_id}`")))
}
