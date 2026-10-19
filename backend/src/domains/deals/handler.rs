use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::NaiveDate;
use std::sync::Arc;

use crate::{
    app::http::error::{AppError, AppResult},
    domains::deals::service::{
        Deal, DealFields, DealService, DealWithMetadata, SaveDealInput, SaveDealResponse,
    },
};

#[derive(Clone)]
pub(super) struct DealsHttpState {
    pub deals: Arc<DealService>,
}

pub(super) async fn list_deals_handler(
    State(state): State<DealsHttpState>,
) -> AppResult<Json<Vec<DealWithMetadata>>> {
    state.deals.list().await.map(Json).map_err(AppError::from)
}

pub(super) async fn get_deal_handler(
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
) -> crate::app::http::error::AppResult<Json<DealWithMetadata>> {
    validate_deal_id(&deal_id)?;
    state
        .deals
        .get(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn create_deal_handler(
    State(state): State<DealsHttpState>,
    Json(input): Json<SaveDealInput>,
) -> crate::app::http::error::AppResult<(StatusCode, Json<SaveDealResponse>)> {
    validate_deal_input(&input)?;
    state
        .deals
        .create(input)
        .await
        .map(|response| (StatusCode::CREATED, Json(response)))
        .map_err(AppError::from)
}

pub(super) fn validate_deal_id(id: &str) -> AppResult<()> {
    if !id.starts_with("DEAL-")
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(AppError::bad_request("invalid dealId"));
    }
    Ok(())
}
fn validate_deal_input(input: &SaveDealInput) -> AppResult<()> {
    validate_deal_id(&input.deal_id)?;
    validate_text("userEmail", &input.user_email, 320)?;
    validate_fields(&input.fields)?;
    if let Some(path) = &input.local_path {
        validate_text("localPath", path, 4096)?;
    }
    let local_path = trim_optional(input.local_path.as_deref());
    let sharepoint_link = trim_optional(input.sharepoint_link.as_deref());
    if local_path.is_some() && sharepoint_link.is_some() {
        return Err(AppError::bad_request(
            "localPath and sharepointLink cannot both be provided",
        ));
    }
    validate_sharepoint_link(sharepoint_link)
}
fn validate_text(field: &str, value: &str, max: usize) -> AppResult<()> {
    if value.trim().is_empty() || value.chars().count() > max || value.chars().any(char::is_control)
    {
        return Err(AppError::bad_request(format!("{field} is invalid")));
    }
    Ok(())
}
fn validate_fields(fields: &DealFields) -> AppResult<()> {
    validate_text("dealName", &fields.deal_name, 300)?;
    if let Some(code) = &fields.deal_code {
        validate_text("dealCode", code, 100)?;
    }
    for (name, date) in [
        ("startDate", &fields.start_date),
        ("expectedCloseDate", &fields.expected_close_date),
        ("actualCloseDate", &fields.actual_close_date),
        ("engagementCompletedDate", &fields.engagement_completed_date),
    ] {
        if let Some(date) = date {
            parse_date(name, date)?;
        }
    }
    if let (Some(start), Some(completed)) = (&fields.start_date, &fields.engagement_completed_date)
    {
        if completed < start {
            return Err(AppError::bad_request(
                "engagementCompletedDate cannot be before startDate",
            ));
        }
    }
    if fields.parties.len() > 100 || fields.scopes.len() > 7 {
        return Err(AppError::bad_request("too many parties or scopes"));
    }
    let mut seen = std::collections::HashSet::new();
    for party in &fields.parties {
        uuid::Uuid::parse_str(&party.company_id)
            .map_err(|_| AppError::bad_request("invalid companyId"))?;
        if !seen.insert((&party.company_id, party.role)) {
            return Err(AppError::bad_request("duplicate party assignment"));
        }
        if let Some(notes) = &party.notes {
            validate_text("notes", notes, 2000)?;
        }
    }
    if fields
        .scopes
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != fields.scopes.len()
    {
        return Err(AppError::bad_request("duplicate scope"));
    }
    Ok(())
}
pub(super) async fn update_deal_handler(
    State(state): State<DealsHttpState>,
    Path(id): Path<String>,
    Json(fields): Json<DealFields>,
) -> AppResult<Json<SaveDealResponse>> {
    validate_deal_id(&id)?;
    validate_fields(&fields)?;
    Ok(Json(state.deals.update(&id, fields).await?))
}

pub(super) fn validate_sharepoint_link(link: Option<&str>) -> AppResult<()> {
    if let Some(link) = link.filter(|value| !value.trim().is_empty()) {
        validate_text("sharepointLink", link, 2048)?;
        let parsed = reqwest::Url::parse(link)
            .map_err(|_| AppError::bad_request("sharepointLink must be an HTTPS SharePoint URL"))?;
        let is_sharepoint = parsed
            .host_str()
            .is_some_and(|host| host.to_ascii_lowercase().ends_with(".sharepoint.com"));
        if parsed.scheme() != "https"
            || !is_sharepoint
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(AppError::bad_request(
                "sharepointLink must be an HTTPS SharePoint URL",
            ));
        }
    }
    Ok(())
}

fn parse_date(field: &str, value: &str) -> AppResult<NaiveDate> {
    if value.len() != 10 || value.as_bytes()[4] != b'-' || value.as_bytes()[7] != b'-' {
        return Err(AppError::bad_request(format!(
            "{field} must use YYYY-MM-DD format"
        )));
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request(format!("{field} must use YYYY-MM-DD format")))
}

pub(super) fn validate_https_link(field: &str, link: Option<&str>) -> AppResult<()> {
    if let Some(link) = link.filter(|value| !value.trim().is_empty()) {
        validate_text(field, link, 2048)?;
        let parsed = reqwest::Url::parse(link)
            .map_err(|_| AppError::bad_request(format!("{field} must be an HTTPS URL")))?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(AppError::bad_request(format!(
                "{field} must be an HTTPS URL"
            )));
        }
    }
    Ok(())
}

fn trim_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(super) async fn archive_deal_handler(
    State(state): State<DealsHttpState>,
    Path(deal_id): Path<String>,
) -> crate::app::http::error::AppResult<Json<Deal>> {
    validate_deal_id(&deal_id)?;
    state
        .deals
        .archive(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}
