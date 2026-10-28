use std::sync::Arc;

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

pub use crate::domains::deals::repository::{Deal, DealMetadata, DealWithMetadata};

use crate::{
    adapters::openai::client::{OpenAiClient, ResponsesFileInput},
    domains::{
        deals::repository::{CreateDealRecord, DealRepository, UpsertDealMetadataRecord},
        users::UserDirectory,
    },
    shared::{
        error::{ServiceError, ServiceResult},
        prompts::{DealExtractionPromptData, PromptCatalog, PromptId},
    },
};

pub use super::model::DealFields;
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDealInput {
    pub deal_id: String,
    #[serde(skip_deserializing)]
    pub user_email: String,
    #[serde(flatten)]
    pub fields: DealFields,
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

#[derive(Debug, Clone, Default)]
pub struct SaveDealMetadataInput {
    pub uploaded_files: Vec<UploadedDealFile>,
    pub sharepoint_link: Option<String>,
    pub sow_link: Option<String>,
    pub fact_sheet_link: Option<String>,
    pub rl_link: Option<String>,
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
    users: UserDirectory,
    deals: DealRepository,
    openai: Option<Arc<OpenAiClient>>,
    extraction_model: String,
    prompts: Arc<PromptCatalog>,
}

impl DealService {
    pub fn new(
        users: UserDirectory,
        deals: DealRepository,
        openai: Option<Arc<OpenAiClient>>,
        extraction_model: String,
        prompts: Arc<PromptCatalog>,
    ) -> Self {
        Self {
            users,
            deals,
            openai,
            extraction_model,
            prompts,
        }
    }

    pub async fn list_owned(&self, owner: i64) -> ServiceResult<Vec<DealWithMetadata>> {
        self.deals.list_owned(owner).await.map_err(Into::into)
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
        let user_email = input.user_email.trim().to_string();
        let user = self
            .users
            .by_email(&user_email)
            .await?
            .ok_or_else(|| ServiceError::not_found("user not found"))?;
        validate_target_rule(&input.fields)?;
        let aggregate = self
            .deals
            .create(CreateDealRecord {
                deal_id: input.deal_id,
                user_id: user.id,
                fields: input.fields,
                local_path: trim_optional_owned(input.local_path),
                sharepoint_link: trim_optional_owned(input.sharepoint_link),
            })
            .await?;
        let deal = aggregate.deal;
        let metadata = aggregate
            .metadata
            .ok_or_else(|| ServiceError::internal("created metadata missing"))?;
        Ok(SaveDealResponse { deal, metadata })
    }

    pub async fn update(&self, id: &str, fields: DealFields) -> ServiceResult<SaveDealResponse> {
        validate_target_rule(&fields)?;
        let aggregate = self.deals.update(id.to_string(), fields).await?;
        let metadata = aggregate
            .metadata
            .ok_or_else(|| ServiceError::internal("deal metadata missing"))?;
        Ok(SaveDealResponse {
            deal: aggregate.deal,
            metadata,
        })
    }

    pub async fn save_metadata(
        &self,
        deal_id: &str,
        input: SaveDealMetadataInput,
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
        let sharepoint_link =
            updated_optional_value(input.sharepoint_link, existing_metadata.sharepoint_link);
        let sow_link = updated_optional_value(input.sow_link, existing_metadata.sow_link);
        let fact_sheet_link =
            updated_optional_value(input.fact_sheet_link, existing_metadata.fact_sheet_link);
        let rl_link = updated_optional_value(input.rl_link, existing_metadata.rl_link);
        if existing_metadata.local_path.is_some() && sharepoint_link.is_some() {
            return Err(ServiceError::validation(
                "localPath and sharepointLink cannot both be provided",
            ));
        }
        let metadata = self
            .deals
            .upsert_metadata(UpsertDealMetadataRecord {
                deal_id: deal.deal_id.clone(),
                key_questions_json: existing_metadata.key_questions_json,
                local_path: existing_metadata.local_path,
                sharepoint_link,
                sow_link,
                fact_sheet_link,
                rl_link,
            })
            .await?;
        let files = input
            .uploaded_files
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
        if files.is_empty() {
            return Ok(SaveDealMetadataResponse {
                deal,
                files: Vec::new(),
                extraction: DealExtraction {
                    key_questions: Vec::new(),
                },
                metadata,
            });
        }
        let extraction = self.extract_from_files(&deal, &files).await?;
        let key_questions_json =
            serde_json::to_string(&extraction.key_questions).map_err(|error| {
                ServiceError::internal(format!("failed to serialize deal key questions: {error}"))
            })?;
        let metadata = self
            .deals
            .upsert_metadata(UpsertDealMetadataRecord {
                deal_id: deal.deal_id.clone(),
                key_questions_json,
                local_path: metadata.local_path,
                sharepoint_link: metadata.sharepoint_link,
                sow_link: metadata.sow_link,
                fact_sheet_link: metadata.fact_sheet_link,
                rl_link: metadata.rl_link,
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
        let prompt = self
            .prompts
            .render_deal_extraction(DealExtractionPromptData {
                deal_id: &deal.deal_id,
                deal_name: &deal.deal_name,
                transaction_type: deal.transaction_type.label(),
                engagement_type: deal.engagement_type.label(),
                parties: &party_context(deal),
                scopes: &deal
                    .scopes
                    .iter()
                    .map(|s| s.label())
                    .collect::<Vec<_>>()
                    .join(", "),
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
                Some(self.prompts.prompt(PromptId::DealExtractionSystem)),
                Some(&self.extraction_model),
                Some(&file_inputs),
            )
            .await
            .map_err(ServiceError::internal)?;
        parse_deal_extraction(&response).map_err(ServiceError::validation)
    }
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

fn trim_optional_owned(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn updated_optional_value(submitted: Option<String>, existing: Option<String>) -> Option<String> {
    match submitted {
        Some(value) => trim_optional_owned(Some(value)),
        None => existing,
    }
}

#[cfg(test)]
#[path = "../../../tests/integration/domains/deals/service_tests.rs"]
mod tests;

fn validate_target_rule(fields: &DealFields) -> ServiceResult<()> {
    use super::model::{PartyRole, TransactionType};
    if fields.status.requires_acquisition_target()
        && matches!(
            fields.transaction_type,
            TransactionType::PlatformAcquisition
                | TransactionType::AddOnAcquisition
                | TransactionType::StrategicAcquisition
        )
        && fields
            .parties
            .iter()
            .filter(|p| p.role == PartyRole::Target)
            .count()
            != 1
    {
        return Err(ServiceError::validation(
            "working-stage or closed acquisitions require exactly one target",
        ));
    }
    Ok(())
}
pub(super) fn party_context(deal: &Deal) -> String {
    deal.parties
        .iter()
        .map(|p| format!("{}: {}", p.role.label(), p.company.name))
        .collect::<Vec<_>>()
        .join("\n")
}
