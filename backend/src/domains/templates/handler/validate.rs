use super::super::model::MAX_TEMPLATE_ID_BYTES;
use crate::app::http::error::{AppError, AppResult};
use std::path::Path as FilePath;

pub(super) fn validate_template_id(template_id: &str) -> AppResult<()> {
    if template_id.trim().is_empty()
        || template_id.len() > MAX_TEMPLATE_ID_BYTES
        || template_id.chars().any(char::is_control)
    {
        return Err(AppError::bad_request("template ID is invalid"));
    }
    Ok(())
}

pub(super) fn validate_presentation_document(document: &serde_json::Value) -> AppResult<()> {
    if !document
        .as_object()
        .and_then(|root| root.get("presentation"))
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(AppError::bad_request(
            "presentation document is missing a presentation object",
        ));
    }
    Ok(())
}

pub(super) fn validate_pptx_template_filename(filename: &str) -> AppResult<()> {
    let is_path_like = filename.trim() != filename
        || filename.contains(['/', '\\'])
        || filename.chars().any(char::is_control)
        || matches!(filename, "." | "..");
    let has_pptx_extension = FilePath::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pptx"));
    if is_path_like || !has_pptx_extension {
        return Err(AppError::bad_request(
            "the template filename must be a safe .pptx name",
        ));
    }
    Ok(())
}
