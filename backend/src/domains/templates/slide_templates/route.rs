//! Relative feature routes bound before host composition.
use super::{
    handler::{self, EndpointState},
    SlideTemplates,
};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
pub const PREFIX: &str = "/api/v1/templates/slide-templates";
pub(crate) fn routes(studio: Arc<SlideTemplates>) -> Router {
    let routes = Router::new()
        .route(
            "/export",
            post(handler::export).layer(DefaultBodyLimit::max(
                super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES,
            )),
        )
        .route(
            "/export/insert",
            post(handler::insert).layer(DefaultBodyLimit::max(76 * 1024 * 1024)),
        )
        .route(
            "/import",
            post(handler::import).layer(DefaultBodyLimit::max(
                super::super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
            )),
        )
        .route(
            "/batch-import",
            post(handler::batch_import).layer(DefaultBodyLimit::max(
                super::super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
            )),
        )
        .route(
            "/import/classified",
            post(handler::import_classified).layer(DefaultBodyLimit::max(
                super::super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
            )),
        )
        .route("/catalog", get(handler::catalog))
        .route("/catalog/previews", get(handler::previews))
        .route("/catalog/{id}", get(handler::get).delete(handler::delete))
        .route("/catalog/{id}/preview", get(handler::preview))
        .route("/catalog/{id}/retrieval", get(handler::retrieval))
        .route("/import/{id}", get(handler::get))
        .route("/import/{id}/assets/{asset}", get(handler::asset))
        .with_state(EndpointState {
            studio: studio.clone(),
        })
        .layer(axum::middleware::from_fn_with_state(studio, admit));
    Router::new().nest("/templates/slide-templates", routes)
}

async fn admit(
    axum::extract::State(studio): axum::extract::State<Arc<SlideTemplates>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match studio.try_admit() {
        Ok(permit) => super::super::route::hold_admission(next.run(request).await, permit),
        Err(error) => error.into_response(),
    }
}
