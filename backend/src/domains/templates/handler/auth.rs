use super::{TemplateService, TemplatesHttpState};
use std::sync::Arc;

pub(crate) async fn admit(
    axum::extract::State(templates): axum::extract::State<Arc<TemplateService>>,
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
    let scoped = match templates.for_user(actor.user.id) {
        Ok(scoped) => scoped,
        Err(error) => return crate::app::http::error::AppError::from(error).into_response(),
    };
    request.extensions_mut().insert(TemplatesHttpState {
        templates: Arc::new(scoped),
    });
    match templates.try_admit() {
        Ok(Some(permit)) => hold_admission(next.run(request).await, permit),
        Ok(None) => next.run(request).await,
        Err(error) => crate::app::http::error::AppError::from(error).into_response(),
    }
}
/// Retain admission until the response body is consumed or dropped, including slow clients.
pub(crate) fn hold_admission(
    response: axum::response::Response,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> axum::response::Response {
    use futures_util::StreamExt;
    let (parts, body) = response.into_parts();
    let stream =
        futures_util::stream::unfold(
            (body.into_data_stream(), permit),
            |(mut stream, permit)| async move {
                stream.next().await.map(|chunk| (chunk, (stream, permit)))
            },
        )
        .fuse();
    axum::response::Response::from_parts(parts, axum::body::Body::from_stream(stream))
}
