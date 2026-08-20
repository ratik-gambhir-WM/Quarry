use axum::{extract::Multipart, Json};

use super::upload_support::collect_wm_upload_files;
use crate::{
    core::clients::wm_ai_services::{FileExtractResponse, FileUploadServiceClient},
    handlers::AppError,
};

pub(crate) async fn extract_files_handler(
    multipart: Multipart,
) -> crate::errors::AppResult<Json<FileExtractResponse>> {
    let files = collect_wm_upload_files(multipart).await?;
    let client = FileUploadServiceClient::from_env().map_err(AppError::internal)?;

    client
        .extract_files(files)
        .await
        .map(Json)
        .map_err(AppError::bad_request)
}
