use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::deal::{
        archive_deal_handler, create_deal_handler, database_status_handler, get_deal_handler,
        list_deals_handler, save_deal_metadata_handler,
    },
    state::AppState,
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/database/status", get(database_status_handler))
        .route("/deals", get(list_deals_handler).post(create_deal_handler))
        .route("/deals/{deal_id}", get(get_deal_handler))
        .route(
            "/deals/{deal_id}/metadata",
            post(save_deal_metadata_handler),
        )
        .route(
            "/deals/{deal_id}/extraction/upload",
            post(save_deal_metadata_handler),
        )
        .route("/deals/{deal_id}/archive", post(archive_deal_handler))
}
