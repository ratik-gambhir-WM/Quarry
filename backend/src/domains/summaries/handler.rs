pub(super) mod upload;

use axum::{
    extract::{Multipart, State},
    Json,
};
use serde::Serialize;

use crate::{
    app::http::error::{AppError, AppResult},
    domains::summaries::service::SummaryService,
};
use std::sync::Arc;
use upload::collect_summary_upload;

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
    if upload.files.is_empty() {
        return Err(AppError::bad_request(format!(
            "no supported files found in {}",
            upload.root_label
        )));
    }
    state
        .document_summaries
        .summarize_collected_files(&upload.root_label, upload.files, upload.skipped_files)
        .await
        .map(|summary| Json(SummaryResponse { summary }))
        .map_err(AppError::from)
}
