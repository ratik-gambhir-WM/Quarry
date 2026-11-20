use axum::{extract::DefaultBodyLimit, routing::post, Router};
use std::sync::Arc;

use super::{
    handler::{request_classification_handler, ClassificationHttpState},
    service::ClassificationService,
};

/// The request body is a single small JSON object.
const CLASSIFICATION_BODY_BYTES: usize = 1024;

pub(crate) fn routes(classification: Arc<ClassificationService>) -> Router {
    Router::new()
        .route(
            "/deals/{deal_id}/documents/{file_id}/classification",
            post(request_classification_handler)
                .layer(DefaultBodyLimit::max(CLASSIFICATION_BODY_BYTES)),
        )
        .with_state(ClassificationHttpState { classification })
}
