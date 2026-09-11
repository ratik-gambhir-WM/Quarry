pub mod error;
mod middleware;

use axum::Router;

use crate::app::config::HttpConfig;

pub fn create_router(api: Router, config: &HttpConfig) -> Router {
    let router = Router::new().nest("/api", api.clone()).nest("/api/v1", api);
    middleware::apply(router, config)
}
