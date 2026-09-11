//! Tests for stored-document viewing services.

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
