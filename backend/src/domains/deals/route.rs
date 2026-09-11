use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::{
    extraction_handler::save_deal_metadata_handler,
    handler::{
        archive_deal_handler, create_deal_handler, get_deal_handler, list_deals_handler,
        DealsHttpState,
    },
    service::DealService,
};

pub(crate) fn routes(deals: Arc<DealService>) -> Router {
    Router::new()
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
        .with_state(DealsHttpState { deals })
}
