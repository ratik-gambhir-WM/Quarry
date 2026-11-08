use crate::app::http::error::{AppError, AppResult};

pub(super) fn preview_path(value: &str) -> AppResult<()> {
    super::super::validate::relative_document_path(value)
        .map(|_| ())
        .map_err(AppError::bad_request)
}
