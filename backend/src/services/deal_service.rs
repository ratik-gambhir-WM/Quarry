use std::{env, fs, path::Path};

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

use crate::{
    core::{
        clients::openai::{OpenAiClient, ResponsesFileInput},
        display_relative_path, infer_supported_mime_type,
        nodes::deal_node::DealNode,
    },
    repository::deal_repository::{
        create_deal, get_deal_by_id, get_deal_metadata_by_deal_id, get_helix_deal_by_id,
        upsert_deal_metadata, upsert_helix_deal, CreateDealRecord, Deal, DealMetadata,
        UpsertDealMetadataRecord,
    },
    state::AppState,
    utils::openai_api_key,
};

const DEFAULT_DEAL_EXTRACTION_MODEL: &str = "gpt-5.6-luna";
const SOW_MATCH_TERMS: [&str; 2] = ["sow", "scope of work"];
const PROJECT_TIMELINE_MATCH_TERMS: [&str; 6] = [
    "project timeline",
    "timeline",
    "project plan",
    "workplan",
    "work plan",
    "schedule",
];
const DEAL_TYPES: [&str; 6] = [
    "Buy-side",
    "Sell-side",
    "Carve-out",
    "Add-on",
    "Recapitalization",
    "Growth equity",
];

pub async fn save_helix_deal(
    state: &AppState,
    deal: DealNode,
    user_id: i64,
) -> Result<Value, String> {
    upsert_helix_deal(state, deal, user_id).await
}

pub async fn get_helix_deal(state: &AppState, deal_id: i64) -> Result<Value, String> {
    get_helix_deal_by_id(state, deal_id).await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDealAndExtractInput {
    pub deal_name: String,
    pub main_data_room_folder: String,
    pub deal_type: String,
    pub pe_firm: String,
    pub target_company: Option<String>,
    pub buyer_or_platform_company: Option<String>,
    pub parent_or_seller_company: Option<String>,
    pub carve_out_business: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UploadedDealFile {
    pub relative_path: String,
    pub filename: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealSourceFile {
    pub path: String,
    pub filename: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub matched_on: Vec<String>,
    pub text_extracted: bool,
    pub text_truncated: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealExtraction {
    pub key_questions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDealAndExtractResponse {
    pub deal: Deal,
    pub files: Vec<DealSourceFile>,
    pub extraction: DealExtraction,
    pub metadata: DealMetadata,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDealAndFindFilesResponse {
    pub deal: Deal,
    pub files: Vec<DealSourceFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractDealQuestionsInput {
    pub sow_file_path: Option<String>,
    pub project_timeline_file_path: Option<String>,
}

struct MatchedDealFile {
    source_file: DealSourceFile,
    data_base64: Option<String>,
    mime_type: Option<String>,
}

pub fn create_local_deal(
    state: &AppState,
    input: SaveDealAndExtractInput,
) -> Result<SaveDealAndFindFilesResponse, String> {
    validate_deal_input(&input, true)?;
    let root = Path::new(input.main_data_room_folder.trim());
    validate_data_room_root(root)?;
    let deal = save_deal(state, &input)?;
    let files = discover_local_source_files(root)?
        .into_iter()
        .map(|file| file.source_file)
        .collect();
    Ok(SaveDealAndFindFilesResponse { deal, files })
}

pub fn create_uploaded_deal(
    state: &AppState,
    mut input: SaveDealAndExtractInput,
    upload_root: &str,
    uploaded_files: &[UploadedDealFile],
) -> Result<SaveDealAndFindFilesResponse, String> {
    if uploaded_files.is_empty() {
        return Err("at least one supported file upload is required".to_string());
    }
    input.main_data_room_folder = format!("browser-upload://{}", safe_root_label(upload_root));
    validate_deal_input(&input, false)?;
    let deal = save_deal(state, &input)?;

    let document_count = i64::try_from(uploaded_files.len())
        .map_err(|_| "uploaded document count exceeds supported range".to_string())?;
    let data_room_size_bytes = uploaded_files.iter().try_fold(0_i64, |total, file| {
        let size = i64::try_from(file.bytes.len())
            .map_err(|_| "uploaded file size exceeds supported range".to_string())?;
        total
            .checked_add(size)
            .ok_or_else(|| "uploaded data room size exceeds supported range".to_string())
    })?;
    upsert_deal_metadata(
        state,
        UpsertDealMetadataRecord {
            deal_id: deal.id,
            key_questions_json: "[]",
            document_count,
            data_room_size_bytes,
        },
    )?;

    let mut files = uploaded_files
        .iter()
        .filter_map(uploaded_source_file)
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(SaveDealAndFindFilesResponse { deal, files })
}

pub async fn extract_local_deal(
    state: &AppState,
    deal_id: i64,
    input: ExtractDealQuestionsInput,
) -> Result<SaveDealAndExtractResponse, String> {
    let deal = get_deal_by_id(state, deal_id)?
        .ok_or_else(|| format!("deal not found for id `{deal_id}`"))?;
    if deal.main_data_room_folder.starts_with("browser-upload://") {
        return Err(
            "this deal was created from browser uploads; use the upload extraction endpoint"
                .to_string(),
        );
    }

    let root = Path::new(&deal.main_data_room_folder);
    let mut files = Vec::new();
    for selected in [
        input.sow_file_path.as_deref(),
        input.project_timeline_file_path.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let selected = selected.trim();
        if selected.is_empty()
            || files
                .iter()
                .any(|file: &MatchedDealFile| file.source_file.path == selected)
        {
            continue;
        }
        files.push(load_local_selected_file(root, Path::new(selected))?);
    }

    let extraction = extract_from_files(&deal, &files).await?;
    let (document_count, data_room_size_bytes) = measure_data_room(root)?;
    finish_extraction(
        state,
        deal,
        files,
        extraction,
        document_count,
        data_room_size_bytes,
    )
}

pub async fn extract_uploaded_deal(
    state: &AppState,
    deal_id: i64,
    uploaded_files: Vec<UploadedDealFile>,
) -> Result<SaveDealAndExtractResponse, String> {
    let deal = get_deal_by_id(state, deal_id)?
        .ok_or_else(|| format!("deal not found for id `{deal_id}`"))?;
    let files = uploaded_files
        .into_iter()
        .map(|file| {
            let matched_on = matching_terms(&file.filename);
            MatchedDealFile {
                source_file: DealSourceFile {
                    path: file.relative_path.clone(),
                    filename: file.filename,
                    relative_path: file.relative_path,
                    size_bytes: file.bytes.len() as u64,
                    matched_on,
                    text_extracted: false,
                    text_truncated: false,
                },
                data_base64: Some(general_purpose::STANDARD.encode(file.bytes)),
                mime_type: Some(file.mime_type),
            }
        })
        .collect::<Vec<_>>();
    let extraction = extract_from_files(&deal, &files).await?;
    let existing_metadata = get_deal_metadata_by_deal_id(state, deal_id)?;
    let document_count = existing_metadata
        .as_ref()
        .map(|metadata| metadata.document_count)
        .unwrap_or(files.len() as i64);
    let data_room_size_bytes = existing_metadata
        .as_ref()
        .map(|metadata| metadata.data_room_size_bytes)
        .unwrap_or_else(|| {
            files
                .iter()
                .map(|file| file.source_file.size_bytes as i64)
                .sum()
        });

    finish_extraction(
        state,
        deal,
        files,
        extraction,
        document_count,
        data_room_size_bytes,
    )
}

fn finish_extraction(
    state: &AppState,
    deal: Deal,
    files: Vec<MatchedDealFile>,
    extraction: DealExtraction,
    document_count: i64,
    data_room_size_bytes: i64,
) -> Result<SaveDealAndExtractResponse, String> {
    let key_questions_json = serde_json::to_string(&extraction.key_questions)
        .map_err(|err| format!("failed to serialize deal key questions: {err}"))?;
    let metadata = upsert_deal_metadata(
        state,
        UpsertDealMetadataRecord {
            deal_id: deal.id,
            key_questions_json: &key_questions_json,
            document_count,
            data_room_size_bytes,
        },
    )?;
    Ok(SaveDealAndExtractResponse {
        deal,
        files: files.into_iter().map(|file| file.source_file).collect(),
        extraction,
        metadata,
    })
}

fn save_deal(state: &AppState, input: &SaveDealAndExtractInput) -> Result<Deal, String> {
    create_deal(
        state,
        CreateDealRecord {
            deal_name: input.deal_name.trim(),
            main_data_room_folder: input.main_data_room_folder.trim(),
            deal_type: input.deal_type.trim(),
            pe_firm: input.pe_firm.trim(),
            target_company: trim_optional(input.target_company.as_deref()),
            buyer_or_platform_company: trim_optional(input.buyer_or_platform_company.as_deref()),
            parent_or_seller_company: trim_optional(input.parent_or_seller_company.as_deref()),
            carve_out_business: trim_optional(input.carve_out_business.as_deref()),
        },
    )
}

fn validate_deal_input(
    input: &SaveDealAndExtractInput,
    require_local_path: bool,
) -> Result<(), String> {
    if input.deal_name.trim().is_empty() {
        return Err("dealName is required".to_string());
    }
    if require_local_path && input.main_data_room_folder.trim().is_empty() {
        return Err("mainDataRoomFolder is required".to_string());
    }
    if !DEAL_TYPES.contains(&input.deal_type.trim()) {
        return Err(format!(
            "dealType must be one of: {}",
            DEAL_TYPES.join(", ")
        ));
    }
    if input.pe_firm.trim().is_empty() {
        return Err("peFirm is required".to_string());
    }

    let required = |value: Option<&str>, name: &str| {
        if trim_optional(value).is_none() {
            Err(format!("{name} is required for {}", input.deal_type.trim()))
        } else {
            Ok(())
        }
    };
    match input.deal_type.trim() {
        "Sell-side" | "Recapitalization" | "Growth equity" => {
            required(input.target_company.as_deref(), "targetCompany")?
        }
        "Buy-side" | "Add-on" => {
            required(
                input.buyer_or_platform_company.as_deref(),
                "buyerOrPlatformCompany",
            )?;
            required(input.target_company.as_deref(), "targetCompany")?;
        }
        "Carve-out" => {
            required(
                input.parent_or_seller_company.as_deref(),
                "parentOrSellerCompany",
            )?;
            required(input.carve_out_business.as_deref(), "carveOutBusiness")?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_data_room_root(root: &Path) -> Result<(), String> {
    if !root.exists() {
        return Err(format!(
            "data room folder does not exist: {}",
            root.display()
        ));
    }
    if !root.is_dir() {
        return Err(format!(
            "data room path is not a folder: {}",
            root.display()
        ));
    }
    Ok(())
}

fn discover_local_source_files(root: &Path) -> Result<Vec<MatchedDealFile>, String> {
    validate_data_room_root(root)?;
    let admin_roots = admin_search_roots(root);
    let mut files = Vec::new();
    if admin_roots.is_empty() {
        collect_matching_local_files(root, root, &mut files)?;
    } else {
        for admin_root in &admin_roots {
            collect_matching_local_files(root, admin_root, &mut files)?;
        }
        if !has_source_type(&files, "SOW") || !has_source_type(&files, "Project Timeline") {
            collect_matching_local_files(root, root, &mut files)?;
        }
    }
    files.sort_by(|left, right| {
        left.source_file
            .relative_path
            .cmp(&right.source_file.relative_path)
    });
    files.dedup_by(|left, right| left.source_file.path == right.source_file.path);
    Ok(files)
}

fn collect_matching_local_files(
    data_room_root: &Path,
    search_root: &Path,
    files: &mut Vec<MatchedDealFile>,
) -> Result<(), String> {
    for entry in WalkDir::new(search_root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || is_ignored_source_file(entry.path()) {
            continue;
        }
        let path = entry.path();
        let metadata = fs::metadata(path)
            .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?;
        if metadata.len() == 0 {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
            .to_string();
        let matched_on = matching_terms(&filename);
        if matched_on.is_empty() {
            continue;
        }
        files.push(MatchedDealFile {
            source_file: DealSourceFile {
                path: path.display().to_string(),
                filename,
                relative_path: display_relative_path(data_room_root, path),
                size_bytes: metadata.len(),
                matched_on,
                text_extracted: false,
                text_truncated: false,
            },
            data_base64: None,
            mime_type: infer_supported_mime_type(path).map(str::to_string),
        });
    }
    Ok(())
}

fn load_local_selected_file(root: &Path, path: &Path) -> Result<MatchedDealFile, String> {
    validate_data_room_root(root)?;
    if !path.is_file() {
        return Err(format!(
            "selected file does not exist or is not a file: {}",
            path.display()
        ));
    }
    if is_ignored_source_file(path) {
        return Err(format!(
            "selected file is a temporary or system file: {}",
            path.display()
        ));
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|err| format!("failed to resolve data room folder: {err}"))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|err| format!("failed to resolve selected file: {err}"))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "selected file is outside the deal data room: {}",
            path.display()
        ));
    }
    let mime_type = infer_supported_mime_type(path)
        .ok_or_else(|| format!("selected file type is unsupported: {}", path.display()))?;
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    if bytes.is_empty() {
        return Err(format!("selected file is empty: {}", path.display()));
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
        .to_string();
    Ok(MatchedDealFile {
        source_file: DealSourceFile {
            path: path.display().to_string(),
            filename: filename.clone(),
            relative_path: display_relative_path(root, path),
            size_bytes: bytes.len() as u64,
            matched_on: matching_terms(&filename),
            text_extracted: false,
            text_truncated: false,
        },
        data_base64: Some(general_purpose::STANDARD.encode(bytes)),
        mime_type: Some(mime_type.to_string()),
    })
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
    let attachable = files
        .iter()
        .filter(|file| file.data_base64.is_some() && file.mime_type.is_some())
        .collect::<Vec<_>>();
    if attachable.is_empty() {
        return Err("none of the selected files can be sent for extraction".to_string());
    }
    let api_key = openai_api_key()?;
    let client = OpenAiClient::new(&api_key);
    let model = env::var("OPENAI_DEAL_EXTRACTION_MODEL")
        .unwrap_or_else(|_| DEFAULT_DEAL_EXTRACTION_MODEL.to_string());
    let prompt = build_deal_extraction_prompt(deal, &attachable);
    let file_inputs = attachable
        .iter()
        .map(|file| ResponsesFileInput::FileData {
            filename: &file.source_file.filename,
            mime_type: file
                .mime_type
                .as_deref()
                .unwrap_or("application/octet-stream"),
            data_base64: file.data_base64.as_deref().unwrap_or_default(),
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

fn build_deal_extraction_prompt(deal: &Deal, files: &[&MatchedDealFile]) -> String {
    let manifest = files
        .iter()
        .map(|file| {
            format!(
                "- {} ({}, {} bytes, matched on: {})",
                file.source_file.relative_path,
                file.mime_type.as_deref().unwrap_or("unknown"),
                file.source_file.size_bytes,
                file.source_file.matched_on.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Deal metadata:\nDeal name: {}\nDeal type: {}\nPE firm: {}\nTarget company: {}\nBuyer/platform company: {}\nParent/seller company: {}\nCarve-out business: {}\n\n\
Review the attached files. Extract only questions explicitly listed beneath a section heading labeled Key Questions or Key Diligence Questions in an attached Word document. Do not create, infer, rewrite, synthesize, or add questions.\n\n\
Return strict JSON with exactly one key: \"keyQuestions\".\n\n\
Rules:\n- keyQuestions must contain only verbatim or near-verbatim questions from those labeled sections.\n- Do not include questions from unlabeled sections, timelines, risks, assumptions, dependencies, milestones, workstreams, or next steps.\n- If no attached Word document has one of those sections, return an empty keyQuestions array.\n- Use attached files as the source of truth.\n- Do not include Markdown, commentary, citations, or extra keys.\n\nAttached file manifest:\n{}",
        deal.deal_name,
        deal.deal_type,
        deal.pe_firm,
        deal.target_company.as_deref().unwrap_or(""),
        deal.buyer_or_platform_company.as_deref().unwrap_or(""),
        deal.parent_or_seller_company.as_deref().unwrap_or(""),
        deal.carve_out_business.as_deref().unwrap_or(""),
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

fn uploaded_source_file(file: &UploadedDealFile) -> Option<DealSourceFile> {
    let matched_on = matching_terms(&file.filename);
    if matched_on.is_empty() {
        return None;
    }
    Some(DealSourceFile {
        path: file.relative_path.clone(),
        filename: file.filename.clone(),
        relative_path: file.relative_path.clone(),
        size_bytes: file.bytes.len() as u64,
        matched_on,
        text_extracted: false,
        text_truncated: false,
    })
}

fn measure_data_room(root: &Path) -> Result<(i64, i64), String> {
    validate_data_room_root(root)?;
    let mut count = 0_i64;
    let mut size = 0_i64;
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || is_ignored_source_file(entry.path()) {
            continue;
        }
        let file_size = i64::try_from(
            fs::metadata(entry.path())
                .map_err(|err| format!("failed to inspect {}: {err}", entry.path().display()))?
                .len(),
        )
        .map_err(|_| "data room file size exceeds supported range".to_string())?;
        count += 1;
        size = size
            .checked_add(file_size)
            .ok_or_else(|| "data room size exceeds supported range".to_string())?;
    }
    Ok((count, size))
}

fn admin_search_roots(root: &Path) -> Vec<std::path::PathBuf> {
    let mut roots = WalkDir::new(root)
        .min_depth(1)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| {
                    name.to_ascii_lowercase()
                        .replace(['.', '_', '-'], " ")
                        .split_whitespace()
                        .any(|part| part == "admin" || part == "administration")
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

fn has_source_type(files: &[MatchedDealFile], source_type: &str) -> bool {
    files.iter().any(|file| {
        file.source_file
            .matched_on
            .iter()
            .any(|item| item == source_type)
    })
}

fn matching_terms(content: &str) -> Vec<String> {
    let haystack = content.to_ascii_lowercase();
    let mut matches = Vec::new();
    if SOW_MATCH_TERMS.iter().any(|term| haystack.contains(term)) {
        matches.push("SOW".to_string());
    }
    if PROJECT_TIMELINE_MATCH_TERMS
        .iter()
        .any(|term| haystack.contains(term))
    {
        matches.push("Project Timeline".to_string());
    }
    matches
}

fn is_ignored_source_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("~$") || name == ".DS_Store")
        .unwrap_or(false)
}

fn trim_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn safe_root_label(value: &str) -> String {
    value
        .trim()
        .split(['/', '\\'])
        .find(|part| !part.is_empty() && *part != "." && *part != "..")
        .unwrap_or("data-room")
        .replace(':', "-")
}

#[cfg(test)]
#[path = "../../tests/services/deal_service_tests.rs"]
mod tests;
