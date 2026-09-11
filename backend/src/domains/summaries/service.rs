use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use base64::{engine::general_purpose, Engine};
use serde::Serialize;
use walkdir::WalkDir;

use crate::{
    adapters::openai::client::{OpenAiClient, ResponsesFileInput},
    domains::summaries::prompt::{build_document_summary_prompt, DOCUMENT_SUMMARY_SYSTEM_PROMPT},
    shared::{
        error::{ServiceError, ServiceResult},
        file_policy::{
            display_relative_path, infer_supported_mime_type, write_summary, CollectedFile,
            MAX_FILE_BYTES, MAX_TOTAL_REQUEST_FILE_BYTES,
        },
    },
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummarizableFile {
    pub path: String,
    pub filename: String,
    pub relative_path: String,
    pub mime_type: String,
    pub size_bytes: usize,
    pub supported: bool,
}

pub struct SummaryService {
    openai: Option<Arc<OpenAiClient>>,
    model: String,
}

impl SummaryService {
    pub fn new(openai: Option<Arc<OpenAiClient>>, model: String) -> Self {
        Self { openai, model }
    }

    pub async fn summarize_dir(&self, path: String) -> ServiceResult<String> {
        let root = PathBuf::from(&path);
        if !root.exists() {
            return Err(ServiceError::validation(format!(
                "path does not exist: {}",
                root.display()
            )));
        }
        if !root.is_dir() {
            return Err(ServiceError::validation(format!(
                "path is not a directory: {}",
                root.display()
            )));
        }
        let (files, skipped_files) =
            collect_dir_content(&root).map_err(ServiceError::validation)?;
        self.summarize_collected_files(&path, files, skipped_files)
            .await
    }

    pub fn list_files(&self, path: String) -> ServiceResult<Vec<SummarizableFile>> {
        let root = PathBuf::from(&path);
        if !root.exists() {
            return Err(ServiceError::validation(format!(
                "path does not exist: {}",
                root.display()
            )));
        }
        if !root.is_dir() {
            return Err(ServiceError::validation(format!(
                "path is not a directory: {}",
                root.display()
            )));
        }

        let mut files = Vec::new();
        for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.into_path();
            let metadata = fs::metadata(&path).map_err(|error| {
                ServiceError::validation(format!(
                    "failed to read metadata for {}: {error}",
                    path.display()
                ))
            })?;
            if metadata.len() == 0 {
                continue;
            }
            let mime_type = infer_supported_mime_type(&path);
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    ServiceError::validation(format!(
                        "failed to derive filename from {}",
                        path.display()
                    ))
                })?
                .to_string();
            files.push(SummarizableFile {
                path: path.display().to_string(),
                filename,
                relative_path: display_relative_path(&root, &path),
                mime_type: mime_type.unwrap_or("unsupported").to_string(),
                size_bytes: metadata.len() as usize,
                supported: mime_type.is_some(),
            });
        }
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(files)
    }

    pub async fn summarize_paths(&self, paths: Vec<String>) -> ServiceResult<String> {
        if paths.is_empty() {
            return Err(ServiceError::validation("no files selected for summary"));
        }
        let selected_paths = paths.into_iter().map(PathBuf::from).collect::<Vec<_>>();
        let root = common_parent_path(&selected_paths).unwrap_or_default();
        let (files, skipped_files) =
            collect_paths_content(&root, &selected_paths).map_err(ServiceError::validation)?;
        self.summarize_collected_files(&root.display().to_string(), files, skipped_files)
            .await
    }

    pub async fn summarize_collected_files(
        &self,
        root_label: &str,
        files: Vec<CollectedFile>,
        skipped_files: Vec<String>,
    ) -> ServiceResult<String> {
        if files.is_empty() {
            return Err(ServiceError::validation(format!(
                "no supported files found in {root_label}"
            )));
        }
        let client = self
            .openai
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("OpenAI capability is not configured"))?;
        let root = Path::new(root_label);
        let prompt = build_document_summary_prompt(root, &files, &skipped_files);
        let file_inputs: Vec<ResponsesFileInput<'_>> = files
            .iter()
            .map(|file| ResponsesFileInput::FileData {
                filename: file.filename.as_str(),
                mime_type: file.mime_type,
                data_base64: file.data_base64.as_str(),
            })
            .collect();

        client
            .gen_model_response_with_files(
                Some(&prompt),
                Some(DOCUMENT_SUMMARY_SYSTEM_PROMPT),
                Some(&self.model),
                Some(&file_inputs),
            )
            .await
            .map_err(ServiceError::internal)
    }

    pub fn save_markdown(&self, summary: &str, path: String) -> ServiceResult<()> {
        write_summary(summary, path).map_err(ServiceError::validation)
    }
}

fn collect_dir_content(root: &Path) -> Result<(Vec<CollectedFile>, Vec<String>), String> {
    let mut files = Vec::new();
    let mut skipped_files = Vec::new();
    let mut total_file_bytes = 0usize;
    let mut total_limit_reached = false;

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.into_path();
        let relative_path = display_relative_path(root, &path);
        let Some(mime_type) = infer_supported_mime_type(&path) else {
            skipped_files.push(relative_path);
            continue;
        };
        println!("{}", path.display());

        if total_limit_reached {
            skipped_files.push(format!(
                "{relative_path} (skipped: total request file size limit already reached)"
            ));
            continue;
        }

        let file_size_bytes = fs::metadata(&path)
            .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?
            .len() as usize;
        if file_size_bytes == 0 {
            skipped_files.push(format!("{relative_path} (empty)"));
            continue;
        }

        if file_size_bytes > MAX_FILE_BYTES {
            skipped_files.push(format!(
                "{relative_path} (skipped: file exceeds 50 MB limit)"
            ));
            continue;
        }

        if total_file_bytes + file_size_bytes > MAX_TOTAL_REQUEST_FILE_BYTES {
            skipped_files.push(format!(
                "{relative_path} (skipped: total request file size would exceed 50 MB limit)"
            ));
            total_limit_reached = true;
            continue;
        }

        let file_bytes =
            fs::read(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        if file_bytes.is_empty() {
            skipped_files.push(format!("{relative_path} (empty)"));
            continue;
        }

        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
            .to_string();

        files.push(CollectedFile {
            filename,
            relative_path,
            mime_type,
            size_bytes: file_size_bytes,
            data_base64: general_purpose::STANDARD.encode(file_bytes),
        });
        total_file_bytes += file_size_bytes;
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    skipped_files.sort();

    Ok((files, skipped_files))
}

fn collect_paths_content(
    root: &Path,
    paths: &[PathBuf],
) -> Result<(Vec<CollectedFile>, Vec<String>), String> {
    let mut files = Vec::new();
    let mut skipped_files = Vec::new();
    let mut total_file_bytes = 0usize;

    for path in paths {
        let relative_path = display_relative_path(root, path);
        if !path.is_file() {
            skipped_files.push(format!("{relative_path} (missing or not a file)"));
            continue;
        }
        let Some(mime_type) = infer_supported_mime_type(path) else {
            skipped_files.push(format!("{relative_path} (unsupported)"));
            continue;
        };
        let file_size_bytes = fs::metadata(path)
            .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?
            .len() as usize;
        if file_size_bytes == 0 {
            skipped_files.push(format!("{relative_path} (empty)"));
            continue;
        }
        if file_size_bytes > MAX_FILE_BYTES {
            skipped_files.push(format!("{relative_path} (file exceeds 50 MB limit)"));
            continue;
        }
        if total_file_bytes + file_size_bytes > MAX_TOTAL_REQUEST_FILE_BYTES {
            skipped_files.push(format!(
                "{relative_path} (total request exceeds 50 MB limit)"
            ));
            continue;
        }
        let bytes =
            fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
            .to_string();
        files.push(CollectedFile {
            filename,
            relative_path,
            mime_type,
            size_bytes: file_size_bytes,
            data_base64: general_purpose::STANDARD.encode(bytes),
        });
        total_file_bytes += file_size_bytes;
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    skipped_files.sort();
    Ok((files, skipped_files))
}

fn common_parent_path(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut components = paths.first()?.parent()?.components().collect::<Vec<_>>();
    for path in paths.iter().skip(1) {
        let parent_components = path.parent()?.components().collect::<Vec<_>>();
        let shared_len = components
            .iter()
            .zip(parent_components.iter())
            .take_while(|(left, right)| left == right)
            .count();
        components.truncate(shared_len);
    }
    let mut root = PathBuf::new();
    for component in components {
        root.push(component.as_os_str());
    }
    Some(root)
}
