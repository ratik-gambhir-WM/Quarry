use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::data_room::{list_deal_data_room_handler, preview_deal_document_handler},
    state::AppState,
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/deals/{deal_id}/data-room",
            get(list_deal_data_room_handler),
        )
        .route(
            "/deals/{deal_id}/data-room/preview",
            post(preview_deal_document_handler),
        )
}
