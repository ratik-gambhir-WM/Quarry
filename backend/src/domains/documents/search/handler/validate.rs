use crate::app::http::error::{AppError, AppResult};
use crate::domains::documents::graph::query::MAX_FILE_CHUNK_SEARCH_LIMIT;

pub(super) fn validate_common(limit: usize) -> AppResult<()> {
    if limit == 0 {
        return Err(AppError::bad_request("limit must be greater than zero"));
    }
    if limit > MAX_FILE_CHUNK_SEARCH_LIMIT {
        return Err(AppError::bad_request(format!(
            "limit must not exceed {MAX_FILE_CHUNK_SEARCH_LIMIT}"
        )));
    }
    Ok(())
}
