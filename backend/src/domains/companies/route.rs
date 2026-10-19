use super::{
    handler::{create, list, update, CompanyState},
    service::CompanyService,
};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
pub(crate) fn routes(companies: Arc<CompanyService>) -> Router {
    Router::new()
        .route("/companies", get(list).post(create))
        .route("/companies/{company_id}/update", post(update))
        .with_state(CompanyState { companies })
}
