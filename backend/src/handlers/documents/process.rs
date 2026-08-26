use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::stream::{self, Stream};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    document_jobs::DocumentJobEvent,
    errors::AppResult,
    handlers::{AppError, AppState},
    services::{
        document_ingestion_service::{
            process_uploaded_documents, ProcessDocumentsResponse, UploadedDocument,
        },
        document_service::{MAX_FILE_BYTES, MAX_TOTAL_REQUEST_FILE_BYTES},
    },
};

const COMPLETED_JOB_RETENTION: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcessFileJobResponse {
    job_id: String,
    filename: String,
}

pub(crate) async fn process_documents_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
    multipart: Multipart,
) -> AppResult<Json<ProcessDocumentsResponse>> {
    let (user_id, files) = collect_document_upload(multipart).await?;
    process_uploaded_documents(&state, &deal_id, &user_id, files)
        .await
        .map(Json)
        .map_err(AppError::internal)
}

pub(crate) async fn start_process_file_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
    multipart: Multipart,
) -> AppResult<(StatusCode, Json<ProcessFileJobResponse>)> {
    let (user_id, mut files) = collect_document_upload(multipart).await?;
    if files.len() != 1 {
        return Err(AppError::bad_request(
            "process_file accepts exactly one file per job",
        ));
    }

    let file = files.remove(0);
    let filename = file.filename.clone();
    let job_id = Uuid::new_v4().to_string();
    state
        .create_document_job(DocumentJobEvent::processing(
            job_id.clone(),
            filename.clone(),
        ))
        .await;

    let worker_state = state.clone();
    let worker_deal_id = deal_id.clone();
    let worker_job_id = job_id.clone();
    let worker_filename = filename.clone();
    tokio::spawn(async move {
        let event =
            match process_uploaded_documents(&worker_state, &worker_deal_id, &user_id, vec![file])
                .await
            {
                Ok(response) => match response.documents.into_iter().next() {
                    Some(document) if document.skipped => DocumentJobEvent::skipped(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        document.document_id,
                    ),
                    Some(document) if document.success => DocumentJobEvent::completed(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        document.document_id,
                        document.chunk_count,
                    ),
                    Some(document) => DocumentJobEvent::failed(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        document
                            .error
                            .unwrap_or_else(|| "document processing failed".to_string()),
                    ),
                    None => DocumentJobEvent::failed(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        "document processing returned no result".to_string(),
                    ),
                },
                Err(error) => {
                    DocumentJobEvent::failed(worker_job_id.clone(), worker_filename.clone(), error)
                }
            };

        worker_state
            .update_document_job(&worker_job_id, event)
            .await;
        tokio::time::sleep(COMPLETED_JOB_RETENTION).await;
        worker_state.remove_document_job(&worker_job_id).await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(ProcessFileJobResponse { job_id, filename }),
    ))
}

pub(crate) async fn process_document_job_events_handler(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let receiver = state
        .subscribe_to_document_job(&job_id)
        .await
        .ok_or_else(|| AppError::not_found(format!("document job `{job_id}` was not found")))?;

    let events = stream::unfold(
        (receiver, true, false),
        |(mut receiver, initial, finished)| async move {
            if finished {
                return None;
            }
            if !initial && receiver.changed().await.is_err() {
                return None;
            }

            let status = receiver.borrow_and_update().clone();
            let finished = status.is_terminal();
            let event = Event::default()
                .event(status.event_name())
                .json_data(status)
                .expect("document job events are JSON serializable");
            Some((Ok(event), (receiver, false, finished)))
        },
    );

    Ok(Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

async fn collect_document_upload(
    mut multipart: Multipart,
) -> AppResult<(String, Vec<UploadedDocument>)> {
    let mut user_id = None;
    let mut files = Vec::new();
    let mut total_bytes = 0usize;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        AppError::bad_request(format!("failed to read multipart field: {error}"))
    })? {
        let name = field.name().unwrap_or_default().to_string();
        if matches!(name.as_str(), "userId" | "user_id") {
            let value = field.text().await.map_err(|error| {
                AppError::bad_request(format!("failed to read userId field: {error}"))
            })?;
            user_id = Some(value);
            continue;
        }
        if matches!(name.as_str(), "dealId" | "deal_id") {
            return Err(AppError::bad_request(
                "dealId must be supplied only by the request path",
            ));
        }
        if name != "files" {
            continue;
        }

        let filename = field
            .file_name()
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::bad_request("every file must include a filename"))?;
        let bytes = field.bytes().await.map_err(|error| {
            AppError::bad_request(format!("failed to read upload `{filename}`: {error}"))
        })?;
        if bytes.is_empty() {
            return Err(AppError::bad_request(format!(
                "upload `{filename}` is empty"
            )));
        }
        if bytes.len() > MAX_FILE_BYTES {
            return Err(AppError::bad_request(format!(
                "upload `{filename}` exceeds the 50 MB file limit"
            )));
        }
        total_bytes = total_bytes.checked_add(bytes.len()).ok_or_else(|| {
            AppError::bad_request("uploaded file sizes exceed the supported range")
        })?;
        if total_bytes > MAX_TOTAL_REQUEST_FILE_BYTES {
            return Err(AppError::bad_request(
                "uploaded files exceed the 50 MB request limit",
            ));
        }
        files.push(UploadedDocument {
            filename,
            bytes: bytes.to_vec(),
        });
    }

    let user_id = user_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("userId is required"))?;
    if files.is_empty() {
        return Err(AppError::bad_request(
            "at least one PDF or DOCX upload is required",
        ));
    }
    Ok((user_id, files))
}
