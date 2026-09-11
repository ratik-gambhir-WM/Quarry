use axum::{routing::post, Router};
use std::sync::Arc;

use crate::domains::summaries::service::SummaryService;

use super::handler::{
    list_summary_files_handler, save_markdown_summary_handler, summarize_handler,
    summarize_selected_handler, summarize_upload_handler, SummariesHttpState,
};

pub(crate) fn routes(document_summaries: Arc<SummaryService>) -> Router {
    Router::new()
        .route("/summarize", post(summarize_handler))
        .route("/summarize/files", post(list_summary_files_handler))
        .route("/summarize/selected", post(summarize_selected_handler))
        .route("/summarize/upload", post(summarize_upload_handler))
        .route("/summaries/markdown", post(save_markdown_summary_handler))
        .with_state(SummariesHttpState { document_summaries })
}
