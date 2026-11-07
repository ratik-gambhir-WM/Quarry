use super::super::model::DealRole;
use super::Paging;
use crate::app::http::error::{AppError, AppResult};

pub(super) fn paging(p: Paging) -> AppResult<(String, usize)> {
    let limit = p.limit.unwrap_or(30);
    let cursor = p.cursor.unwrap_or_default();
    if !(1..=100).contains(&limit) || cursor.len() > 128 || cursor.chars().any(char::is_control) {
        return Err(AppError::bad_request("invalid pagination"));
    }
    Ok((cursor, limit))
}

pub(super) fn user(id: i64) -> AppResult<()> {
    if !(1..=9_007_199_254_740_991).contains(&id) {
        return Err(AppError::bad_request("invalid userId"));
    }
    Ok(())
}

pub(super) fn revision(value: i64) -> AppResult<()> {
    if !(1..=9_007_199_254_740_991).contains(&value) {
        return Err(AppError::bad_request("invalid revision"));
    }
    Ok(())
}

pub(super) fn team(id: &str) -> AppResult<()> {
    uuid::Uuid::parse_str(id).map_err(|_| AppError::bad_request("invalid teamId"))?;
    Ok(())
}

pub(super) fn name(value: &str) -> AppResult<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty()
        || normalized.chars().count() > 100
        || value.chars().any(char::is_control)
    {
        return Err(AppError::bad_request(
            "team name must contain 1 to 100 characters",
        ));
    }
    Ok(normalized)
}

pub(super) fn non_owner(role: DealRole) -> AppResult<()> {
    if role == DealRole::Owner {
        return Err(AppError::bad_request("use ownership transfer"));
    }
    Ok(())
}

pub(super) fn member_email(value: &str) -> AppResult<String> {
    let value = value.trim().to_ascii_lowercase();
    let (local, domain) = value
        .split_once('@')
        .ok_or_else(|| AppError::bad_request("invalid email"))?;
    if value.chars().count() > 320
        || local.is_empty()
        || !domain.contains('.')
        || domain.contains('@')
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AppError::bad_request("invalid email"));
    }
    Ok(value)
}
