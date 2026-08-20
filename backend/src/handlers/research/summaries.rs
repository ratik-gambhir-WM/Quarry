use axum::{extract::Multipart, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use super::upload_support::collect_summary_upload;
use crate::{
    core::write_summary,
    errors::AppResult,
    handlers::AppError,
    services::document_service::{
        list_summarizable_files, summarize_collected_files, summarize_dir, summarize_paths,
        SummarizableFile,
    },
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PathPayload {
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SummaryResponse {
    pub summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectedPathsPayload {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveSummaryPayload {
    pub path: String,
    pub summary: String,
}

pub(crate) async fn list_summary_files_handler(
    Json(payload): Json<PathPayload>,
) -> AppResult<Json<Vec<SummarizableFile>>> {
    list_summarizable_files(payload.path)
        .map(Json)
        .map_err(AppError::bad_request)
}

pub(crate) async fn summarize_handler(
    Json(payload): Json<PathPayload>,
) -> AppResult<Json<SummaryResponse>> {
    summarize_dir(payload.path)
        .await
        .map(|summary| Json(SummaryResponse { summary }))
        .map_err(|error| AppError::bad_request(error.to_string()))
}

pub(crate) async fn summarize_selected_handler(
    Json(payload): Json<SelectedPathsPayload>,
) -> AppResult<Json<SummaryResponse>> {
    summarize_paths(payload.paths)
        .await
        .map(|summary| Json(SummaryResponse { summary }))
        .map_err(AppError::bad_request)
}

pub(crate) async fn summarize_upload_handler(
    multipart: Multipart,
) -> AppResult<Json<SummaryResponse>> {
    let upload = collect_summary_upload(multipart).await?;
    summarize_collected_files(&upload.root_label, upload.files, upload.skipped_files)
        .await
        .map(|summary| Json(SummaryResponse { summary }))
        .map_err(AppError::bad_request)
}

pub(crate) async fn save_markdown_summary_handler(
    Json(payload): Json<SaveSummaryPayload>,
) -> AppResult<StatusCode> {
    write_summary(&payload.summary, payload.path)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::bad_request)
}
