use std::env;

use base64::{engine::general_purpose, Engine as _};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    core::{
        clients::openai::{OpenAiClient, ResponsesFileInput},
        nodes::deal_node::DealNode,
    },
    repository::deal_repository::{
        create_deal, get_deal_by_id, get_deal_metadata_by_deal_id, get_helix_deal_by_id,
        upsert_deal_metadata, upsert_helix_deal, CreateDealRecord, Deal, DealMetadata,
        UpsertDealMetadataRecord,
    },
    services::user_service::get_sqlite_user_by_email,
    state::AppState,
    utils::openai_api_key,
};

const DEFAULT_DEAL_EXTRACTION_MODEL: &str = "gpt-5.6-luna";

pub async fn save_helix_deal(state: &AppState, deal: DealNode) -> Result<Value, String> {
    upsert_helix_deal(state, deal).await
}

pub async fn get_helix_deal(state: &AppState, deal_id: &str) -> Result<Value, String> {
    get_helix_deal_by_id(state, deal_id).await
}

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

pub fn save_deal(state: &AppState, input: SaveDealInput) -> Result<SaveDealResponse, String> {
    validate_deal_input(&input)?;
    let user = get_sqlite_user_by_email(state, input.user_email.trim())?
        .ok_or_else(|| format!("user not found for email `{}`", input.user_email.trim()))?;
    let deal = create_deal(
        state,
        CreateDealRecord {
            deal_id: input.deal_id.trim(),
            deal_name: input.deal_name.trim(),
            status: input.status.trim(),
            start_date: input.start_date.trim(),
            close_date: input.close_date.trim(),
            transaction_type: input.transaction_type.trim(),
            target_company: input.target_company.trim(),
            primary_buyer: input.primary_buyer.trim(),
            deal_sponsor: input.deal_sponsor.trim(),
        },
    )?;
    let metadata = upsert_deal_metadata(
        state,
        UpsertDealMetadataRecord {
            deal_id: &deal.deal_id,
            user_id: user.id,
            key_questions_json: "[]",
            local_path: trim_optional(input.local_path.as_deref()),
            sharepoint_link: trim_optional(input.sharepoint_link.as_deref()),
        },
    )?;
    Ok(SaveDealResponse { deal, metadata })
}

pub async fn save_deal_metadata(
    state: &AppState,
    deal_id: &str,
    uploaded_files: Vec<UploadedDealFile>,
) -> Result<SaveDealMetadataResponse, String> {
    let deal = get_deal_by_id(state, deal_id)?
        .ok_or_else(|| format!("deal not found for id `{deal_id}`"))?;
    let existing_metadata = get_deal_metadata_by_deal_id(state, deal_id)?
        .ok_or_else(|| format!("deal metadata not found for id `{deal_id}`"))?;
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
    let extraction = extract_from_files(&deal, &files).await?;
    let key_questions_json = serde_json::to_string(&extraction.key_questions)
        .map_err(|err| format!("failed to serialize deal key questions: {err}"))?;
    let metadata = upsert_deal_metadata(
        state,
        UpsertDealMetadataRecord {
            deal_id: &deal.deal_id,
            user_id: existing_metadata.user_id,
            key_questions_json: &key_questions_json,
            local_path: existing_metadata.local_path.as_deref(),
            sharepoint_link: existing_metadata.sharepoint_link.as_deref(),
        },
    )?;
    Ok(SaveDealMetadataResponse {
        deal,
        files: files.into_iter().map(|file| file.source_file).collect(),
        extraction,
        metadata,
    })
}

fn validate_deal_input(input: &SaveDealInput) -> Result<(), String> {
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
        return Err(format!("{name} is required"));
    }
    if !input.deal_id.starts_with("DEAL-")
        || input.deal_id.len() > 64
        || !input
            .deal_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("dealId must start with DEAL- and contain only letters, numbers, hyphens, and underscores".to_string());
    }
    let start_date = parse_date("startDate", &input.start_date)?;
    let close_date = parse_date("closeDate", &input.close_date)?;
    if close_date < start_date {
        return Err("closeDate cannot be before startDate".to_string());
    }

    let local_path = trim_optional(input.local_path.as_deref());
    let sharepoint_link = trim_optional(input.sharepoint_link.as_deref());
    if local_path.is_some() && sharepoint_link.is_some() {
        return Err("localPath and sharepointLink cannot both be provided".to_string());
    }
    if let Some(link) = sharepoint_link {
        if !link.starts_with("https://") || !link.contains(".sharepoint.com/") {
            return Err("sharepointLink must be an HTTPS SharePoint URL".to_string());
        }
    }
    Ok(())
}

fn parse_date(field: &str, value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| format!("{field} must use YYYY-MM-DD format"))
}

async fn extract_from_files(
    deal: &Deal,
    files: &[MatchedDealFile],
) -> Result<DealExtraction, String> {
    if files.is_empty() {
        return Ok(DealExtraction {
            key_questions: Vec::new(),
        });
    }
    let api_key = openai_api_key()?;
    let client = OpenAiClient::new(&api_key);
    let model = env::var("OPENAI_DEAL_EXTRACTION_MODEL")
        .unwrap_or_else(|_| DEFAULT_DEAL_EXTRACTION_MODEL.to_string());
    let prompt = build_deal_extraction_prompt(deal, files);
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
            Some("You extract private equity diligence questions from deal documents. Return only strict JSON with no Markdown."),
            Some(&model),
            Some(&file_inputs),
        )
        .await?;
    parse_deal_extraction(&response)
}

fn build_deal_extraction_prompt(deal: &Deal, files: &[MatchedDealFile]) -> String {
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
    format!(
        "Deal metadata:\nDeal ID: {}\nDeal name: {}\nTransaction type: {}\nTarget company: {}\nPrimary buyer: {}\nDeal sponsor: {}\n\n\
Review the attached files. Extract only questions explicitly listed beneath a section heading labeled Key Questions or Key Diligence Questions in an attached Word document. Do not create, infer, rewrite, synthesize, or add questions.\n\n\
Return strict JSON with exactly one key: \"keyQuestions\". If there is no qualifying section, return an empty array. Do not include Markdown, commentary, citations, or extra keys.\n\nAttached file manifest:\n{}",
        deal.deal_id,
        deal.deal_name,
        deal.transaction_type,
        deal.target_company,
        deal.primary_buyer,
        deal.deal_sponsor,
        manifest
    )
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
        .map_err(|err| format!("failed to parse deal extraction JSON: {err}"))
}

fn trim_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../tests/services/deal_service_tests.rs"]
mod tests;
