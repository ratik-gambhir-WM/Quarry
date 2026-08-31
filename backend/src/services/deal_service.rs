use std::sync::Arc;

use base64::{engine::general_purpose, Engine as _};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub use crate::repository::deal_repository::{Deal, DealMetadata, DealWithMetadata};

use crate::{
    core::{
        clients::openai::{OpenAiClient, ResponsesFileInput},
        prompts::{
            build_deal_extraction_prompt, DealExtractionPromptVariables,
            DEAL_EXTRACTION_SYSTEM_PROMPT,
        },
    },
    repository::{
        deal_repository::{CreateDealRecord, DealRepository, UpsertDealMetadataRecord},
        user_repository::UserRepository,
    },
    services::error::{ServiceError, ServiceResult},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDealInput {
    pub deal_id: String,
    pub deal_name: String,
    pub status: String,
    pub start_date: String,
    pub close_date: String,
    pub transaction_type: String,
    pub target_company: String,
    pub primary_buyer: String,
    pub deal_sponsor: String,
    pub user_email: String,
    pub local_path: Option<String>,
    pub sharepoint_link: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UploadedDealFile {
    pub relative_path: String,
    pub filename: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealSourceFile {
    pub path: String,
    pub filename: String,
    pub relative_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealExtraction {
    pub key_questions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDealResponse {
    pub deal: Deal,
    pub metadata: DealMetadata,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDealMetadataResponse {
    pub deal: Deal,
    pub files: Vec<DealSourceFile>,
    pub extraction: DealExtraction,
    pub metadata: DealMetadata,
}

struct MatchedDealFile {
    source_file: DealSourceFile,
    data_base64: String,
    mime_type: String,
}

#[derive(Clone)]
pub struct DealService {
    users: UserRepository,
    deals: DealRepository,
    openai: Option<Arc<OpenAiClient>>,
    extraction_model: String,
}

impl DealService {
    pub fn new(
        users: UserRepository,
        deals: DealRepository,
        openai: Option<Arc<OpenAiClient>>,
        extraction_model: String,
    ) -> Self {
        Self {
            users,
            deals,
            openai,
            extraction_model,
        }
    }

    pub async fn list(&self) -> ServiceResult<Vec<DealWithMetadata>> {
        self.deals.list().await.map_err(Into::into)
    }

    pub async fn get(&self, deal_id: &str) -> ServiceResult<DealWithMetadata> {
        self.deals
            .with_metadata(deal_id.to_string())
            .await?
            .ok_or_else(|| ServiceError::not_found(format!("deal not found for id `{deal_id}`")))
    }

    pub async fn archive(&self, deal_id: &str) -> ServiceResult<Deal> {
        self.deals
            .archive(deal_id.to_string())
            .await?
            .ok_or_else(|| ServiceError::not_found(format!("deal not found for id `{deal_id}`")))
    }

    pub async fn create(&self, input: SaveDealInput) -> ServiceResult<SaveDealResponse> {
        validate_deal_input(&input)?;
        let user_email = input.user_email.trim().to_string();
        let user = self
            .users
            .by_email(user_email.clone())
            .await?
            .ok_or_else(|| {
                ServiceError::validation(format!("user not found for email `{user_email}`"))
            })?;
        let deal = self
            .deals
            .create(CreateDealRecord {
                deal_id: input.deal_id.trim().to_string(),
                user_id: user.id,
                deal_name: input.deal_name.trim().to_string(),
                status: input.status.trim().to_string(),
                start_date: input.start_date.trim().to_string(),
                close_date: input.close_date.trim().to_string(),
                transaction_type: input.transaction_type.trim().to_string(),
                target_company: input.target_company.trim().to_string(),
                primary_buyer: input.primary_buyer.trim().to_string(),
                deal_sponsor: input.deal_sponsor.trim().to_string(),
            })
            .await
            .map_err(|error| ServiceError::validation(error.to_string()))?;
        let metadata = self
            .deals
            .upsert_metadata(UpsertDealMetadataRecord {
                deal_id: deal.deal_id.clone(),
                user_id: deal.user_id,
                key_questions_json: "[]".to_string(),
                local_path: trim_optional_owned(input.local_path),
                sharepoint_link: trim_optional_owned(input.sharepoint_link),
            })
            .await
            .map_err(|error| ServiceError::validation(error.to_string()))?;
        Ok(SaveDealResponse { deal, metadata })
    }

    pub async fn save_metadata(
        &self,
        deal_id: &str,
        uploaded_files: Vec<UploadedDealFile>,
    ) -> ServiceResult<SaveDealMetadataResponse> {
        let deal = self
            .deals
            .by_id(deal_id.to_string())
            .await?
            .ok_or_else(|| ServiceError::not_found(format!("deal not found for id `{deal_id}`")))?;
        let existing_metadata =
            self.deals
                .metadata(deal_id.to_string())
                .await?
                .ok_or_else(|| {
                    ServiceError::not_found(format!("deal metadata not found for id `{deal_id}`"))
                })?;
        let files = uploaded_files
            .into_iter()
            .map(|file| MatchedDealFile {
                source_file: DealSourceFile {
                    path: file.relative_path.clone(),
                    filename: file.filename,
                    relative_path: file.relative_path,
                    size_bytes: file.bytes.len() as u64,
                },
                data_base64: general_purpose::STANDARD.encode(file.bytes),
                mime_type: file.mime_type,
            })
            .collect::<Vec<_>>();
        let extraction = self.extract_from_files(&deal, &files).await?;
        let key_questions_json =
            serde_json::to_string(&extraction.key_questions).map_err(|error| {
                ServiceError::internal(format!("failed to serialize deal key questions: {error}"))
            })?;
        let metadata = self
            .deals
            .upsert_metadata(UpsertDealMetadataRecord {
                deal_id: deal.deal_id.clone(),
                user_id: deal.user_id,
                key_questions_json,
                local_path: existing_metadata.local_path,
                sharepoint_link: existing_metadata.sharepoint_link,
            })
            .await?;
        Ok(SaveDealMetadataResponse {
            deal,
            files: files.into_iter().map(|file| file.source_file).collect(),
            extraction,
            metadata,
        })
    }

    async fn extract_from_files(
        &self,
        deal: &Deal,
        files: &[MatchedDealFile],
    ) -> ServiceResult<DealExtraction> {
        if files.is_empty() {
            return Ok(DealExtraction {
                key_questions: Vec::new(),
            });
        }
        let client = self
            .openai
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("OpenAI capability is not configured"))?;
        let manifest = files
            .iter()
            .map(|file| {
                format!(
                    "- {} ({}, {} bytes)",
                    file.source_file.relative_path, file.mime_type, file.source_file.size_bytes
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = build_deal_extraction_prompt(DealExtractionPromptVariables {
            deal_id: &deal.deal_id,
            deal_name: &deal.deal_name,
            transaction_type: &deal.transaction_type,
            target_company: &deal.target_company,
            primary_buyer: &deal.primary_buyer,
            deal_sponsor: &deal.deal_sponsor,
            attached_file_manifest: &manifest,
        });
        let file_inputs = files
            .iter()
            .map(|file| ResponsesFileInput::FileData {
                filename: &file.source_file.filename,
                mime_type: &file.mime_type,
                data_base64: &file.data_base64,
            })
            .collect::<Vec<_>>();
        let response = client
            .gen_model_response_with_files(
                Some(&prompt),
                Some(DEAL_EXTRACTION_SYSTEM_PROMPT),
                Some(&self.extraction_model),
                Some(&file_inputs),
            )
            .await
            .map_err(ServiceError::internal)?;
        parse_deal_extraction(&response).map_err(ServiceError::validation)
    }
}

fn validate_deal_input(input: &SaveDealInput) -> ServiceResult<()> {
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
        return Err(ServiceError::validation(format!("{name} is required")));
    }
    if !input.deal_id.starts_with("DEAL-")
        || input.deal_id.len() > 64
        || !input
            .deal_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(ServiceError::validation("dealId must start with DEAL- and contain only letters, numbers, hyphens, and underscores"));
    }
    let start_date = parse_date("startDate", &input.start_date)?;
    let close_date = parse_date("closeDate", &input.close_date)?;
    if close_date < start_date {
        return Err(ServiceError::validation(
            "closeDate cannot be before startDate",
        ));
    }

    let local_path = trim_optional(input.local_path.as_deref());
    let sharepoint_link = trim_optional(input.sharepoint_link.as_deref());
    if local_path.is_some() && sharepoint_link.is_some() {
        return Err(ServiceError::validation(
            "localPath and sharepointLink cannot both be provided",
        ));
    }
    if let Some(link) = sharepoint_link {
        if !link.starts_with("https://") || !link.contains(".sharepoint.com/") {
            return Err(ServiceError::validation(
                "sharepointLink must be an HTTPS SharePoint URL",
            ));
        }
    }
    Ok(())
}

fn parse_date(field: &str, value: &str) -> ServiceResult<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| ServiceError::validation(format!("{field} must use YYYY-MM-DD format")))
}

fn parse_deal_extraction(response: &str) -> Result<DealExtraction, String> {
    let trimmed = response.trim();
    let json_text = trimmed
        .strip_prefix("```json")
        .and_then(|text| text.strip_suffix("```"))
        .or_else(|| {
            trimmed
                .strip_prefix("```")
                .and_then(|text| text.strip_suffix("```"))
        })
        .unwrap_or(trimmed)
        .trim();
    serde_json::from_str(json_text)
        .map_err(|error| format!("failed to parse deal extraction JSON: {error}"))
}

fn trim_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn trim_optional_owned(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../tests/services/deal_service_tests.rs"]
mod tests;
