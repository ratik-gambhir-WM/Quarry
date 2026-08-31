use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;

use crate::{
    config::DataRoomConfig,
    core::{
        clients::office_converter::OfficeConverter,
        data_room_helpers::{
            build_directory_node, read_pdf, resolve_relative_file, validate_pdf_bytes,
            DataRoomTreeNode,
        },
    },
    repository::deal_repository::DealRepository,
    services::error::{ServiceError, ServiceResult},
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

#[derive(Clone)]
pub struct DataRoomService {
    deals: DealRepository,
    config: DataRoomConfig,
    office: OfficeConverter,
}

impl DataRoomService {
    pub fn new(deals: DealRepository, config: DataRoomConfig, office: OfficeConverter) -> Self {
        Self {
            deals,
            config,
            office,
        }
    }

    pub async fn list(&self, deal_id: String) -> ServiceResult<DealDataRoom> {
        let Some(configured_root) = self.deal_data_room_root(&deal_id).await? else {
            return Ok(DealDataRoom {
                deal_id,
                root_name: "Data Room".to_string(),
                root_path: String::new(),
                tree: Vec::new(),
            });
        };
        tokio::task::spawn_blocking(move || {
            let root = canonicalize_data_room_root(configured_root)?;
            let root_name = root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Data Room")
                .to_string();
            let root_node = build_directory_node(&root, &root, Path::new(""), true);
            Ok::<DealDataRoom, String>(DealDataRoom {
                deal_id,
                root_name,
                root_path: root.display().to_string(),
                tree: vec![root_node],
            })
        })
        .await
        .map_err(|error| ServiceError::internal(format!("data-room worker failed: {error}")))?
        .map_err(ServiceError::validation)
    }

    pub async fn preview(
        &self,
        deal_id: &str,
        relative_path: &str,
    ) -> ServiceResult<DocumentPreview> {
        let root = self.canonical_deal_root(deal_id).await?;
        let relative_path = relative_path.to_string();
        let office = self.office.clone();
        tokio::task::spawn_blocking(move || {
            let file_path = resolve_relative_file(&root, &relative_path)?;
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
            let (pdf_bytes, source_kind) =
                match extension.as_str() {
                    "pdf" => (read_pdf(&file_path)?, "native".to_string()),
                    "docx" | "xlsx" | "pptx" => (
                        office.convert_path(&file_path)?,
                        format!("converted-from-{extension}"),
                    ),
                    _ => return Err(
                        "Preview is unsupported. Supported formats are PDF, DOCX, XLSX, and PPTX."
                            .to_string(),
                    ),
                };
            validate_pdf_bytes(&pdf_bytes, "the generated PDF")?;
            Ok(DocumentPreview {
                file_name,
                mime_type: "application/pdf".to_string(),
                pdf_base64: general_purpose::STANDARD.encode(pdf_bytes),
                source_kind,
            })
        })
        .await
        .map_err(|error| {
            ServiceError::internal(format!("document preview worker failed: {error}"))
        })?
        .map_err(ServiceError::validation)
    }

    async fn deal_data_room_root(&self, deal_id: &str) -> ServiceResult<Option<PathBuf>> {
        if let Some(metadata) = self.deals.metadata(deal_id.to_string()).await? {
            return Ok(metadata.local_path.map(PathBuf::from));
        }
        Ok(self.config.root_for_deal(deal_id).cloned())
    }

    async fn canonical_deal_root(&self, deal_id: &str) -> ServiceResult<PathBuf> {
        let configured_root = self.deal_data_room_root(deal_id).await?.ok_or_else(|| {
            ServiceError::validation(format!(
                "no local data-room root is configured for deal \"{deal_id}\""
            ))
        })?;
        canonicalize_data_room_root(configured_root).map_err(ServiceError::validation)
    }
}

fn canonicalize_data_room_root(configured_root: PathBuf) -> Result<PathBuf, String> {
    let root = configured_root.canonicalize().map_err(|error| {
        format!(
            "the configured data-room root is unavailable ({}): {error}",
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
