use axum::{
    routing::{get, post},
    Router,
};

use super::handler::{greet_handler, login_demo_event_handler, login_demo_handler};

pub(crate) fn routes() -> Router {
    Router::new()
        .route("/greet", get(greet_handler))
        .route("/login-demo", post(login_demo_handler))
        .route("/login-demo/event", post(login_demo_event_handler))
}
