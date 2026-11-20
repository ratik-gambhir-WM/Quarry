//! HTTP boundary for explicit (re)classification. Authentication and the initial Upload
//! capability are enforced by the auth middleware; the repository rechecks them at the
//! protected mutation.
use std::sync::Arc;

use axum::{
    extract::{rejection::JsonRejection, Path, State},
    Extension, Json,
};
use serde::Deserialize;

use super::{model::DocumentClassification, service::ClassificationService};
use crate::{
    app::http::error::{AppError, AppResult},
    domains::identity::model::AuthenticatedUser,
};
use validate::{validate_expected_generation, validate_file_id};
mod validate;

#[derive(Clone)]
pub(super) struct ClassificationHttpState {
    pub classification: Arc<ClassificationService>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ClassificationRequest {
    expected_generation: i64,
}

pub(super) async fn request_classification_handler(
    State(state): State<ClassificationHttpState>,
    Extension(actor): Extension<AuthenticatedUser>,
    Path((deal_id, file_id)): Path<(String, String)>,
    input: Result<Json<ClassificationRequest>, JsonRejection>,
) -> AppResult<Json<DocumentClassification>> {
    validate_file_id(&file_id)?;
    let Json(input) = input.map_err(|_| AppError::bad_request("invalid classification request"))?;
    validate_expected_generation(input.expected_generation)?;
    // Synchronous: the provider call runs in this request under the shared request bound.
    let classification = state
        .classification
        .request(&deal_id, &file_id, actor.user.id, input.expected_generation)
        .await?;
    Ok(Json(classification))
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/classification/handler_tests.rs"]
mod tests;
