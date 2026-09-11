use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::{
    handler::{get_sqlite_user_by_email_handler, save_sqlite_user_handler, UsersHttpState},
    service::UserService,
};

pub(crate) fn routes(users: Arc<UserService>) -> Router {
    Router::new()
        .route("/users", post(save_sqlite_user_handler))
        .route("/users/by-email", get(get_sqlite_user_by_email_handler))
        .with_state(UsersHttpState { users })
}
