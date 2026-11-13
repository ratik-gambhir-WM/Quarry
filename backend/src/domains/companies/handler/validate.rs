use super::super::model::CompanyInput;
use crate::app::http::error::{AppError, AppResult};

pub(super) fn validate_company(input: &CompanyInput) -> AppResult<()> {
    validate_text("name", &input.name, 300)?;
    for (field, value, max) in [
        ("website", &input.website, 2048),
        ("industry", &input.industry, 300),
        ("description", &input.description, 4000),
    ] {
        if let Some(value) = value {
            validate_text(field, value, max)?;
        }
    }
    if let Some(website) = &input.website {
        let url = reqwest::Url::parse(website)
            .map_err(|_| AppError::bad_request("website must be an HTTPS URL"))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(AppError::bad_request("website must be an HTTPS URL"));
        }
    }
    Ok(())
}

pub(super) fn validate_text(field: &str, value: &str, max: usize) -> AppResult<()> {
    if value.trim().is_empty() || value.chars().count() > max || value.chars().any(char::is_control)
    {
        return Err(AppError::bad_request(format!("{field} is invalid")));
    }
    Ok(())
}

pub(super) fn revision(value: i64) -> AppResult<()> {
    if !(1..=crate::shared::revision::MAX_REVISION).contains(&value) {
        return Err(AppError::bad_request("invalid expectedRevision"));
    }
    Ok(())
}
