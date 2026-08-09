use crate::core::{
    build_summary_prompt,
    clients::openai::{OpenAiClient, ResponsesFileInput},
    display_relative_path, infer_supported_mime_type,
    models::document::ParsedFileData,
    parsers::docx::parse_docx_from_path as parse_docx_from_path_in_parser,
    CollectedFile,
};
use crate::prompts::{
    DATA_ROOM_TECH_DILIGENCE_SUMMARY_PROMPT, DOCUMENT_SUMMARY_SYSTEM_PROMPT,
    PRODUCT_AND_APPLICATION_DEEP_DIVE_PROMPT,
};
use base64::engine::general_purpose;
use base64::Engine;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::{env, fs};
use walkdir::WalkDir;

pub enum ParsedFile {
    Docx(ParsedFileData),
    PPTX(ParsedFileData),
    Spreadsheet(ParsedFileData),
    Image(ParsedFileData),
}

pub enum DirectoryFile {
    Docx(ParsedFileData),
    PPTX(ParsedFileData),
    Spreadsheet(ParsedFileData),
    Image(ParsedFileData),
}

const DEFAULT_DOCUMENT_SUMMARY_MODEL: &str = "gpt-5.5";
const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
const MAX_TOTAL_REQUEST_FILE_BYTES: usize = 50 * 1024 * 1024;

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

pub fn parse_docx_from_path(path: &Path) -> Result<String, String> {
    parse_docx_from_path_in_parser(path)
}

pub async fn summarize_dir(path: String) -> Result<String, String> {
    let root = PathBuf::from(&path);
    if !root.exists() {
        return Err(format!("path does not exist: {}", root.display()));
    }
    if !root.is_dir() {
        return Err(format!("path is not a directory: {}", root.display()));
    }
    let client = OpenAiClient::new()?;
    let model = env::var("OPENAI_DOCUMENT_SUMMARY_MODEL")
        .unwrap_or_else(|_| DEFAULT_DOCUMENT_SUMMARY_MODEL.to_string());
    let (files, skipped_files) = collect_dir_content(&root)?;
    let prompt = build_document_summary_prompt(&root, &files, &skipped_files);
    let file_inputs: Vec<ResponsesFileInput<'_>> = files
        .iter()
        .map(|file| ResponsesFileInput::FileData {
            filename: file.filename.as_str(),
            mime_type: file.mime_type,
            data_base64: file.data_base64.as_str(),
        })
        .collect();

    let summary = client
        .gen_model_response_with_files(
            Some(&prompt),
            Some(DOCUMENT_SUMMARY_SYSTEM_PROMPT),
            Some(&model),
            Some(&file_inputs),
        )
        .await?;

    println!("{summary}");
    // write_summary(&summary)?;

    if !skipped_files.is_empty() {
        eprintln!("skipped {} unsupported or empty files", skipped_files.len());
    }

    Ok(summary)
}

pub fn list_summarizable_files(path: String) -> Result<Vec<SummarizableFile>, String> {
    let root = PathBuf::from(&path);
    if !root.exists() {
        return Err(format!("path does not exist: {}", root.display()));
    }
    if !root.is_dir() {
        return Err(format!("path is not a directory: {}", root.display()));
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.into_path();
        let mime_type = infer_supported_mime_type(&path);
        let metadata = fs::metadata(&path)
            .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?;
        if metadata.len() == 0 {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
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

pub async fn summarize_paths(paths: Vec<String>) -> Result<String, String> {
    if paths.is_empty() {
        return Err("no files selected for summary".to_string());
    }

    let selected_paths = paths.into_iter().map(PathBuf::from).collect::<Vec<_>>();
    let root = common_parent_path(&selected_paths).unwrap_or_else(|| PathBuf::from(""));
    let client = OpenAiClient::new()?;
    let model = env::var("OPENAI_DOCUMENT_SUMMARY_MODEL")
        .unwrap_or_else(|_| DEFAULT_DOCUMENT_SUMMARY_MODEL.to_string());
    let (files, skipped_files) = collect_paths_content(&root, &selected_paths)?;
    if files.is_empty() {
        return Err("no selected files could be summarized".to_string());
    }

    let prompt = build_document_summary_prompt(&root, &files, &skipped_files);
    let file_inputs: Vec<ResponsesFileInput<'_>> = files
        .iter()
        .map(|file| ResponsesFileInput::FileData {
            filename: file.filename.as_str(),
            mime_type: file.mime_type,
            data_base64: file.data_base64.as_str(),
        })
        .collect();

    let summary = client
        .gen_model_response_with_files(
            Some(&prompt),
            Some(DOCUMENT_SUMMARY_SYSTEM_PROMPT),
            Some(&model),
            Some(&file_inputs),
        )
        .await?;

    println!("{summary}");

    if !skipped_files.is_empty() {
        eprintln!("skipped {} unsupported or empty files", skipped_files.len());
    }

    Ok(summary)
}

fn build_document_summary_prompt(
    root: &Path,
    files: &[CollectedFile],
    skipped_files: &[String],
) -> String {
    format!(
        "{}\n\n{}\n\n{}",
        build_summary_prompt(root, files, skipped_files),
        DATA_ROOM_TECH_DILIGENCE_SUMMARY_PROMPT.trim(),
        PRODUCT_AND_APPLICATION_DEEP_DIVE_PROMPT.trim()
    )
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
    let mut total_limit_reached = false;

    for path in paths {
        let relative_path = display_relative_path(root, path);
        if total_limit_reached {
            skipped_files.push(format!(
                "{relative_path} (skipped: total request file size limit already reached)"
            ));
            continue;
        }
        if !path.exists() {
            skipped_files.push(format!("{relative_path} (missing)"));
            continue;
        }
        if !path.is_file() {
            skipped_files.push(format!("{relative_path} (not a file)"));
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
            fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
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
