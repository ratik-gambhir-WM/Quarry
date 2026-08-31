mod data_room;
mod deal;
mod documents;
mod research;
mod system;
mod users;

use axum::{
    http::{header, HeaderName, Method, StatusCode},
    Router,
};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{config::HttpConfig, state::AppState};

pub fn create_router(state: AppState, config: &HttpConfig) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(config.cors_origins.clone()))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, request_id_header.clone()])
        .expose_headers([request_id_header.clone()]);

    let api = Router::new()
        .merge(system::routes())
        .merge(users::routes())
        .merge(deal::routes())
        .merge(documents::routes())
        .merge(data_room::routes())
        .merge(research::routes());

    Router::new()
        .nest("/api", api.clone())
        .nest("/api/v1", api)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            config.request_timeout,
        ))
        .layer(cors)
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .with_state(state)
}

#[cfg(test)]
#[path = "../../tests/routes/mod_tests.rs"]
mod tests;
