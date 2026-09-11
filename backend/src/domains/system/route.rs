use axum::{routing::get, Router};
use std::sync::Arc;

use super::{
    database_handler::{database_status_handler, SystemHttpState},
    handler::{capabilities_handler, health_handler},
    service::DatabaseService,
};

pub(crate) fn routes(database: Arc<DatabaseService>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/capabilities", get(capabilities_handler))
        .route("/database/status", get(database_status_handler))
        .with_state(SystemHttpState { database })
}
