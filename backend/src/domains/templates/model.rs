use serde::{Deserialize, Serialize};
pub const PPTX_MULTI_SLIDE_IMPORT_MESSAGE: &str =
    "This PowerPoint contains multiple slides. Use Import Deck Template instead.";
pub const MAX_PPTX_TEMPLATE_IMPORT_BYTES: usize = 25 * 1024 * 1024;
pub const MAX_TEMPLATE_DOCUMENT_BYTES: usize = 50 * 1024 * 1024;
pub const POWERPOINT_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PptxTemplateImportMode {
    Single,
    Batch,
}

#[derive(Debug)]
pub struct PptxTemplateUpload {
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxTemplateImportResult {
    pub import_mode: PptxTemplateImportMode,
    pub imported_count: usize,
    pub warning_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerPointExport {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub warning_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewPage {
    pub pagination: TemplatePreviewPagination,
    pub previews: Vec<TemplatePreview>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewPagination {
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreview {
    pub template_id: String,
    pub content_type: String,
    pub data_url: String,
    pub preview_url: String,
    pub width: u32,
    pub height: u32,
}

pub const MAX_TEMPLATE_ID_BYTES: usize = 256;
pub const MAX_TEMPLATE_PREVIEW_PAGES: usize = 100;
pub const MAX_POWERPOINT_EXPORT_BYTES: usize = 64 * 1024 * 1024;
