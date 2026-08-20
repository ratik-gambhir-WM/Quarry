use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::deal::{
        archive_deal_handler, create_deal_handler, create_deal_upload_handler,
        database_status_handler, extract_deal_handler, extract_deal_upload_handler,
        get_deal_handler, get_helix_deal_handler, list_deals_handler, save_helix_deal_handler,
    },
    state::AppState,
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/database/status", get(database_status_handler))
        .route("/deals", get(list_deals_handler).post(create_deal_handler))
        .route("/deals/upload", post(create_deal_upload_handler))
        .route("/deals/helix", post(save_helix_deal_handler))
        .route("/deals/helix/{deal_id}", get(get_helix_deal_handler))
        .route("/deals/{deal_id}", get(get_deal_handler))
        .route("/deals/{deal_id}/extraction", post(extract_deal_handler))
        .route(
            "/deals/{deal_id}/extraction/upload",
            post(extract_deal_upload_handler),
        )
        .route("/deals/{deal_id}/archive", post(archive_deal_handler))
}
