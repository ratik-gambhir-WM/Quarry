use std::path::Path;

use axum::extract::Multipart;

use crate::{
    app::http::error::{AppError, AppResult},
    domains::deals::service::{SaveDealMetadataInput, UploadedDealFile},
    shared::file_policy::{
        infer_supported_mime_type, MAX_FILE_BYTES, MAX_TOTAL_REQUEST_FILE_BYTES,
    },
};

pub(crate) async fn collect_deal_metadata_input(
    mut multipart: Multipart,
) -> AppResult<SaveDealMetadataInput> {
    let mut files = Vec::new();
    let mut sharepoint_link = None;
    let mut sow_link = None;
    let mut fact_sheet_link = None;
    let mut rl_link = None;
    let mut total_bytes = 0usize;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::bad_request(format!("failed to read multipart field: {err}")))?
    {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name != "files" {
            let target = match field_name.as_str() {
                "sharepointLink" => &mut sharepoint_link,
                "sowLink" => &mut sow_link,
                "factSheetLink" => &mut fact_sheet_link,
                "rlLink" => &mut rl_link,
                _ => continue,
            };
            if target.is_some() {
                return Err(AppError::bad_request(format!(
                    "duplicate multipart field: {field_name}"
                )));
            }
            let value = field.text().await.map_err(|err| {
                AppError::bad_request(format!("failed to read {field_name}: {err}"))
            })?;
            if value.len() > 4096 {
                return Err(AppError::bad_request(format!(
                    "{field_name} exceeds the 4096 character limit"
                )));
            }
            *target = Some(value);
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
    Ok(SaveDealMetadataInput {
        uploaded_files: files,
        sharepoint_link,
        sow_link,
        fact_sheet_link,
        rl_link,
    })
}

fn validate_upload_size(filename: &str, size: usize, total_before: usize) -> AppResult<()> {
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
