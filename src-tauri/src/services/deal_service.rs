use std::{env, fs, path::Path};

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{
    core::{
        clients::openai::{OpenAiClient, ResponsesFileInput},
        display_relative_path, infer_supported_mime_type,
    },
    repository::deal_repository::{
        create_deal, get_deal_by_id, upsert_deal_metadata, CreateDealRecord, Deal, DealMetadata,
        UpsertDealMetadataRecord,
    },
    state::AppState,
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealExtraction {
    pub key_questions: Vec<String>,
    pub investment_thesis: String,
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
pub struct ExtractDealQuestionsAndThesisInput {
    pub deal_id: i64,
    pub sow_file_path: Option<String>,
    pub project_timeline_file_path: Option<String>,
}

struct MatchedDealFile {
    source_file: DealSourceFile,
    data_base64: Option<String>,
    mime_type: Option<&'static str>,
}

pub async fn save_deal_and_extract(
    state: &AppState,
    mut input: SaveDealAndExtractInput,
) -> Result<SaveDealAndFindFilesResponse, String> {
    validate_deal_input(&input)?;
    let canonical_root = Path::new(input.main_data_room_folder.trim())
        .canonicalize()
        .map_err(|_| "mainDataRoomFolder is unavailable".to_string())?;
    if !canonical_root.is_dir() {
        return Err("mainDataRoomFolder must be a folder".to_string());
    }
    input.main_data_room_folder = canonical_root.display().to_string();
    let deal = save_deal(state, &input)?;
    let matched_files = discover_sow_and_timeline_files(Path::new(&deal.main_data_room_folder))?;
    let files = matched_files
        .into_iter()
        .map(|file| file.source_file)
        .collect::<Vec<_>>();

    Ok(SaveDealAndFindFilesResponse { deal, files })
}

pub async fn extract_deal_questions_and_thesis_for_selected_files(
    state: &AppState,
    input: ExtractDealQuestionsAndThesisInput,
) -> Result<SaveDealAndExtractResponse, String> {
    if input
        .sow_file_path
        .as_deref()
        .is_none_or(|path| path.trim().is_empty())
    {
        return Err("sowFilePath is required".to_string());
    }
    let deal = get_deal_by_id(state, input.deal_id)?
        .ok_or_else(|| format!("deal not found for id `{}`", input.deal_id))?;
    let matched_files = load_selected_deal_files(&deal, &input)?;
    let extraction = extract_deal_questions_and_thesis_from_files(&deal, &matched_files).await?;
    let metadata = persist_deal_metadata(state, &deal, &extraction)?;
    let files = matched_files
        .into_iter()
        .map(|file| file.source_file)
        .collect::<Vec<_>>();

    Ok(SaveDealAndExtractResponse {
        deal,
        files,
        extraction,
        metadata,
    })
}

fn persist_deal_metadata(
    state: &AppState,
    deal: &Deal,
    extraction: &DealExtraction,
) -> Result<DealMetadata, String> {
    let key_questions_json = serde_json::to_string(&extraction.key_questions)
        .map_err(|err| format!("failed to serialize deal key questions: {err}"))?;
    let (document_count, data_room_size_bytes) =
        measure_data_room(Path::new(&deal.main_data_room_folder))?;

    //Chnange to Helix??
    upsert_deal_metadata(
        state,
        UpsertDealMetadataRecord {
            deal_id: deal.id,
            key_questions_json: &key_questions_json,
            legacy_investment_thesis: (!extraction.investment_thesis.trim().is_empty())
                .then_some(extraction.investment_thesis.as_str()),
            document_count,
            data_room_size_bytes,
        },
    )
}

fn measure_data_room(root: &Path) -> Result<(i64, i64), String> {
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

    let mut document_count = 0_i64;
    let mut data_room_size_bytes = 0_i64;

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if is_ignored_source_file(path) {
            continue;
        }

        let metadata = fs::metadata(path)
            .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?;
        let file_size_bytes = i64::try_from(metadata.len())
            .map_err(|_| "data room file size exceeds supported range".to_string())?;

        document_count += 1;
        data_room_size_bytes = data_room_size_bytes
            .checked_add(file_size_bytes)
            .ok_or_else(|| "data room size exceeds supported range".to_string())?;
    }

    Ok((document_count, data_room_size_bytes))
}

fn save_deal(state: &AppState, input: &SaveDealAndExtractInput) -> Result<Deal, String> {
    save_deal_with_repository(input, |record| create_deal(state, record))
}

fn save_deal_with_repository<'a>(
    input: &'a SaveDealAndExtractInput,
    persist: impl FnOnce(CreateDealRecord<'a>) -> Result<Deal, String>,
) -> Result<Deal, String> {
    let deal_name = input.deal_name.trim();
    let main_data_room_folder = input.main_data_room_folder.trim();
    let deal_type = input.deal_type.trim();
    let pe_firm = input.pe_firm.trim();
    let target_company = trim_optional(input.target_company.as_deref());
    let buyer_or_platform_company = trim_optional(input.buyer_or_platform_company.as_deref());
    let parent_or_seller_company = trim_optional(input.parent_or_seller_company.as_deref());
    let carve_out_business = trim_optional(input.carve_out_business.as_deref());

    persist(CreateDealRecord {
        deal_name,
        main_data_room_folder,
        deal_type,
        pe_firm,
        target_company,
        buyer_or_platform_company,
        parent_or_seller_company,
        carve_out_business,
    })
}

fn discover_sow_and_timeline_files(root: &Path) -> Result<Vec<MatchedDealFile>, String> {
    collect_sow_and_timeline_files_with_options(root, false)
}

fn collect_sow_and_timeline_files_with_options(
    root: &Path,
    include_file_data: bool,
) -> Result<Vec<MatchedDealFile>, String> {
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

    let search_roots = admin_search_roots(root);
    let is_admin_first_search = !search_roots.is_empty();
    let mut files = Vec::new();

    for search_root in &search_roots {
        collect_matching_files_from_root_with_options(
            root,
            search_root,
            &mut files,
            include_file_data,
        )?;
    }

    if is_admin_first_search && !has_required_source_file_types(&files) {
        collect_matching_files_from_root_with_options(root, root, &mut files, include_file_data)?;
    }

    files.sort_by(|left, right| {
        left.source_file
            .relative_path
            .cmp(&right.source_file.relative_path)
    });
    files.dedup_by(|left, right| left.source_file.path == right.source_file.path);

    Ok(files)
}

fn has_required_source_file_types(files: &[MatchedDealFile]) -> bool {
    has_matched_source_type(files, "SOW") && has_matched_source_type(files, "Project Timeline")
}

fn has_matched_source_type(files: &[MatchedDealFile], source_type: &str) -> bool {
    files.iter().any(|file| {
        file.source_file
            .matched_on
            .iter()
            .any(|match_name| match_name == source_type)
    })
}

fn load_selected_deal_files(
    deal: &Deal,
    input: &ExtractDealQuestionsAndThesisInput,
) -> Result<Vec<MatchedDealFile>, String> {
    let selected_paths = selected_deal_file_paths(input)?;
    let data_room_root = Path::new(&deal.main_data_room_folder);

    selected_paths
        .iter()
        .map(|path| build_selected_matched_file(data_room_root, Path::new(path)))
        .collect()
}

fn selected_deal_file_paths(
    input: &ExtractDealQuestionsAndThesisInput,
) -> Result<Vec<&str>, String> {
    let selected_paths = [
        input.sow_file_path.as_deref().map(str::trim),
        input.project_timeline_file_path.as_deref().map(str::trim),
    ];
    let mut paths = Vec::new();

    for path in selected_paths.into_iter().flatten() {
        if path.is_empty() {
            continue;
        }

        if !paths.contains(&path) {
            paths.push(path);
        }
    }

    Ok(paths)
}

fn build_selected_matched_file(
    data_room_root: &Path,
    path: &Path,
) -> Result<MatchedDealFile, String> {
    if !path.exists() {
        return Err(format!("selected file does not exist: {}", path.display()));
    }

    if !path.is_file() {
        return Err(format!("selected path is not a file: {}", path.display()));
    }

    if is_ignored_source_file(path) {
        return Err(format!(
            "selected file is a temporary or system file: {}",
            path.display()
        ));
    }

    let canonical_root = data_room_root
        .canonicalize()
        .map_err(|err| format!("failed to resolve data room folder: {err}"))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|err| format!("failed to resolve selected file {}: {err}", path.display()))?;

    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "selected file is outside the deal data room: {}",
            path.display()
        ));
    }

    let metadata = fs::metadata(path)
        .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?;
    if metadata.len() == 0 {
        return Err(format!("selected file is empty: {}", path.display()));
    }

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
        .to_string();
    let mut matched_on = matching_terms(&filename);
    matched_on.sort();
    matched_on.dedup();

    Ok(MatchedDealFile {
        data_base64: encode_supported_file(path).transpose()?,
        mime_type: infer_supported_mime_type(path),
        source_file: DealSourceFile {
            path: path.display().to_string(),
            filename,
            relative_path: display_relative_path(data_room_root, path),
            size_bytes: metadata.len(),
            matched_on,
            text_extracted: false,
            text_truncated: false,
        },
    })
}

fn collect_matching_files_from_root_with_options(
    data_room_root: &Path,
    search_root: &Path,
    files: &mut Vec<MatchedDealFile>,
    include_file_data: bool,
) -> Result<(), String> {
    for entry in WalkDir::new(search_root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if is_ignored_source_file(path) {
            continue;
        }

        let metadata = fs::metadata(path)
            .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?;
        if metadata.len() == 0 {
            continue;
        }

        let relative_path = display_relative_path(data_room_root, path);
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
            .to_string();
        let mut matched_on = matching_terms(&filename);
        matched_on.sort();
        matched_on.dedup();

        if matched_on.is_empty() {
            continue;
        }

        let data_base64 = if include_file_data {
            encode_supported_file(path).transpose()?
        } else {
            None
        };

        files.push(MatchedDealFile {
            data_base64,
            mime_type: infer_supported_mime_type(path),
            source_file: DealSourceFile {
                path: path.display().to_string(),
                filename,
                relative_path,
                size_bytes: metadata.len(),
                matched_on,
                text_extracted: false,
                text_truncated: false,
            },
        });
    }

    Ok(())
}

fn admin_search_roots(root: &Path) -> Vec<std::path::PathBuf> {
    let mut admin_roots = WalkDir::new(root)
        .min_depth(1)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(is_admin_folder_name)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    admin_roots.sort();
    admin_roots.dedup();

    admin_roots
}

fn is_admin_folder_name(folder_name: &str) -> bool {
    let normalized = folder_name
        .to_ascii_lowercase()
        .replace(['.', '_', '-'], " ");

    normalized
        .split_whitespace()
        .any(|part| part == "admin" || part == "administration")
}

async fn extract_deal_questions_and_thesis_from_files(
    deal: &Deal,
    files: &[MatchedDealFile],
) -> Result<DealExtraction, String> {
    if files.is_empty() {
        return Ok(DealExtraction {
            key_questions: Vec::new(),
            investment_thesis: String::new(),
        });
    }

    let attachable_files = files
        .iter()
        .filter(|file| {
            file.mime_type.is_some()
                && file
                    .data_base64
                    .as_deref()
                    .map(|data| !data.trim().is_empty())
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    if attachable_files.is_empty() {
        return Ok(DealExtraction {
            key_questions: Vec::new(),
            investment_thesis: String::new(),
        });
    }

    let client = OpenAiClient::new()?;
    let model = env::var("OPENAI_DEAL_EXTRACTION_MODEL")
        .unwrap_or_else(|_| DEFAULT_DEAL_EXTRACTION_MODEL.to_string());
    let prompt = build_deal_extraction_prompt(deal, &attachable_files);
    let file_inputs = attachable_files
        .iter()
        .filter_map(|file| {
            Some(ResponsesFileInput::FileData {
                filename: file.source_file.filename.as_str(),
                mime_type: file.mime_type?,
                data_base64: file.data_base64.as_deref()?,
            })
        })
        .collect::<Vec<_>>();
    let response = client
        .gen_model_response_with_files_and_reasoning(
            Some(&prompt),
            Some("You extract private equity diligence outputs from deal documents. Return only strict JSON with no Markdown."),
            Some(&model),
            Some(&file_inputs),
            Some("none"),
        )
        .await?;

    parse_deal_extraction(&response)
}

fn build_deal_extraction_prompt(deal: &Deal, files: &[&MatchedDealFile]) -> String {
    let file_manifest = files
        .iter()
        .map(|file| {
            format!(
                "- {} ({}, {} bytes, matched on: {})",
                file.source_file.relative_path,
                file.mime_type.unwrap_or("unknown"),
                file.source_file.size_bytes,
                file.source_file.matched_on.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let narrative_rule = if deal.deal_type == "Sell-side" {
        let company_name = deal
            .target_company
            .as_deref()
            .filter(|company| !company.trim().is_empty())
            .unwrap_or("the company");
        format!(
            "- investmentThesis must contain a concise equity story for {company_name}. Base it only on the attached files and deal metadata. Focus on the company's buyer-facing value proposition, growth story, differentiation, and reasons a buyer should care.\n"
        )
    } else {
        "- Always return investmentThesis as an empty string.\n".to_string()
    };

    format!(
        "Deal metadata:\n\
Deal name: {}\n\
Deal type: {}\n\
PE firm: {}\n\
Target company: {}\n\
Buyer/platform company: {}\n\
Parent/seller company: {}\n\
Carve-out business: {}\n\n\
Review the attached files listed below. Extract only the questions that are explicitly labeled as key questions in the attached Word document. The relevant section heading must be labeled Key Questions or Key Diligence Questions. Do not create, infer, rewrite, synthesize, or add any key questions of your own.\n\n\
Return strict JSON with exactly these keys: \"keyQuestions\" and \"investmentThesis\".\n\n\
Rules:\n\
- keyQuestions must contain only verbatim or near-verbatim questions found under a Key Questions or Key Diligence Questions label in the attached Word document.\n\
- Do not include questions from unlabeled sections, timelines, risks, assumptions, dependencies, milestones, workstreams, or next steps.\n\
- Do not infer questions from scope or timeline content.\n\
- If no attached Word document has a Key Questions or Key Diligence Questions section, return an empty keyQuestions array.\n\
{}\
- Use the attached files as the source of truth.\n\
- Do not include Markdown, commentary, citations, or extra keys.\n\n\
Attached file manifest:\n{}",
        deal.deal_name,
        deal.deal_type,
        deal.pe_firm,
        deal.target_company.as_deref().unwrap_or(""),
        deal.buyer_or_platform_company.as_deref().unwrap_or(""),
        deal.parent_or_seller_company.as_deref().unwrap_or(""),
        deal.carve_out_business.as_deref().unwrap_or(""),
        narrative_rule,
        file_manifest
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

    serde_json::from_str::<DealExtraction>(json_text)
        .map_err(|err| format!("failed to parse deal extraction JSON: {err}; response: {response}"))
}

fn encode_supported_file(path: &Path) -> Option<Result<String, String>> {
    infer_supported_mime_type(path)?;

    Some(
        fs::read(path)
            .map(|bytes| general_purpose::STANDARD.encode(bytes))
            .map_err(|err| {
                format!(
                    "failed to read {} for OpenAI request: {err}",
                    path.display()
                )
            }),
    )
}

fn is_ignored_source_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|filename| filename.starts_with("~$"))
        .unwrap_or(false)
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

fn validate_deal_input(input: &SaveDealAndExtractInput) -> Result<(), String> {
    if input.deal_name.trim().is_empty() {
        return Err("dealName is required".to_string());
    }

    if input.main_data_room_folder.trim().is_empty() {
        return Err("mainDataRoomFolder is required".to_string());
    }

    let deal_type = input.deal_type.trim();
    if deal_type.is_empty() {
        return Err("dealType is required".to_string());
    }

    if ![
        "Buy-side",
        "Sell-side",
        "Carve-out",
        "Add-on",
        "Recapitalization",
        "Growth equity",
    ]
    .contains(&deal_type)
    {
        return Err("dealType is not supported".to_string());
    }

    if input.pe_firm.trim().is_empty() {
        return Err("peFirm is required".to_string());
    }

    let target_company = trim_optional(input.target_company.as_deref());
    let buyer_or_platform_company = trim_optional(input.buyer_or_platform_company.as_deref());
    let parent_or_seller_company = trim_optional(input.parent_or_seller_company.as_deref());
    let carve_out_business = trim_optional(input.carve_out_business.as_deref());

    match deal_type {
        "Sell-side" | "Recapitalization" | "Growth equity" if target_company.is_none() => {
            Err(format!("targetCompany is required for {deal_type} deals"))
        }
        "Buy-side" | "Add-on" if target_company.is_none() => {
            Err(format!("targetCompany is required for {deal_type} deals"))
        }
        "Buy-side" | "Add-on" if buyer_or_platform_company.is_none() => Err(format!(
            "buyerOrPlatformCompany is required for {deal_type} deals"
        )),
        "Carve-out" if parent_or_seller_company.is_none() => {
            Err("parentOrSellerCompany is required for Carve-out deals".to_string())
        }
        "Carve-out" if carve_out_business.is_none() => {
            Err("carveOutBusiness is required for Carve-out deals".to_string())
        }
        _ => Ok(()),
    }
}

fn trim_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../tests/services/deal_service_tests.rs"]
mod tests;
