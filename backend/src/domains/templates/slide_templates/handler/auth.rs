use super::{EndpointState, SlideTemplates};
use std::sync::Arc;

pub(crate) async fn admit(
    axum::extract::State(studio): axum::extract::State<Arc<SlideTemplates>>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let Some(actor) = request
        .extensions()
        .get::<crate::domains::identity::model::AuthenticatedUser>()
    else {
        return crate::app::http::error::AppError::Unauthorized.into_response();
    };
    let scoped = match studio.for_user(actor.user.id) {
        Ok(scoped) => scoped,
        Err(error) => return error.into_response(),
    };
    request.extensions_mut().insert(EndpointState {
        studio: Arc::new(scoped),
    });
    match studio.try_admit() {
        Ok(permit) => {
            super::super::super::handler::auth::hold_admission(next.run(request).await, permit)
        }
        Err(error) => error.into_response(),
    }
}
