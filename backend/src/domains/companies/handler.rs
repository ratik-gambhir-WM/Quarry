use validate::validate_company;
mod validate;
use super::{
    model::{Company, CompanyInput, CompanyPage, CompanyUpdateInput},
    service::CompanyService,
};
use crate::app::http::error::{AppError, AppResult};
use crate::domains::identity::model::AuthenticatedUser;
use axum::Extension;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
#[derive(Clone)]
pub(super) struct CompanyState {
    pub companies: Arc<CompanyService>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SearchInput {
    query: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}
pub(super) async fn list(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<CompanyState>,
    Query(input): Query<SearchInput>,
) -> AppResult<Json<CompanyPage>> {
    let limit = input.limit.unwrap_or(30);
    if !(1..=100).contains(&limit) || input.offset.unwrap_or(0) > 1_000_000 {
        return Err(AppError::bad_request("invalid company pagination"));
    }
    if let Some(query) = &input.query {
        if query.chars().count() > 200 || query.chars().any(char::is_control) {
            return Err(AppError::bad_request("invalid company query"));
        }
    }
    Ok(Json(
        state
            .companies
            .list(
                &actor.user.email,
                input.query,
                limit,
                input.offset.unwrap_or(0),
            )
            .await?,
    ))
}
pub(super) async fn create(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<CompanyState>,
    Json(mut input): Json<CompanyInput>,
) -> AppResult<(StatusCode, Json<Company>)> {
    input.user_email = actor.user.email;
    validate_company(&input)?;
    Ok((
        StatusCode::CREATED,
        Json(state.companies.save(None, input, None).await?),
    ))
}
pub(super) async fn update(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<CompanyState>,
    Path(id): Path<String>,
    Json(mut input): Json<CompanyUpdateInput>,
) -> AppResult<Json<Company>> {
    uuid::Uuid::parse_str(&id).map_err(|_| AppError::bad_request("invalid companyId"))?;
    validate::revision(input.expected_revision)?;
    input.fields.user_email = actor.user.email;
    validate_company(&input.fields)?;
    Ok(Json(
        state
            .companies
            .save(Some(id), input.fields, Some(input.expected_revision))
            .await?,
    ))
}
