use super::{handler, service::DealDeletionService};
use axum::{
    routing::{delete, get},
    Router,
};
use std::sync::Arc;
pub(crate) fn routes(service: Arc<DealDeletionService>) -> Router {
    Router::new()
        .route("/deals/{deal_id}", delete(handler::delete))
        .route("/deal-deletions/{deletion_id}", get(handler::status))
        .with_state(service)
}
