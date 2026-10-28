use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};

use super::{
    handler::{
        delete_template_handler, export_powerpoint_handler, get_template_handler,
        import_pptx_template_handler, list_template_previews_handler, TemplatesHttpState,
    },
    service::{TemplateService, MAX_TEMPLATE_DOCUMENT_BYTES},
};

const PPTX_TEMPLATE_IMPORT_BODY_BYTES: usize = 26 * 1024 * 1024;

pub(crate) fn routes(templates: Arc<TemplateService>) -> Router {
    Router::new()
        .route("/templates/previews", get(list_template_previews_handler))
        .route(
            "/templates/import",
            post(import_pptx_template_handler)
                .layer(DefaultBodyLimit::max(PPTX_TEMPLATE_IMPORT_BODY_BYTES)),
        )
        .route(
            "/templates/export",
            post(export_powerpoint_handler)
                .layer(DefaultBodyLimit::max(MAX_TEMPLATE_DOCUMENT_BYTES)),
        )
        .route(
            "/templates/{template_id}",
            get(get_template_handler).delete(delete_template_handler),
        )
        .with_state(TemplatesHttpState {
            templates: templates.clone(),
        })
        .layer(axum::middleware::from_fn_with_state(templates, admit))
}

async fn admit(
    axum::extract::State(templates): axum::extract::State<Arc<TemplateService>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match templates.try_admit() {
        Ok(Some(permit)) => hold_admission(next.run(request).await, permit),
        Ok(None) => next.run(request).await,
        Err(error) => crate::app::http::error::AppError::from(error).into_response(),
    }
}
/// Retain admission until the response body is consumed or dropped, including slow clients.
pub(super) fn hold_admission(
    response: axum::response::Response,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> axum::response::Response {
    use futures_util::StreamExt;
    let (parts, body) = response.into_parts();
    let stream =
        futures_util::stream::unfold(
            (body.into_data_stream(), permit),
            |(mut stream, permit)| async move {
                stream.next().await.map(|chunk| (chunk, (stream, permit)))
            },
        )
        .fuse();
    axum::response::Response::from_parts(parts, axum::body::Body::from_stream(stream))
}
