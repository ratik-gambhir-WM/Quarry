use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};

use super::upload_support::{collect_deal_upload, collect_selected_deal_uploads};
use crate::{
    handlers::{run_blocking, AppError, AppState},
    services::deal_service::{
        create_uploaded_deal, extract_local_deal, extract_uploaded_deal, ExtractDealQuestionsInput,
        SaveDealAndExtractResponse, SaveDealAndFindFilesResponse,
    },
};

pub(crate) async fn create_deal_upload_handler(
    State(state): State<AppState>,
    multipart: Multipart,
) -> crate::errors::AppResult<(StatusCode, Json<SaveDealAndFindFilesResponse>)> {
    let (input, root_label, files) = collect_deal_upload(multipart).await?;
    run_blocking(move || create_uploaded_deal(&state, input, &root_label, &files))
        .await
        .map(|response| (StatusCode::CREATED, Json(response)))
        .map_err(AppError::bad_request)
}

pub(crate) async fn extract_deal_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<i64>,
    Json(input): Json<ExtractDealQuestionsInput>,
) -> crate::errors::AppResult<Json<SaveDealAndExtractResponse>> {
    extract_local_deal(&state, deal_id, input)
        .await
        .map(Json)
        .map_err(map_deal_error)
}

pub(crate) async fn extract_deal_upload_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<i64>,
    multipart: Multipart,
) -> crate::errors::AppResult<Json<SaveDealAndExtractResponse>> {
    let files = collect_selected_deal_uploads(multipart).await?;
    extract_uploaded_deal(&state, deal_id, files)
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
