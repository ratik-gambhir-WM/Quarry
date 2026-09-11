use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use super::upload::collect_summary_upload;
use crate::{
    app::http::error::{AppError, AppResult},
    domains::summaries::service::{SummarizableFile, SummaryService},
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct SummariesHttpState {
    pub document_summaries: Arc<SummaryService>,
}

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

pub(super) async fn list_summary_files_handler(
    State(state): State<SummariesHttpState>,
    Json(payload): Json<PathPayload>,
) -> AppResult<Json<Vec<SummarizableFile>>> {
    state
        .document_summaries
        .list_files(payload.path)
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn summarize_handler(
    State(state): State<SummariesHttpState>,
    Json(payload): Json<PathPayload>,
) -> AppResult<Json<SummaryResponse>> {
    state
        .document_summaries
        .summarize_dir(payload.path)
        .await
        .map(|summary| Json(SummaryResponse { summary }))
        .map_err(AppError::from)
}

pub(super) async fn summarize_selected_handler(
    State(state): State<SummariesHttpState>,
    Json(payload): Json<SelectedPathsPayload>,
) -> AppResult<Json<SummaryResponse>> {
    state
        .document_summaries
        .summarize_paths(payload.paths)
        .await
        .map(|summary| Json(SummaryResponse { summary }))
        .map_err(AppError::from)
}

pub(super) async fn summarize_upload_handler(
    State(state): State<SummariesHttpState>,
    multipart: Multipart,
) -> AppResult<Json<SummaryResponse>> {
    let upload = collect_summary_upload(multipart).await?;
    state
        .document_summaries
        .summarize_collected_files(&upload.root_label, upload.files, upload.skipped_files)
        .await
        .map(|summary| Json(SummaryResponse { summary }))
        .map_err(AppError::from)
}

pub(super) async fn save_markdown_summary_handler(
    State(state): State<SummariesHttpState>,
    Json(payload): Json<SaveSummaryPayload>,
) -> AppResult<StatusCode> {
    state
        .document_summaries
        .save_markdown(&payload.summary, payload.path)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}
