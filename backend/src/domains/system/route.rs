use super::handler::{capabilities_handler, health_handler};
use axum::{routing::get, Router};
pub(crate) fn routes() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/capabilities", get(capabilities_handler))
        .with_state(())
}
