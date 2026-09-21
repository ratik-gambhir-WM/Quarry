use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};

use super::{handler, service::AssistantChatService};

const QUERY_BODY_LIMIT: usize = 54 * 1024 * 1024;

pub(crate) fn routes(assistant_chat: Arc<AssistantChatService>) -> Router {
    Router::new()
        .route("/query_model", post(handler::query_model_handler))
        .route(
            "/assistant/threads",
            get(handler::list_threads_handler).post(handler::create_thread_handler),
        )
        .route(
            "/assistant/threads/{thread_id}",
            get(handler::get_thread_handler).delete(handler::delete_thread_handler),
        )
        .route(
            "/assistant/threads/{thread_id}/rename",
            post(handler::rename_thread_handler),
        )
        .route(
            "/assistant/threads/{thread_id}/archive",
            post(handler::archive_thread_handler),
        )
        .route(
            "/assistant/threads/{thread_id}/unarchive",
            post(handler::unarchive_thread_handler),
        )
        .route(
            "/assistant/threads/{thread_id}/runs",
            post(handler::run_thread_handler),
        )
        .layer(DefaultBodyLimit::max(QUERY_BODY_LIMIT))
        .with_state(handler::AssistantChatHttpState { assistant_chat })
}
