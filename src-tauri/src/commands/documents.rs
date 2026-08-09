use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::{
    commands::{CommandResult, CommandResultExt},
    core::helix_queries::files::search_quarry_file::{
        ChunkKeywordSearch, ChunkVectorSearch, MAX_SEARCH_LIMIT,
    },
    document_jobs::{DocumentJobEvent, DOCUMENT_JOB_EVENT},
    errors::AppError,
    repository::document_repository::{
        search_document_chunks_by_keyword as repository_keyword_search,
        search_document_chunks_by_vector as repository_vector_search, DocumentChunkSearchResult,
    },
    services::document_service::process_local_document,
    state::AppState,
};

pub const MAX_DOCUMENT_COUNT: usize = 20;
pub const EXPECTED_EMBEDDING_DIMENSION: usize = 1536;
const MAX_DOCUMENT_BYTES: u64 = 50 * 1024 * 1024;
const MAX_DOCUMENT_BATCH_BYTES: u64 = 50 * 1024 * 1024;
const MAX_KEYWORD_QUERY_CHARS: usize = 500;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDocumentJobsInput {
    pub user_id: String,
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedLocalFile {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDocumentJobsResponse {
    pub jobs: Vec<DocumentJobEvent>,
}

#[derive(Debug)]
struct ValidatedDocumentFile {
    name: String,
    path: PathBuf,
}

#[tauri::command]
pub async fn select_document_files(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Vec<SelectedLocalFile>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let selections = app
            .dialog()
            .file()
            .set_title("Choose PDF or DOCX files")
            .add_filter("Documents", &["pdf", "docx"])
            .blocking_pick_files()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(selections.len());
        let mut grants = Vec::with_capacity(selections.len());
        for selection in selections {
            let path = selection
                .into_path()
                .map_err(|_| "a selected file path is invalid".to_string())?
                .canonicalize()
                .map_err(|_| "a selected file is no longer available".to_string())?;
            let metadata = path
                .metadata()
                .map_err(|_| "a selected file is no longer available".to_string())?;
            let name = safe_file_name(&path)?;
            grants.push(path.clone());
            result.push(SelectedLocalFile {
                name,
                path: path.display().to_string(),
                size_bytes: metadata.len(),
            });
        }
        state.grant_paths(grants)?;
        Ok(result)
    })
    .await
    .map_err(|error| format!("native file picker worker failed: {error}"))
    .and_then(|result| result)
    .command_context("select_document_files")
}

#[tauri::command]
pub async fn describe_document_files(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> CommandResult<Vec<SelectedLocalFile>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || describe_granted_document_files(&state, paths))
        .await
        .map_err(|error| format!("dropped-file validation worker failed: {error}"))
        .and_then(|result| result)
        .validation_context("describe_document_files")
}

#[tauri::command]
pub async fn start_document_jobs(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StartDocumentJobsInput,
) -> CommandResult<StartDocumentJobsResponse> {
    let state = state.inner().clone();
    let validation_state = state.clone();
    let user_id = input.user_id.trim().to_string();
    let files = tauri::async_runtime::spawn_blocking(move || {
        validate_start_request(&validation_state, input)
    })
    .await
    .map_err(|error| format!("document validation worker failed: {error}"))
    .and_then(|result| result)
    .validation_context("start_document_jobs")?;

    let mut jobs = Vec::with_capacity(files.len());
    for file in files {
        let job = DocumentJobEvent::processing(Uuid::new_v4().to_string(), file.name.clone());
        state.document_jobs().insert(job.clone()).await;
        jobs.push(job.clone());

        let worker_app = app.clone();
        let worker_state = state.clone();
        let worker_user_id = user_id.clone();
        tauri::async_runtime::spawn(async move {
            let _processing_slot = worker_state.document_jobs().acquire_processing_slot().await;
            let event = match process_local_document(&worker_state, &file.path, &worker_user_id)
                .await
            {
                Ok(processed) if processed.skipped => {
                    DocumentJobEvent::skipped(job.job_id, job.filename, Some(processed.document_id))
                }
                Ok(processed) => DocumentJobEvent::completed(
                    job.job_id,
                    job.filename,
                    Some(processed.document_id),
                    processed.chunk_count,
                ),
                Err(_) => DocumentJobEvent::failed(
                    job.job_id,
                    job.filename,
                    "Document processing failed. Check service availability and try again."
                        .to_string(),
                ),
            };
            worker_state.document_jobs().update(event.clone()).await;
            if let Err(error) = worker_app.emit_to("main", DOCUMENT_JOB_EVENT, event) {
                eprintln!("failed to emit document job update: {error}");
            }
        });
    }

    Ok(StartDocumentJobsResponse { jobs })
}

#[tauri::command]
pub async fn get_document_job(
    state: State<'_, AppState>,
    job_id: String,
) -> CommandResult<DocumentJobEvent> {
    Uuid::parse_str(job_id.trim())
        .map_err(|_| AppError::validation("get_document_job", "jobId must be a UUID"))?;
    state
        .document_jobs()
        .get(job_id.trim())
        .await
        .ok_or_else(|| AppError::not_found("get_document_job", "Document job was not found."))
}

#[tauri::command]
pub async fn search_document_chunks_keyword(
    state: State<'_, AppState>,
    input: ChunkKeywordSearch,
) -> CommandResult<Vec<DocumentChunkSearchResult>> {
    validate_keyword_search(&input).validation_context("search_document_chunks_keyword")?;
    repository_keyword_search(state.gen_helix_db_client(), input)
        .await
        .command_context("search_document_chunks_keyword")
}

#[tauri::command]
pub async fn search_document_chunks_vector(
    state: State<'_, AppState>,
    input: ChunkVectorSearch,
) -> CommandResult<Vec<DocumentChunkSearchResult>> {
    validate_vector_search(&input).validation_context("search_document_chunks_vector")?;
    repository_vector_search(state.gen_helix_db_client(), input)
        .await
        .command_context("search_document_chunks_vector")
}

fn validate_keyword_search(input: &ChunkKeywordSearch) -> Result<(), String> {
    validate_search_scope(&input.user_id, input.limit)?;
    if input.query_text.trim().is_empty() {
        return Err("queryText is required".to_string());
    }
    if input.query_text.chars().count() > MAX_KEYWORD_QUERY_CHARS {
        return Err(format!(
            "queryText cannot exceed {MAX_KEYWORD_QUERY_CHARS} characters"
        ));
    }
    Ok(())
}

fn validate_vector_search(input: &ChunkVectorSearch) -> Result<(), String> {
    validate_search_scope(&input.user_id, input.limit)?;
    if input.query_embedding.len() != EXPECTED_EMBEDDING_DIMENSION {
        return Err(format!(
            "queryEmbedding must contain {EXPECTED_EMBEDDING_DIMENSION} values"
        ));
    }
    if input.query_embedding.iter().any(|value| !value.is_finite()) {
        return Err("queryEmbedding must contain only finite values".to_string());
    }
    Ok(())
}

fn validate_search_scope(user_id: &str, limit: usize) -> Result<(), String> {
    if user_id.trim().is_empty() {
        return Err("userId is required".to_string());
    }
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(format!("limit must be between 1 and {MAX_SEARCH_LIMIT}"));
    }
    Ok(())
}

fn validate_start_request(
    state: &AppState,
    input: StartDocumentJobsInput,
) -> Result<Vec<ValidatedDocumentFile>, String> {
    if input.user_id.trim().is_empty() {
        return Err("userId is required".to_string());
    }
    if input.paths.is_empty() {
        return Err("select at least one PDF or DOCX document".to_string());
    }
    if input.paths.len() > MAX_DOCUMENT_COUNT {
        return Err(format!(
            "select no more than {MAX_DOCUMENT_COUNT} documents at once"
        ));
    }

    let mut total_bytes = 0_u64;
    let mut files = Vec::with_capacity(input.paths.len());
    for raw_path in input.paths {
        let path = PathBuf::from(raw_path)
            .canonicalize()
            .map_err(|_| "a selected document is no longer available".to_string())?;
        let name = safe_file_name(&path)?;
        if !state.is_path_granted(&path)? {
            return Err(format!(
                "{name} is not authorized by the native file picker"
            ));
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if !matches!(extension.as_str(), "pdf" | "docx") {
            return Err(format!("{name} is not a PDF or DOCX document"));
        }
        let metadata = path
            .metadata()
            .map_err(|_| format!("{name} is no longer available"))?;
        if !metadata.is_file() {
            return Err(format!("{name} is not a file"));
        }
        if metadata.len() == 0 {
            return Err(format!("{name} is empty"));
        }
        if metadata.len() > MAX_DOCUMENT_BYTES {
            return Err(format!("{name} exceeds the 50 MB file limit"));
        }
        total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| "selected document sizes exceed the supported range".to_string())?;
        if total_bytes > MAX_DOCUMENT_BATCH_BYTES {
            return Err("selected documents exceed the 50 MB batch limit".to_string());
        }
        files.push(ValidatedDocumentFile { name, path });
    }
    Ok(files)
}

fn describe_granted_document_files(
    state: &AppState,
    paths: Vec<String>,
) -> Result<Vec<SelectedLocalFile>, String> {
    if paths.is_empty() {
        return Err("drop at least one PDF or DOCX document".to_string());
    }
    if paths.len() > MAX_DOCUMENT_COUNT {
        return Err(format!(
            "drop no more than {MAX_DOCUMENT_COUNT} documents at once"
        ));
    }

    let mut total_bytes = 0_u64;
    let mut files = Vec::with_capacity(paths.len());
    for raw_path in paths {
        let path = PathBuf::from(raw_path)
            .canonicalize()
            .map_err(|_| "a dropped document is no longer available".to_string())?;
        let name = safe_file_name(&path)?;
        if !state.is_path_granted(&path)? {
            return Err(format!("{name} was not received from a native file drop"));
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if !matches!(extension.as_str(), "pdf" | "docx") {
            return Err(format!("{name} is not a PDF or DOCX document"));
        }
        let metadata = path
            .metadata()
            .map_err(|_| format!("{name} is no longer available"))?;
        if !metadata.is_file() {
            return Err(format!("{name} is not a file"));
        }
        let size_bytes = metadata.len();
        if size_bytes == 0 {
            return Err(format!("{name} is empty"));
        }
        if size_bytes > MAX_DOCUMENT_BYTES {
            return Err(format!("{name} is larger than 50 MB"));
        }
        total_bytes = total_bytes
            .checked_add(size_bytes)
            .ok_or_else(|| "the dropped file size is too large".to_string())?;
        if total_bytes > MAX_DOCUMENT_BATCH_BYTES {
            return Err("the dropped files exceed the 50 MB batch limit".to_string());
        }
        files.push(SelectedLocalFile {
            name,
            path: path.display().to_string(),
            size_bytes,
        });
    }
    Ok(files)
}

fn safe_file_name(path: &std::path::Path) -> Result<String, String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| "a selected document has an invalid filename".to_string())
}

#[cfg(test)]
#[path = "../../tests/commands/document_tests.rs"]
mod tests;
