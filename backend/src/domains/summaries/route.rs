use super::{
    handler::{summarize_upload_handler, SummariesHttpState},
    service::SummaryService,
};
use axum::{routing::post, Router};
use std::sync::Arc;
pub(crate) fn routes(document_summaries: Arc<SummaryService>) -> Router {
    Router::new()
        .route("/summarize/upload", post(summarize_upload_handler))
        .with_state(SummariesHttpState { document_summaries })
}
