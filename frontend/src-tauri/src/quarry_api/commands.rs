use serde_json::Value;
use tauri::{ipc::Response, Emitter, State, WebviewWindow};

use crate::{
    errors::{AppError, AppResult},
    security::verify_main_window_origin,
};

use super::{models::MultipartRequest, service::QuarryApiService};

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

fn api_error(message: String) -> AppError {
    AppError::validation(message)
}
