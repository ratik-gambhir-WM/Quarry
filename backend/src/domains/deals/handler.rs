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
        Deal, DealService, DealWithMetadata, SaveDealInput, SaveDealResponse,
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

fn validate_deal_input(input: &SaveDealInput) -> AppResult<()> {
    let required = [
        ("dealId", input.deal_id.as_str()),
        ("dealName", input.deal_name.as_str()),
        ("status", input.status.as_str()),
        ("startDate", input.start_date.as_str()),
        ("closeDate", input.close_date.as_str()),
        ("transactionType", input.transaction_type.as_str()),
        ("targetCompany", input.target_company.as_str()),
        ("primaryBuyer", input.primary_buyer.as_str()),
        ("dealSponsor", input.deal_sponsor.as_str()),
        ("userEmail", input.user_email.as_str()),
    ];
    if let Some((name, _)) = required.iter().find(|(_, value)| value.trim().is_empty()) {
        return Err(AppError::bad_request(format!("{name} is required")));
    }
    if !input.deal_id.starts_with("DEAL-")
        || input.deal_id.len() > 64
        || !input
            .deal_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::bad_request(
            "dealId must start with DEAL- and contain only letters, numbers, hyphens, and underscores",
        ));
    }
    let start_date = parse_date("startDate", &input.start_date)?;
    let close_date = parse_date("closeDate", &input.close_date)?;
    if close_date < start_date {
        return Err(AppError::bad_request(
            "closeDate cannot be before startDate",
        ));
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

pub(super) fn validate_sharepoint_link(link: Option<&str>) -> AppResult<()> {
    if let Some(link) = link {
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
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::bad_request(format!("{field} must use YYYY-MM-DD format")))
}

pub(super) fn validate_https_link(field: &str, link: Option<&str>) -> AppResult<()> {
    if let Some(link) = link {
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
    state
        .deals
        .archive(&deal_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}
