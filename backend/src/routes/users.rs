use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::users::{
        get_sqlite_user_by_email_handler, greet_handler, login_demo_event_handler,
        login_demo_handler, save_sqlite_user_handler,
    },
    state::AppState,
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/greet", get(greet_handler))
        .route("/users", post(save_sqlite_user_handler))
        .route("/users/by-email", get(get_sqlite_user_by_email_handler))
        .route("/login-demo", post(login_demo_handler))
        .route("/login-demo/event", post(login_demo_event_handler))
}
