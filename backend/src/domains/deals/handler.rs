pub(super) use validate::{
    validate_deal_id, validate_deal_input, validate_fields, validate_https_link,
    validate_sharepoint_link,
};
pub(super) mod upload;
mod validate;

use crate::domains::identity::model::AuthenticatedUser;
use axum::Extension;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use crate::{
    app::http::error::{AppError, AppResult},
    domains::deals::service::{Deal, DealFields, DealService, DealWithMetadata, SaveDealInput},
};

#[derive(Clone)]
pub(super) struct DealsHttpState {
    pub deals: Arc<DealService>,
}

pub(super) async fn list_deals_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<DealsHttpState>,
) -> AppResult<Json<Vec<DealWithMetadata>>> {
    state
        .deals
        .list_owned(actor.user.id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn get_deal_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
) -> crate::app::http::error::AppResult<Json<DealWithMetadata>> {
    validate_deal_id(&deal_id)?;
    state
        .deals
        .get(&deal_id, actor.user.id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn create_deal_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<DealsHttpState>,
    input: Result<Json<SaveDealInput>, axum::extract::rejection::JsonRejection>,
) -> AppResult<Response> {
    let mut input = match input {
        Ok(Json(input)) => input,
        Err(rejection) => return Ok(deal_json_rejection(rejection)),
    };
    input.user_email = actor.user.email;
    validate_deal_input(&input)?;
    state
        .deals
        .create(input)
        .await
        .map(|response| (StatusCode::CREATED, Json(response)).into_response())
        .map_err(AppError::from)
}

fn deal_json_rejection(rejection: axum::extract::rejection::JsonRejection) -> Response {
    use axum::extract::rejection::JsonRejection;
    match rejection {
        JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_) => {
            AppError::bad_request("invalid deal input").into_response()
        }
        other => other.into_response(),
    }
}

pub(super) async fn update_deal_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<DealsHttpState>,
    Path(id): Path<String>,
    fields: Result<Json<super::model::UpdateDealInput>, axum::extract::rejection::JsonRejection>,
) -> AppResult<Response> {
    validate_deal_id(&id)?;
    let fields = match fields {
        Ok(Json(fields)) => fields,
        Err(rejection) => return Ok(deal_json_rejection(rejection)),
    };
    validate::revision(fields.expected_revision)?;
    validate_fields(&fields.fields)?;
    Ok(Json(
        state
            .deals
            .update(&id, actor.user.id, fields.fields, fields.expected_revision)
            .await?,
    )
    .into_response())
}

pub(super) async fn archive_deal_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
) -> crate::app::http::error::AppResult<Json<Deal>> {
    validate_deal_id(&deal_id)?;
    state
        .deals
        .archive(&deal_id, actor.user.id)
        .await
        .map(Json)
        .map_err(AppError::from)
}
