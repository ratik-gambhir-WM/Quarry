//! Caller-only request checks for the classification operation.
use crate::{
    app::http::error::{AppError, AppResult},
    shared::ids::is_content_address,
};

/// JavaScript-safe integer ceiling shared with the DB generation bound.
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Document IDs are deal-scoped content addresses (64 lowercase hex digits).
pub(super) fn validate_file_id(file_id: &str) -> AppResult<()> {
    if !is_content_address(file_id) {
        return Err(AppError::bad_request("invalid fileId"));
    }
    Ok(())
}

pub(super) fn validate_expected_generation(value: i64) -> AppResult<()> {
    if !(0..MAX_SAFE_INTEGER).contains(&value) {
        return Err(AppError::bad_request(
            "expectedGeneration must be a non-negative safe integer",
        ));
    }
    Ok(())
}
