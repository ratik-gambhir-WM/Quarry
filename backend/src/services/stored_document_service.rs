use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

pub use crate::repository::document_repository::DealDocumentSummary;

use crate::{
    core::{
        clients::office_converter::OfficeConverter,
        data_room_helpers::{validate_pdf_bytes, MAX_PDF_BYTES},
        office_extension_for_mime_type,
        parsers::{docx::parse_docx_from_bytes, pdf::parse_pdf_from_bytes},
    },
    repository::document_repository::{DocumentFileRepository, StoredDocumentBlob},
    services::error::{ServiceError, ServiceResult},
    utils::require_non_empty,
};
use lopdf::{
    content::{Content, Operation},
    dictionary, Document as PdfDocument, Object, Stream,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, Semaphore};

const PDF_LINES_PER_PAGE: usize = 46;
const PDF_MAX_LINE_CHARACTERS: usize = 88;
const MAX_CONCURRENT_OFFICE_PREVIEWS: usize = 2;
const OFFICE_PREVIEW_CACHE_ENTRIES: usize = 16;
const OFFICE_PREVIEW_CACHE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Default)]
struct OfficePreviewCache {
    entries: HashMap<String, Vec<u8>>,
    order: VecDeque<String>,
    total_bytes: usize,
}

impl OfficePreviewCache {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        let value = self.entries.get(key)?.clone();
        self.order.retain(|candidate| candidate != key);
        self.order.push_back(key.to_string());
        Some(value)
    }

    fn insert(&mut self, key: String, pdf_bytes: Vec<u8>) {
        if pdf_bytes.len() > OFFICE_PREVIEW_CACHE_BYTES {
            return;
        }
        if let Some(replaced) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(replaced.len());
        }
        self.order.retain(|candidate| candidate != &key);
        self.total_bytes = self.total_bytes.saturating_add(pdf_bytes.len());
        self.entries.insert(key.clone(), pdf_bytes);
        self.order.push_back(key);

        while self.entries.len() > OFFICE_PREVIEW_CACHE_ENTRIES
            || self.total_bytes > OFFICE_PREVIEW_CACHE_BYTES
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(removed.len());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredDocumentText {
    pub file_name: String,
    pub source_kind: String,
    pub text: String,
}

#[derive(Clone)]
pub struct StoredDocumentService {
    files: DocumentFileRepository,
    office: OfficeConverter,
    preview_semaphore: Arc<Semaphore>,
    preview_cache: Arc<Mutex<OfficePreviewCache>>,
}

impl StoredDocumentService {
    pub fn new(files: DocumentFileRepository, office: OfficeConverter) -> Self {
        Self {
            files,
            office,
            preview_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_OFFICE_PREVIEWS)),
            preview_cache: Arc::new(Mutex::new(OfficePreviewCache::default())),
        }
    }

    pub async fn list(&self, deal_id: &str) -> ServiceResult<Vec<DealDocumentSummary>> {
        require_non_empty(deal_id, "dealId").map_err(ServiceError::validation)?;
        self.files.list_for_deal(deal_id).await.map_err(Into::into)
    }

    pub async fn load(&self, deal_id: &str, file_id: &str) -> ServiceResult<StoredDocumentBlob> {
        require_non_empty(deal_id, "dealId").map_err(ServiceError::validation)?;
        require_non_empty(file_id, "fileId").map_err(ServiceError::validation)?;
        self.files
            .current_blob(deal_id, file_id)
            .await?
            .ok_or_else(|| {
                ServiceError::not_found(format!(
                    "file `{file_id}` was not found for deal `{deal_id}`"
                ))
            })
    }

    pub async fn render_pdf(&self, document: StoredDocumentBlob) -> ServiceResult<Vec<u8>> {
        self.render_pdf_inner(document)
            .await
            .map_err(ServiceError::validation)
    }

    async fn render_pdf_inner(&self, document: StoredDocumentBlob) -> Result<Vec<u8>, String> {
        if document.file_bytes.len() as u64 > MAX_PDF_BYTES {
            return Err(format!(
                "The stored document is too large to preview ({} MB; limit is {} MB).",
                document.file_bytes.len() / (1024 * 1024),
                MAX_PDF_BYTES / (1024 * 1024)
            ));
        }

        let pdf_bytes = if document.mime_type == "application/pdf" {
            document.file_bytes
        } else {
            let extension = office_extension_for_mime_type(&document.mime_type).ok_or_else(|| {
            format!(
                "Preview is unsupported for `{}` ({}). Supported formats are PDF, DOC, DOCX, XLS, XLSX, PPT, and PPTX.",
                document.display_name, document.mime_type
            )
        })?;
            let cache_key = office_preview_cache_key(
                &document.mime_type,
                &document.display_name,
                &document.file_bytes,
            );
            if let Some(cached) = self.preview_cache.lock().await.get(&cache_key) {
                validate_pdf_bytes(&cached, "the cached PDF preview")?;
                return Ok(cached);
            }

            let _permit = self
                .preview_semaphore
                .acquire()
                .await
                .map_err(|_| "Office preview conversion is shutting down".to_string())?;
            if let Some(cached) = self.preview_cache.lock().await.get(&cache_key) {
                validate_pdf_bytes(&cached, "the cached PDF preview")?;
                return Ok(cached);
            }

            let display_name = document.display_name;
            let bytes = document.file_bytes;
            let office = self.office.clone();
            let converted = tokio::task::spawn_blocking(move || {
                render_office_bytes_as_pdf(extension, &display_name, &bytes, |extension, bytes| {
                    office.convert_bytes(extension, bytes)
                })
            })
            .await
            .map_err(|error| format!("document preview worker failed: {error}"))??;
            validate_pdf_bytes(&converted, "the converted PDF preview")?;
            self.preview_cache
                .lock()
                .await
                .insert(cache_key, converted.clone());
            converted
        };

        validate_pdf_bytes(&pdf_bytes, "the PDF preview")?;
        Ok(pdf_bytes)
    }
}

fn office_preview_cache_key(mime_type: &str, display_name: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(mime_type.as_bytes());
    hasher.update([0]);
    hasher.update(display_name.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

impl StoredDocumentService {
    pub async fn render_text(
        &self,
        document: StoredDocumentBlob,
    ) -> ServiceResult<StoredDocumentText> {
        render_stored_document_as_text(document)
            .await
            .map_err(ServiceError::validation)
    }
}

async fn render_stored_document_as_text(
    document: StoredDocumentBlob,
) -> Result<StoredDocumentText, String> {
    if document.file_bytes.len() as u64 > MAX_PDF_BYTES {
        return Err(format!(
            "The stored document is too large to extract text from ({} MB; limit is {} MB).",
            document.file_bytes.len() / (1024 * 1024),
            MAX_PDF_BYTES / (1024 * 1024)
        ));
    }

    let file_name = document.display_name;
    let error_file_name = file_name.clone();
    let mime_type = document.mime_type;
    let bytes = document.file_bytes;
    let (source_kind, text) = tokio::task::spawn_blocking(move || match mime_type.as_str() {
        "application/pdf" => parse_pdf_from_bytes(&bytes)
            .map(|text| ("pdf".to_string(), text))
            .map_err(|error| format!("failed to parse the stored PDF: {error}")),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            parse_docx_from_bytes(bytes).map(|text| ("docx".to_string(), text))
        }
        _ => Err(format!(
            "Raw text is unavailable for `{error_file_name}` ({mime_type})."
        )),
    })
    .await
    .map_err(|error| format!("document text worker failed: {error}"))??;

    if text.trim().is_empty() {
        return Err(format!("`{file_name}` did not contain readable text"));
    }

    Ok(StoredDocumentText {
        file_name,
        source_kind,
        text,
    })
}

fn render_office_bytes_as_pdf<F>(
    extension: &str,
    display_name: &str,
    bytes: &[u8],
    converter: F,
) -> Result<Vec<u8>, String>
where
    F: FnOnce(&str, &[u8]) -> Result<Vec<u8>, String>,
{
    match converter(extension, bytes) {
        Ok(pdf) => Ok(pdf),
        Err(conversion_error) if extension == "docx" => {
            let text = parse_docx_from_bytes(bytes.to_vec()).map_err(|fallback_error| {
                format!("{conversion_error} DOCX fallback parsing also failed: {fallback_error}")
            })?;
            render_text_as_pdf(display_name, &text).map_err(|fallback_error| {
                format!("{conversion_error} DOCX fallback rendering also failed: {fallback_error}")
            })
        }
        Err(error) => Err(error),
    }
}

fn render_text_as_pdf(title: &str, text: &str) -> Result<Vec<u8>, String> {
    let lines = wrap_pdf_text(text);
    if lines.is_empty() {
        return Err("the DOCX document did not contain previewable text".to_string());
    }

    let mut pdf = PdfDocument::with_version("1.5");
    let pages_id = pdf.new_object_id();
    let font_id = pdf.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = pdf.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });

    let mut page_ids = Vec::new();
    for page_lines in lines.chunks(PDF_LINES_PER_PAGE) {
        let mut operations = vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 10.into()]),
            Operation::new("TL", vec![15.into()]),
            Operation::new("Td", vec![50.into(), 742.into()]),
        ];
        for (index, line) in page_lines.iter().enumerate() {
            if index > 0 {
                operations.push(Operation::new("T*", vec![]));
            }
            operations.push(Operation::new(
                "Tj",
                vec![Object::string_literal(sanitize_pdf_text(line))],
            ));
        }
        operations.push(Operation::new("ET", vec![]));

        let content = Content { operations }
            .encode()
            .map_err(|error| format!("failed to encode fallback PDF content: {error}"))?;
        let content_id = pdf.add_object(Stream::new(dictionary! {}, content));
        let page_id = pdf.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        page_ids.push(page_id.into());
    }

    let page_count = i64::try_from(page_ids.len())
        .map_err(|_| "fallback PDF page count is too large".to_string())?;
    pdf.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids,
            "Count" => page_count,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        }),
    );
    let catalog_id = pdf.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    let info_id = pdf.add_object(dictionary! {
        "Title" => Object::string_literal(sanitize_pdf_text(title)),
        "Creator" => Object::string_literal("Quarry DOCX preview"),
    });
    pdf.trailer.set("Root", catalog_id);
    pdf.trailer.set("Info", info_id);
    pdf.compress();

    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes)
        .map_err(|error| format!("failed to write fallback PDF: {error}"))?;
    Ok(bytes)
}

fn wrap_pdf_text(text: &str) -> Vec<String> {
    let mut lines = Vec::new();

    for source_line in text.lines() {
        let source_line = source_line.trim();
        if source_line.is_empty() {
            if lines.last().is_some_and(|line: &String| !line.is_empty()) {
                lines.push(String::new());
            }
            continue;
        }

        let mut current = String::new();
        for word in source_line.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + word.chars().count() < PDF_MAX_LINE_CHARACTERS {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = word.to_string();
            }

            while current.chars().count() > PDF_MAX_LINE_CHARACTERS {
                let remainder = current.chars().skip(PDF_MAX_LINE_CHARACTERS).collect();
                let head = current.chars().take(PDF_MAX_LINE_CHARACTERS).collect();
                lines.push(head);
                current = remainder;
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }

    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

fn sanitize_pdf_text(text: &str) -> String {
    text.chars()
        .flat_map(|character| match character {
            '\u{2018}' | '\u{2019}' => "'".chars().collect::<Vec<_>>(),
            '\u{201c}' | '\u{201d}' => "\"".chars().collect(),
            '\u{2013}' | '\u{2014}' => "-".chars().collect(),
            '\u{2022}' => "*".chars().collect(),
            '\u{2026}' => "...".chars().collect(),
            '\u{00a0}' => " ".chars().collect(),
            character if character.is_ascii() => vec![character],
            _ => vec!['?'],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use docx_rust::{document::Paragraph, Docx};

    use super::*;

    #[test]
    fn renders_docx_fallback_pdf_when_office_conversion_is_unavailable() {
        let mut docx = Docx::default();
        docx.document.push(
            Paragraph::default()
                .push_text("A saved DOCX remains previewable when LibreOffice cannot be started."),
        );
        let bytes = docx.write(Cursor::new(Vec::new())).unwrap().into_inner();

        let pdf = render_office_bytes_as_pdf("docx", "fallback.docx", &bytes, |_, _| {
            Err("LibreOffice/soffice was not found".to_string())
        })
        .unwrap();

        validate_pdf_bytes(&pdf, "the fallback PDF").unwrap();
        let extracted = pdf_extract::extract_text_from_mem(&pdf).unwrap();
        assert!(extracted.contains("A saved DOCX remains previewable"));
    }

    #[tokio::test]
    async fn returns_the_canonical_raw_text_for_a_stored_docx() {
        let mut docx = Docx::default();
        docx.document
            .push(Paragraph::default().push_text("Canonical raw text from the stored DOCX."));
        let bytes = docx.write(Cursor::new(Vec::new())).unwrap().into_inner();

        let response = render_stored_document_as_text(StoredDocumentBlob {
            display_name: "raw.docx".to_string(),
            file_bytes: bytes,
            file_id: "file-raw".to_string(),
            mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .to_string(),
        })
        .await
        .unwrap();

        assert_eq!(response.file_name, "raw.docx");
        assert_eq!(response.source_kind, "docx");
        assert_eq!(response.text, "Canonical raw text from the stored DOCX.");
    }

    #[test]
    fn wraps_and_sanitizes_text_for_the_builtin_pdf_font() {
        let text = format!("{}\n\nSmart “quotes” — and bullets •", "word ".repeat(30));
        let lines = wrap_pdf_text(&text);

        assert!(lines.len() >= 3);
        assert!(lines
            .iter()
            .all(|line| line.chars().count() <= PDF_MAX_LINE_CHARACTERS));
        assert_eq!(
            sanitize_pdf_text("Smart “quotes” — bullets •"),
            "Smart \"quotes\" - bullets *"
        );
    }

    #[test]
    fn office_preview_cache_is_bounded_and_evicts_the_oldest_entry() {
        let mut cache = OfficePreviewCache::default();
        for index in 0..=OFFICE_PREVIEW_CACHE_ENTRIES {
            cache.insert(format!("key-{index}"), vec![index as u8]);
        }

        assert_eq!(cache.entries.len(), OFFICE_PREVIEW_CACHE_ENTRIES);
        assert!(!cache.entries.contains_key("key-0"));
        assert!(cache
            .entries
            .contains_key(&format!("key-{OFFICE_PREVIEW_CACHE_ENTRIES}")));
    }
}
