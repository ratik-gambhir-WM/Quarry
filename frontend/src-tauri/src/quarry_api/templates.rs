//! Narrow raw-PPTX forwarding for the classified single-slide and deck imports.
use super::{client::QuarryHttpClient, models::MultipartRequest};
use base64::{engine::general_purpose, Engine as _};
use reqwest::header::HeaderMap;
use serde_json::{json, Value};

const SINGLE: &str = "/api/v1/templates/slide-templates/import/classified";
const BATCH: &str = "/api/v1/templates/slide-templates/batch-import/classified";
pub(super) const POWERPOINT_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";
const MAX_BYTES: usize = 25 * 1024 * 1024;
const MAX_ENCODED_BYTES: usize = MAX_BYTES.div_ceil(3) * 4;
const UNCERTAIN: &str = "Template import could not be confirmed. It may have completed; refresh templates before trying again.";

pub(super) fn import_mode(path: &str) -> Option<&'static str> {
    match path {
        SINGLE => Some("single"),
        BATCH => Some("batch"),
        _ => None,
    }
}

pub(super) async fn import(
    client: &QuarryHttpClient,
    request: MultipartRequest,
    mode: &str,
) -> Result<Value, String> {
    let path = request.path.clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || validate_upload(request))
        .await
        .map_err(|_| "Template import validation failed".to_owned())??;
    let response = client.post_pptx_stream(&path, bytes).await?;
    // Do not move the hydrated slide JSON through IPC; the headers confirm the saved write.
    acknowledgement(response.status().as_u16(), response.headers(), mode)
}

fn validate_upload(mut request: MultipartRequest) -> Result<Vec<u8>, String> {
    if import_mode(&request.path).is_none()
        || !request.fields.is_empty()
        || request.files.len() != 1
    {
        return Err(
            "Template import requires one PowerPoint file and an exact classified route".into(),
        );
    }
    let file = request.files.remove(0);
    if file.field_name != "files"
        || file.filename.trim().is_empty()
        || file.filename.len() > 255
        || file.filename.contains(['/', '\\'])
        || file.filename.chars().any(char::is_control)
        || !file.filename.to_ascii_lowercase().ends_with(".pptx")
        || file.mime_type != POWERPOINT_CONTENT_TYPE
        || file.data_base64.len() > MAX_ENCODED_BYTES
    {
        return Err("Template import file metadata or size is invalid".into());
    }
    let bytes = general_purpose::STANDARD
        .decode(file.data_base64)
        .map_err(|_| "Template import file data is invalid".to_owned())?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("Choose a non-empty PowerPoint file no larger than 25 MB".into());
    }
    Ok(bytes)
}

fn acknowledgement(status: u16, headers: &HeaderMap, mode: &str) -> Result<Value, String> {
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    if status != 201
        || header("content-type")
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            != Some("application/json")
    {
        return Err(UNCERTAIN.into());
    }
    let count = |name: &str, max: usize| -> Result<usize, String> {
        header(name)
            .filter(|value| !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|count| *count <= max)
            .ok_or_else(|| UNCERTAIN.into())
    };
    let warning_count = count("x-powerpoint-warning-count", 10_000)?;
    let imported_count = if mode == "single" {
        if header("x-template-preview-status") != Some("ready")
            || !header("x-template-id").is_some_and(|id| {
                !id.is_empty()
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            })
        {
            return Err(UNCERTAIN.into());
        }
        1
    } else {
        let count = count("x-imported-template-count", 100)?;
        if count == 0 {
            return Err(UNCERTAIN.into());
        }
        count
    };
    Ok(json!({"importMode": mode, "importedCount": imported_count, "warningCount": warning_count}))
}

#[cfg(test)]
#[path = "../../tests/quarry_api/template_import_tests.rs"]
mod tests;
