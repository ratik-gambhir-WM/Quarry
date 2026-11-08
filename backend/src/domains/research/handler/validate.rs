use super::super::service::{CreateIndexPayload, GraphRagQueryPayload};
use crate::app::http::error::{AppError, AppResult};

pub(super) fn validate_create_index_payload(payload: &CreateIndexPayload) -> AppResult<()> {
    if payload.files.is_empty() {
        return Err(AppError::bad_request(
            "at least one extracted file is required",
        ));
    }

    for file in &payload.files {
        if file.filename.trim().is_empty() {
            return Err(AppError::bad_request("filename cannot be empty"));
        }
        if file.text.trim().is_empty() {
            return Err(AppError::bad_request(format!(
                "extracted text cannot be empty for {}",
                file.filename
            )));
        }
    }

    Ok(())
}

pub(super) fn validate_graph_rag_query_payload(payload: &GraphRagQueryPayload) -> AppResult<()> {
    if payload.resource_id.trim().is_empty() {
        return Err(AppError::bad_request("resourceId cannot be empty"));
    }
    if payload.question.trim().is_empty() {
        return Err(AppError::bad_request("question cannot be empty"));
    }

    Ok(())
}

pub(super) fn validate_path_segment(value: &str, name: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::bad_request(format!("{name} cannot be empty")));
    }
    if matches!(value, "." | "..") {
        return Err(AppError::bad_request(format!(
            "{name} cannot be a dot path segment"
        )));
    }
    if value.contains('/') || value.contains('\\') {
        return Err(AppError::bad_request(format!(
            "{name} cannot contain path separators"
        )));
    }

    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/research/handler/validate_tests.rs"]
mod tests;
