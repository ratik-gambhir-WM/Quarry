//! Content revisions use the same exact integer domain in PostgreSQL and JavaScript.
use crate::shared::error::RepositoryError;

pub const MAX_REVISION: i64 = 9_007_199_254_740_991;

pub fn next(current: i64) -> Result<i64, RepositoryError> {
    current
        .checked_add(1)
        .filter(|next| (2..=MAX_REVISION).contains(next))
        .ok_or_else(|| {
            RepositoryError::Conflict("revision exhausted; reload before editing".into())
        })
}

pub fn conflict() -> RepositoryError {
    RepositoryError::Conflict("content changed; reload before editing".into())
}
