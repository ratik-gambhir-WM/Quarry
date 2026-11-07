use crate::app::http::error::{AppError, AppResult};

pub(super) fn username(value: &mut String) -> AppResult<()> {
    *value = value.trim().to_ascii_lowercase();
    if !(3..=32).contains(&value.len())
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    {
        return Err(AppError::bad_request(
            "username must be 3–32 letters, digits, dots, underscores or hyphens",
        ));
    }
    Ok(())
}

pub(super) fn email(value: &mut String) -> AppResult<()> {
    *value = value.trim().to_ascii_lowercase();
    text(value, 320)?;
    let (local, domain) = value
        .split_once('@')
        .ok_or_else(|| AppError::bad_request("invalid email"))?;
    if local.is_empty()
        || !domain.contains('.')
        || domain.contains('@')
        || value.chars().any(char::is_whitespace)
    {
        return Err(AppError::bad_request("invalid email"));
    }
    Ok(())
}

pub(super) fn password(value: &str) -> AppResult<()> {
    if !(15..=128).contains(&value.chars().count()) || value.len() > 512 {
        return Err(AppError::bad_request(
            "password must be 15–128 characters and at most 512 bytes",
        ));
    }
    Ok(())
}

pub(super) fn text(value: &str, max: usize) -> AppResult<()> {
    if value.trim().is_empty() || value.chars().count() > max || value.chars().any(char::is_control)
    {
        return Err(AppError::bad_request("invalid profile field"));
    }
    Ok(())
}
