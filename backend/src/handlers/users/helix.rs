use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    core::models::user::UserNode,
    handlers::{AppError, AppState},
    services::user_service::{get_helix_user_by_email, save_helix_user},
};

#[derive(Debug, Deserialize)]
pub(crate) struct EmailQuery {
    pub email: String,
}

pub(crate) async fn save_helix_user_handler(
    State(state): State<AppState>,
    Json(input): Json<UserNode>,
) -> crate::errors::AppResult<(StatusCode, Json<serde_json::Value>)> {
    save_helix_user(&state, input)
        .await
        .map(|user| (StatusCode::CREATED, Json(user)))
        .map_err(AppError::bad_request)
}

pub(crate) async fn get_helix_user_by_email_handler(
    State(state): State<AppState>,
    Query(query): Query<EmailQuery>,
) -> crate::errors::AppResult<Json<serde_json::Value>> {
    get_helix_user_by_email(&state, &query.email)
        .await
        .map(Json)
        .map_err(AppError::bad_request)
}
