pub mod clients;
pub mod data_room_helpers;
pub mod helix_queries;
pub mod models;
pub mod nodes;
pub mod parsers;
pub mod prompts;
pub mod sqlbuilder;
pub mod text_chunking;

use std::fs;
use std::path::{Path, PathBuf};

pub struct CollectedFile {
    pub filename: String,
    pub relative_path: String,
    pub mime_type: &'static str,
    pub size_bytes: usize,
    pub data_base64: String,
}

pub fn display_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub fn infer_supported_mime_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => Some("application/pdf"),
        Some("txt") => Some("text/plain"),
        Some("md") => Some("text/markdown"),
        Some("json") => Some("application/json"),
        Some("html") => Some("text/html"),
        Some("csv") => Some("text/csv"),
        Some("doc") => Some("application/msword"),
        Some("docx") => {
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        Some("pptx") => {
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        }
        Some("xls") => Some("application/vnd.ms-excel"),
        Some("xlsx") => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        _ => None,
    }
}

pub fn office_extension_for_mime_type(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "application/msword" => Some("doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/vnd.ms-excel" => Some("xls"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
        "application/vnd.ms-powerpoint" => Some("ppt"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
        _ => None,
    }
}

pub fn write_summary(summary: &str, output_path: impl AsRef<Path>) -> Result<(), String> {
    let mut output_path = PathBuf::from(output_path.as_ref());

    if output_path.extension().is_none() {
        output_path.set_extension("md");
    }

    fs::write(&output_path, summary)
        .map_err(|err| format!("failed to write {}: {err}", output_path.display()))?;
    println!("wrote summary to {}", output_path.display());
    Ok(())
}
