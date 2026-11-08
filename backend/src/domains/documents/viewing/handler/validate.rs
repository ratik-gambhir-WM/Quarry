use crate::app::http::error::{AppError, AppResult};

pub(super) fn validate_identifier(field: &str, value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::bad_request(format!("{field} is required")));
    }
    Ok(())
}
