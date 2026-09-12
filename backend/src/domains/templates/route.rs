use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
    Router,
};

use super::{
    handler::{
        delete_template_handler, import_pptx_template_handler, list_template_previews_handler,
        TemplatesHttpState,
    },
    service::TemplateService,
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
        .route("/templates/{template_id}", delete(delete_template_handler))
        .with_state(TemplatesHttpState { templates })
}
