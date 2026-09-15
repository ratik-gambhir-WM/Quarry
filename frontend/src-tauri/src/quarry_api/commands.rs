use serde_json::Value;
use std::sync::Arc;
use tauri::{ipc::Response, Emitter, State, WebviewWindow};

use crate::{
    errors::{AppError, AppResult},
    security::verify_main_window_origin,
};

use super::{
    models::{MultipartRequest, PowerPointExportPayload, QueryEventPayload, QueryStreamRequest},
    service::{QuarryApiService, QuerySubscriptions},
};

#[tauri::command]
pub async fn quarry_api_delete(
    window: WebviewWindow,
    service: State<'_, QuarryApiService>,
    path: String,
) -> AppResult<()> {
    verify_main_window_origin(&window)?;
    service.delete(&path).await.map_err(api_error)
}

#[tauri::command]
pub async fn quarry_api_get(
    window: WebviewWindow,
    service: State<'_, QuarryApiService>,
    path: String,
) -> AppResult<Value> {
    verify_main_window_origin(&window)?;
    service.get(&path).await.map_err(api_error)
}

#[tauri::command]
pub async fn quarry_api_get_pdf(
    window: WebviewWindow,
    service: State<'_, QuarryApiService>,
    path: String,
) -> AppResult<Response> {
    verify_main_window_origin(&window)?;
    service
        .get_pdf(&path)
        .await
        .map(Response::new)
        .map_err(api_error)
}

#[tauri::command]
pub async fn quarry_api_post(
    window: WebviewWindow,
    service: State<'_, QuarryApiService>,
    path: String,
    body: Value,
) -> AppResult<Value> {
    verify_main_window_origin(&window)?;
    service.post(&path, body).await.map_err(api_error)
}

#[tauri::command]
pub async fn quarry_api_post_powerpoint(
    window: WebviewWindow,
    service: State<'_, QuarryApiService>,
    path: String,
    body: Value,
) -> AppResult<PowerPointExportPayload> {
    verify_main_window_origin(&window)?;
    service
        .post_powerpoint(&path, body)
        .await
        .map_err(api_error)
}

#[tauri::command]
pub async fn quarry_api_post_multipart(
    window: WebviewWindow,
    service: State<'_, QuarryApiService>,
    request: MultipartRequest,
) -> AppResult<Value> {
    verify_main_window_origin(&window)?;
    service.post_multipart(request).await.map_err(api_error)
}

#[tauri::command]
pub async fn subscribe_document_job(
    window: WebviewWindow,
    service: State<'_, QuarryApiService>,
    job_id: String,
    subscription_id: String,
) -> AppResult<()> {
    verify_main_window_origin(&window)?;
    let event_window = window.clone();
    service
        .document_job_events(&job_id, &subscription_id, move |payload| {
            event_window
                .emit("quarry-document-job-event", payload)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(api_error)
}

#[tauri::command]
pub async fn send_query_stream(
    window: WebviewWindow,
    service: State<'_, QuarryApiService>,
    subscriptions: State<'_, Arc<QuerySubscriptions>>,
    request: QueryStreamRequest,
    subscription_id: String,
) -> AppResult<()> {
    verify_main_window_origin(&window)?;
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    subscriptions
        .register(subscription_id.clone(), cancel_tx)
        .map_err(api_error)?;
    let service = service.inner().clone();
    let subscriptions = Arc::clone(subscriptions.inner());
    let event_window = window.clone();
    tauri::async_runtime::spawn(async move {
        let result = service
            .query_events(request, &subscription_id, cancel_rx, |payload| {
                event_window
                    .emit("quarry-query-event", payload)
                    .map_err(|error| error.to_string())
            })
            .await;
        if let Err(error) = result {
            let _ = event_window.emit(
                "quarry-query-event",
                QueryEventPayload::ConnectionError {
                    message: error,
                    subscription_id: subscription_id.clone(),
                },
            );
        }
        subscriptions.remove(&subscription_id);
    });
    Ok(())
}

#[tauri::command]
pub async fn cancel_query_stream(
    window: WebviewWindow,
    subscriptions: State<'_, Arc<QuerySubscriptions>>,
    subscription_id: String,
) -> AppResult<()> {
    verify_main_window_origin(&window)?;
    subscriptions.cancel(&subscription_id).map_err(api_error)
}

fn api_error(message: String) -> AppError {
    AppError::validation(message)
}
