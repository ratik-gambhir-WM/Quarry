use std::path::Path;

use axum::extract::Multipart;

use crate::{
    core::infer_supported_mime_type,
    handlers::AppError,
    services::{
        deal_service::{SaveDealAndExtractInput, UploadedDealFile},
        document_service::{MAX_FILE_BYTES, MAX_TOTAL_REQUEST_FILE_BYTES},
    },
};

pub(crate) async fn collect_deal_upload(
    mut multipart: Multipart,
) -> crate::errors::AppResult<(SaveDealAndExtractInput, String, Vec<UploadedDealFile>)> {
    let mut input = None;
    let mut files = Vec::new();
    let mut root_label = "data-room".to_string();
    let mut total_bytes = 0usize;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::bad_request(format!("failed to read multipart field: {err}")))?
    {
        match field.name() {
            Some("input") => {
                let text = field.text().await.map_err(|err| {
                    AppError::bad_request(format!("failed to read deal input: {err}"))
                })?;
                input = Some(
                    serde_json::from_str::<SaveDealAndExtractInput>(&text).map_err(|err| {
                        AppError::bad_request(format!("invalid deal input JSON: {err}"))
                    })?,
                );
            }
            Some("files") => {
                let relative_path = field.file_name().unwrap_or("upload").to_string();
                if files.is_empty() {
                    root_label = relative_path
                        .split('/')
                        .next()
                        .filter(|part| !part.is_empty())
                        .unwrap_or("data-room")
                        .to_string();
                }
                let filename = Path::new(&relative_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("upload")
                    .to_string();
                let Some(mime_type) = infer_supported_mime_type(Path::new(&filename)) else {
                    continue;
                };
                let bytes = field.bytes().await.map_err(|err| {
                    AppError::bad_request(format!("failed to read upload {filename}: {err}"))
                })?;
                validate_upload_size(&filename, bytes.len(), total_bytes)?;
                total_bytes += bytes.len();
                files.push(UploadedDealFile {
                    relative_path,
                    filename,
                    mime_type: mime_type.to_string(),
                    bytes: bytes.to_vec(),
                });
            }
            _ => {}
        }
    }

    let input =
        input.ok_or_else(|| AppError::bad_request("multipart field `input` is required"))?;
    if files.is_empty() {
        return Err(AppError::bad_request(
            "at least one non-empty supported file upload is required",
        ));
    }
    Ok((input, root_label, files))
}

pub(crate) async fn collect_selected_deal_uploads(
    mut multipart: Multipart,
) -> crate::errors::AppResult<Vec<UploadedDealFile>> {
    let mut files = Vec::new();
    let mut total_bytes = 0usize;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::bad_request(format!("failed to read multipart field: {err}")))?
    {
        if field.name() != Some("files") {
            continue;
        }
        let relative_path = field.file_name().unwrap_or("upload").to_string();
        let filename = Path::new(&relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("upload")
            .to_string();
        let mime_type = infer_supported_mime_type(Path::new(&filename))
            .ok_or_else(|| AppError::bad_request(format!("unsupported file type: {filename}")))?;
        let bytes = field.bytes().await.map_err(|err| {
            AppError::bad_request(format!("failed to read upload {filename}: {err}"))
        })?;
        validate_upload_size(&filename, bytes.len(), total_bytes)?;
        total_bytes += bytes.len();
        files.push(UploadedDealFile {
            relative_path,
            filename,
            mime_type: mime_type.to_string(),
            bytes: bytes.to_vec(),
        });
    }
    Ok(files)
}

fn validate_upload_size(
    filename: &str,
    size: usize,
    total_before: usize,
) -> crate::errors::AppResult<()> {
    if size == 0 {
        return Err(AppError::bad_request(format!(
            "uploaded file is empty: {filename}"
        )));
    }
    if size > MAX_FILE_BYTES {
        return Err(AppError::bad_request(format!(
            "uploaded file exceeds 50 MB limit: {filename}"
        )));
    }
    if total_before + size > MAX_TOTAL_REQUEST_FILE_BYTES {
        return Err(AppError::bad_request(
            "uploaded files exceed the 50 MB total request limit",
        ));
    }
    Ok(())
}
