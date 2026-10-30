use super::{handler::*, service::NotebookService};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get as route_get, post},
    Router,
};
use std::sync::Arc;
pub(crate) fn routes(service: Arc<NotebookService>) -> Router {
    Router::new()
        .route("/notebooks", route_get(list).post(create))
        .route(
            "/notebooks/{notebook_id}",
            route_get(get).patch(update).delete(delete),
        )
        .route(
            "/notebooks/{notebook_id}/folders",
            route_get(list_folders).post(create_folder),
        )
        .route(
            "/notebooks/{notebook_id}/folders/{folder_id}",
            route_get(get_folder)
                .patch(update_folder)
                .delete(delete_folder),
        )
        .route(
            "/notebooks/{notebook_id}/notes",
            route_get(list_notes).post(create_note),
        )
        .route(
            "/notebooks/{notebook_id}/notes/{note_id}",
            route_get(get_note).patch(update_note).delete(delete_note),
        )
        .route(
            "/notebooks/{notebook_id}/save-jobs/{job_id}",
            route_get(job),
        )
        .route(
            "/notebooks/{notebook_id}/save-jobs/{job_id}/retry",
            post(retry),
        )
        .route(
            "/notebooks/{notebook_id}/save-jobs/{job_id}/events",
            route_get(events),
        )
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .with_state(NotebookHttpState { service })
}
