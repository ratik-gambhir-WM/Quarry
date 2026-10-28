use axum::{
    extract::{Multipart, State},
    Json,
};
use serde::Serialize;

use super::upload::collect_summary_upload;
use crate::{
    app::http::error::{AppError, AppResult},
    domains::summaries::service::SummaryService,
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct SummariesHttpState {
    pub document_summaries: Arc<SummaryService>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SummaryResponse {
    pub summary: String,
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
