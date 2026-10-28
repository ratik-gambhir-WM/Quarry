use super::{handler, service::IdentityService};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
pub(crate) fn public(identity: Arc<IdentityService>) -> Router {
    Router::new()
        .route("/auth/register", post(handler::register))
        .route("/auth/login", post(handler::login))
        .layer(DefaultBodyLimit::max(8192))
        .with_state(identity)
}
pub(crate) fn protected(identity: Arc<IdentityService>) -> Router {
    Router::new()
        .route("/auth/me", get(handler::me))
        .route("/auth/logout", post(handler::logout))
        .with_state(identity)
}
