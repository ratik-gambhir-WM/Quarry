use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::{
    handler::{list_deal_data_room_handler, preview_deal_document_handler, DataRoomsHttpState},
    service::DataRoomService,
};

pub(crate) fn routes(data_rooms: Arc<DataRoomService>) -> Router {
    Router::new()
        .route(
            "/deals/{deal_id}/data-room",
            get(list_deal_data_room_handler),
        )
        .route(
            "/deals/{deal_id}/data-room/preview",
            post(preview_deal_document_handler),
        )
        .with_state(DataRoomsHttpState { data_rooms })
}
