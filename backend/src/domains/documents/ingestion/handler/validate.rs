use crate::app::http::error::{AppError, AppResult};
use crate::shared::ids::require_non_empty;
use std::path::Path as FilePath;

pub(super) fn normalize_required_request_value(value: String, field: &str) -> AppResult<String> {
    require_non_empty(&value, field).map_err(AppError::bad_request)?;
    Ok(value.trim().to_string())
}

pub(super) fn validate_document_upload_filename(filename: &str) -> AppResult<()> {
    if filename.trim() != filename {
        return Err(AppError::bad_request(
            "upload filenames must not contain surrounding whitespace",
        ));
    }
    let extension = FilePath::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(
        extension.as_deref(),
        Some("pdf" | "docx" | "xlsx" | "pptx" | "png" | "jpg" | "jpeg" | "webp" | "gif")
    ) {
        return Err(AppError::bad_request(format!(
            "upload `{filename}` must be a PDF, DOCX, XLSX, PPTX, PNG, JPEG, WebP, or GIF file"
        )));
    }
    Ok(())
}
