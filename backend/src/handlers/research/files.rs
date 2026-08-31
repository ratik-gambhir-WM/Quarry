use axum::{
    extract::{Multipart, State},
    Json,
};

use super::upload_support::collect_wm_upload_files;
use crate::{
    core::clients::wm_ai_services::FileExtractResponse,
    handlers::{AppError, AppState},
};

pub(crate) async fn extract_files_handler(
    State(state): State<AppState>,
    multipart: Multipart,
) -> crate::errors::AppResult<Json<FileExtractResponse>> {
    let files = collect_wm_upload_files(multipart).await?;
    state
        .research
        .extract_files(files)
        .await
        .map(Json)
        .map_err(AppError::from)
}
