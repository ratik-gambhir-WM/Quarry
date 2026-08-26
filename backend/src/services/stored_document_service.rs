use crate::{
    core::{
        data_room_helpers::{convert_office_bytes_to_pdf, validate_pdf_bytes, MAX_PDF_BYTES},
        office_extension_for_mime_type,
        parsers::{docx::parse_docx_from_bytes, pdf::parse_pdf_from_bytes},
    },
    repository::document_repository::{
        get_current_deal_document_blob, list_current_deal_documents, DealDocumentSummary,
        StoredDocumentBlob,
    },
    state::AppState,
    utils::require_non_empty,
};
use lopdf::{
    content::{Content, Operation},
    dictionary, Document as PdfDocument, Object, Stream,
};
use serde::Serialize;

const PDF_LINES_PER_PAGE: usize = 46;
const PDF_MAX_LINE_CHARACTERS: usize = 88;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredDocumentText {
    pub file_name: String,
    pub source_kind: String,
    pub text: String,
}

pub async fn list_deal_documents(
    state: &AppState,
    deal_id: &str,
) -> Result<Vec<DealDocumentSummary>, String> {
    require_non_empty(deal_id, "dealId")?;
    list_current_deal_documents(state.sqlite(), deal_id).await
}

pub async fn load_deal_document(
    state: &AppState,
    deal_id: &str,
    file_id: &str,
) -> Result<Option<StoredDocumentBlob>, String> {
    require_non_empty(deal_id, "dealId")?;
    require_non_empty(file_id, "fileId")?;
    get_current_deal_document_blob(state.sqlite(), deal_id, file_id).await
}

pub async fn render_stored_document_as_pdf(
    document: StoredDocumentBlob,
) -> Result<Vec<u8>, String> {
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
        let display_name = document.display_name;
        let bytes = document.file_bytes;
        tokio::task::spawn_blocking(move || {
            render_office_bytes_as_pdf(
                extension,
                &display_name,
                &bytes,
                convert_office_bytes_to_pdf,
            )
        })
        .await
        .map_err(|error| format!("document preview worker failed: {error}"))??
    };

    validate_pdf_bytes(&pdf_bytes, "the PDF preview")?;
    Ok(pdf_bytes)
}

pub async fn render_stored_document_as_text(
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
}
