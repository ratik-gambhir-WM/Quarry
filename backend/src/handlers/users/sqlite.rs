use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    handlers::{run_blocking, AppError, AppState},
    services::user_service::{get_sqlite_user_by_email, save_sqlite_user, AddUserInput, User},
};

#[derive(Debug, Deserialize)]
pub(crate) struct EmailQuery {
    pub email: String,
}

pub(crate) async fn save_sqlite_user_handler(
    State(state): State<AppState>,
    Json(input): Json<AddUserInput>,
) -> crate::errors::AppResult<(StatusCode, Json<User>)> {
    run_blocking(move || save_sqlite_user(&state, input))
        .await
        .map(|user| (StatusCode::CREATED, Json(user)))
        .map_err(AppError::bad_request)
}

pub(crate) async fn get_sqlite_user_by_email_handler(
    State(state): State<AppState>,
    Query(query): Query<EmailQuery>,
) -> crate::errors::AppResult<Json<User>> {
    let email = query.email;
    let missing_email = email.clone();
    run_blocking(move || get_sqlite_user_by_email(&state, &email))
        .await
        .map_err(AppError::bad_request)?
        .map(Json)
        .ok_or_else(|| AppError::not_found(format!("user not found for email `{missing_email}`")))
}
