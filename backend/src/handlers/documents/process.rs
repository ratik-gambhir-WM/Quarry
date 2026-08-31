use std::{convert::Infallible, path::Path as FilePath, time::Duration};

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::stream::{self, Stream};
use serde::Serialize;

use crate::{
    errors::AppResult,
    handlers::{AppError, AppState},
    services::{
        document_ingestion_service::{ProcessDocumentsResponse, UploadedDocument},
        document_service::{MAX_FILE_BYTES, MAX_TOTAL_REQUEST_FILE_BYTES},
    },
    utils::require_non_empty,
};

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
    let deal_id = normalize_required_request_value(deal_id, "dealId")?;
    let (user_id, files) = collect_document_upload(multipart).await?;
    state
        .document_ingestion
        .process(&deal_id, &user_id, files)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(crate) async fn start_process_file_handler(
    State(state): State<AppState>,
    Path(deal_id): Path<String>,
    multipart: Multipart,
) -> AppResult<(StatusCode, Json<ProcessFileJobResponse>)> {
    let deal_id = normalize_required_request_value(deal_id, "dealId")?;
    let (user_id, mut files) = collect_document_upload(multipart).await?;
    if files.len() != 1 {
        return Err(AppError::bad_request(
            "process_file accepts exactly one file per job",
        ));
    }

    let started = state
        .document_jobs
        .start(deal_id, user_id, files.remove(0))
        .await
        .map_err(AppError::from)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(ProcessFileJobResponse {
            job_id: started.job_id,
            filename: started.filename,
        }),
    ))
}

pub(crate) async fn process_document_job_events_handler(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let job_id = normalize_required_request_value(job_id, "jobId")?;
    let receiver = state
        .document_jobs
        .subscribe(&job_id)
        .await
        .map_err(AppError::from)?;

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
        validate_document_upload_filename(&filename)?;
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

    let user_id = normalize_required_request_value(user_id.unwrap_or_default(), "userId")?;
    if files.is_empty() {
        return Err(AppError::bad_request(
            "at least one PDF or DOCX upload is required",
        ));
    }
    Ok((user_id, files))
}

fn normalize_required_request_value(value: String, field: &str) -> AppResult<String> {
    require_non_empty(&value, field).map_err(AppError::bad_request)?;
    Ok(value.trim().to_string())
}

fn validate_document_upload_filename(filename: &str) -> AppResult<()> {
    if filename.trim() != filename {
        return Err(AppError::bad_request(
            "upload filenames must not contain surrounding whitespace",
        ));
    }
    let extension = FilePath::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("pdf" | "docx")) {
        return Err(AppError::bad_request(format!(
            "upload `{filename}` must be a PDF or DOCX file"
        )));
    }
    Ok(())
}
