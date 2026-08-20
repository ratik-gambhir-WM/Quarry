use axum::{routing::get, Router};

use crate::{
    handlers::system::{capabilities_handler, health_handler},
    state::AppState,
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_handler))
        .route("/capabilities", get(capabilities_handler))
}
