use super::{model::DeletionStatus, service::DealDeletionService};
use crate::{
    app::http::error::{AppError, AppResult},
    domains::identity::model::AuthenticatedUser,
};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use std::sync::Arc;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DeleteRevision {
    expected_revision: i64,
}

pub(super) async fn delete(
    State(service): State<Arc<DealDeletionService>>,
    Extension(actor): Extension<AuthenticatedUser>,
    Path(deal): Path<String>,
    Query(input): Query<DeleteRevision>,
) -> AppResult<Response> {
    crate::domains::deals::handler::validate_deal_id(&deal)?;
    if !(1..=crate::shared::revision::MAX_REVISION).contains(&input.expected_revision) {
        return Err(AppError::bad_request("invalid expectedRevision"));
    }
    service.authorize(deal.clone(), actor.user.id).await?;
    let op = service
        .accept(deal, actor.user.id, input.expected_revision)
        .await?;
    if op.status == DeletionStatus::Completed {
        return Ok((
            StatusCode::NO_CONTENT,
            [(header::CACHE_CONTROL, "no-store")],
        )
            .into_response());
    }
    Ok((
        StatusCode::ACCEPTED,
        [
            (header::LOCATION, op.status_url.clone()),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        Json(op),
    )
        .into_response())
}
pub(super) async fn status(
    State(service): State<Arc<DealDeletionService>>,
    Extension(actor): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err(AppError::bad_request("invalid deletionId"));
    }
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(service.status(id, actor.user.id).await?),
    )
        .into_response())
}
