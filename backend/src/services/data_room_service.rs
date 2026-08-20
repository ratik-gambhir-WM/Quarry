use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;

use crate::core::data_room_helpers::{
    build_directory_node, convert_office_to_pdf, read_pdf, resolve_relative_file, DataRoomTreeNode,
    MAX_PDF_BYTES,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealDataRoom {
    pub deal_id: String,
    pub root_name: String,
    pub root_path: String,
    pub tree: Vec<DataRoomTreeNode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPreview {
    pub file_name: String,
    pub mime_type: String,
    pub pdf_base64: String,
    pub source_kind: String,
}

pub fn list_deal_data_room(deal_id: String) -> Result<DealDataRoom, String> {
    let root = canonical_deal_root(&deal_id)?;
    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Data Room")
        .to_string();
    let root_node = build_directory_node(&root, &root, Path::new(""), true);
    Ok(DealDataRoom {
        deal_id,
        root_name,
        root_path: root.display().to_string(),
        tree: vec![root_node],
    })
}

pub fn build_document_preview(
    deal_id: &str,
    relative_path: &str,
) -> Result<DocumentPreview, String> {
    let root = canonical_deal_root(deal_id)?;
    let file_path = resolve_relative_file(&root, relative_path)?;
    let file_name = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Document")
        .to_string();
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let (pdf_bytes, source_kind) = match extension.as_str() {
        "pdf" => (read_pdf(&file_path)?, "native".to_string()),
        "docx" | "xlsx" | "pptx" => (
            convert_office_to_pdf(&file_path)?,
            format!("converted-from-{extension}"),
        ),
        _ => {
            return Err(
                "Preview is unsupported. Supported formats are PDF, DOCX, XLSX, and PPTX."
                    .to_string(),
            )
        }
    };
    if pdf_bytes.len() as u64 > MAX_PDF_BYTES {
        return Err(format!(
            "The generated PDF is too large to preview ({} MB; limit is {} MB).",
            pdf_bytes.len() / (1024 * 1024),
            MAX_PDF_BYTES / (1024 * 1024)
        ));
    }
    Ok(DocumentPreview {
        file_name,
        mime_type: "application/pdf".to_string(),
        pdf_base64: general_purpose::STANDARD.encode(pdf_bytes),
        source_kind,
    })
}

fn deal_data_room_root(deal_id: &str) -> Option<PathBuf> {
    let env_key = format!(
        "QUARRY_DATA_ROOM_{}",
        deal_id
            .chars()
            .map(|character| if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            })
            .collect::<String>()
    );
    if let Some(configured) = std::env::var_os(env_key) {
        return Some(PathBuf::from(configured));
    }

    let root = match deal_id {
        "project-alpha" => "/Users/rgambhir/BetaNXT/02 - Data Room (CIM, Target Docs)",
        "project-beta" => "/Users/rgambhir/OmegaHealthcare/02. Discovery",
        "logistics-merger" => "/Users/rgambhir/Telluride-Discovery",
        _ => return None,
    };
    Some(PathBuf::from(root))
}

fn canonical_deal_root(deal_id: &str) -> Result<PathBuf, String> {
    let configured_root = deal_data_room_root(deal_id)
        .ok_or_else(|| format!("no local data-room root is configured for deal \"{deal_id}\""))?;
    let root = configured_root.canonicalize().map_err(|err| {
        format!(
            "the configured data-room root is unavailable ({}): {err}",
            configured_root.display()
        )
    })?;
    if !root.is_dir() {
        return Err(format!(
            "the configured data-room root is not a directory: {}",
            root.display()
        ));
    }
    Ok(root)
}
