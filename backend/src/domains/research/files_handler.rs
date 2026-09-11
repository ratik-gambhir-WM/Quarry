use axum::{
    extract::{Multipart, State},
    Json,
};

use super::{service::ResearchService, upload::collect_wm_upload_files};
use crate::{
    app::http::error::{AppError, AppResult},
    domains::research::service::FileExtractResponse,
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct ResearchHttpState {
    pub research: Arc<ResearchService>,
}

pub(super) async fn extract_files_handler(
    State(state): State<ResearchHttpState>,
    multipart: Multipart,
) -> AppResult<Json<FileExtractResponse>> {
    let files = collect_wm_upload_files(multipart).await?;
    state
        .research
        .extract_files(files)
        .await
        .map(Json)
        .map_err(AppError::from)
}
