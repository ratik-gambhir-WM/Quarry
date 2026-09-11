use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    app::http::error::{AppError, AppResult},
    domains::users::service::{AddUserInput, User, UserService},
};

#[derive(Clone)]
pub(super) struct UsersHttpState {
    pub users: Arc<UserService>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EmailQuery {
    pub email: String,
}

pub(super) async fn save_sqlite_user_handler(
    State(state): State<UsersHttpState>,
    Json(input): Json<AddUserInput>,
) -> AppResult<(StatusCode, Json<User>)> {
    validate_user_input(&input)?;
    state
        .users
        .create(input)
        .await
        .map(|user| (StatusCode::CREATED, Json(user)))
        .map_err(AppError::from)
}

pub(super) async fn get_sqlite_user_by_email_handler(
    State(state): State<UsersHttpState>,
    Query(query): Query<EmailQuery>,
) -> AppResult<Json<User>> {
    let email = validate_email(&query.email)?;
    state
        .users
        .by_email(email)
        .await
        .map_err(AppError::from)?
        .map(Json)
        .ok_or_else(|| AppError::not_found(format!("user not found for email `{email}`")))
}

fn validate_user_input(input: &AddUserInput) -> AppResult<()> {
    let required = [
        ("first_name", input.first_name.as_str()),
        ("last_name", input.last_name.as_str()),
        ("email", input.email.as_str()),
        ("api_key", input.api_key.as_str()),
        ("role", input.role.as_str()),
    ];
    if let Some((field, _)) = required.iter().find(|(_, value)| value.trim().is_empty()) {
        return Err(AppError::bad_request(format!("{field} is required")));
    }
    Ok(())
}

fn validate_email(email: &str) -> AppResult<&str> {
    let email = email.trim();
    if email.is_empty() {
        return Err(AppError::bad_request("email is required"));
    }
    Ok(email)
}
