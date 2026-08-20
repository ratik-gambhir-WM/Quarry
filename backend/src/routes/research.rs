use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::research::{
        create_index_handler, extract_files_handler, graph_rag_query_handler, index_status_handler,
        list_summary_files_handler, save_markdown_summary_handler, summarize_handler,
        summarize_selected_handler, summarize_upload_handler,
    },
    state::AppState,
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/files/extract", post(extract_files_handler))
        .route("/indexes", post(create_index_handler))
        .route("/indexes/{index_id}/status", get(index_status_handler))
        .route("/graphrag/query", post(graph_rag_query_handler))
        .route("/summarize", post(summarize_handler))
        .route("/summarize/files", post(list_summary_files_handler))
        .route("/summarize/selected", post(summarize_selected_handler))
        .route("/summarize/upload", post(summarize_upload_handler))
        .route("/summaries/markdown", post(save_markdown_summary_handler))
}
