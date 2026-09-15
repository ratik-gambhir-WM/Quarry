use std::sync::Arc;

use axum::{extract::DefaultBodyLimit, routing::post, Router};

use super::{handler, service::AssistantChatService};

const QUERY_BODY_LIMIT: usize = 54 * 1024 * 1024;

pub(crate) fn routes(assistant_chat: Arc<AssistantChatService>) -> Router {
    Router::new()
        .route("/query_model", post(handler::query_model_handler))
        .layer(DefaultBodyLimit::max(QUERY_BODY_LIMIT))
        .with_state(handler::AssistantChatHttpState { assistant_chat })
}
