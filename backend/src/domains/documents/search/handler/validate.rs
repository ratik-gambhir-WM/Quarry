use crate::app::http::error::{AppError, AppResult};
use crate::domains::documents::store::chunk::MAX_FILE_CHUNK_SEARCH_LIMIT;

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

pub(super) fn validate_vector(values: &[f32]) -> AppResult<()> {
    let stored_squared_norm = values.iter().map(|value| value * value).sum::<f32>();
    if values.is_empty()
        || values.len() > 16000
        || values.iter().any(|v| !v.is_finite())
        || !stored_squared_norm.is_finite()
        || stored_squared_norm <= 0.0
        || values.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() <= 0.0
    {
        return Err(AppError::bad_request(
            "queryEmbedding must have valid dimensions and finite nonzero norm",
        ));
    }
    Ok(())
}
