//! Template HTTP routes: compact product endpoints and the full slide-template API.
use super::{
    handler,
    slide_templates::{handler as slide_handler, SlideTemplates},
};
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

pub(super) const SLIDE_TEMPLATE_PREFIX: &str = "/api/v1/templates/slide-templates";

const PPTX_TEMPLATE_IMPORT_BODY_BYTES: usize = 26 * 1024 * 1024;

pub(crate) fn routes(templates: Arc<TemplateService>) -> Router {
    let native = templates
        .studio()
        .map(|studio| slide_template_routes(Arc::clone(studio)))
        .unwrap_or_default();
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
        .layer(axum::middleware::from_fn_with_state(
            templates,
            handler::auth::admit,
        ))
        .merge(native)
}

fn slide_template_routes(studio: Arc<SlideTemplates>) -> Router {
    let routes = Router::new()
        .route(
            "/export",
            post(slide_handler::export).layer(DefaultBodyLimit::max(
                super::model::MAX_TEMPLATE_DOCUMENT_BYTES,
            )),
        )
        .route(
            "/export/insert",
            post(slide_handler::insert).layer(DefaultBodyLimit::max(76 * 1024 * 1024)),
        )
        .route(
            "/import",
            post(slide_handler::import).layer(DefaultBodyLimit::max(
                super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
            )),
        )
        .route(
            "/batch-import",
            post(slide_handler::batch_import).layer(DefaultBodyLimit::max(
                super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
            )),
        )
        .route(
            "/batch-import/classified",
            post(slide_handler::batch_import_classified).layer(DefaultBodyLimit::max(
                super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
            )),
        )
        .route(
            "/import/classified",
            post(slide_handler::import_classified).layer(DefaultBodyLimit::max(
                super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
            )),
        )
        .route("/catalog", get(slide_handler::catalog))
        .route("/catalog/previews", get(slide_handler::previews))
        .route(
            "/catalog/{id}",
            get(slide_handler::get).delete(slide_handler::delete),
        )
        .route("/catalog/{id}/preview", get(slide_handler::preview))
        .route("/catalog/{id}/retrieval", get(slide_handler::retrieval))
        .route(
            "/catalog/{id}/retrieval/retry",
            post(slide_handler::retry_processing).layer(DefaultBodyLimit::max(1024)),
        )
        .route("/import/{id}", get(slide_handler::get))
        .route("/import/{id}/assets/{asset}", get(slide_handler::asset))
        .with_state(slide_handler::EndpointState {
            studio: studio.clone(),
        })
        .layer(axum::middleware::from_fn_with_state(
            studio,
            slide_handler::auth::admit,
        ));
    Router::new().nest("/templates/slide-templates", routes)
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/templates/route_tests.rs"]
mod tests;
