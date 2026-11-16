//! Tests for stored-document viewing services.

use std::io::Cursor;

use docx_rust::{document::Paragraph, Docx};
use image::{DynamicImage, ImageOutputFormat, RgbImage};

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

#[test]
fn routes_xlsx_and_pptx_previews_through_the_existing_office_converter() {
    for extension in ["xlsx", "pptx"] {
        let pdf =
            render_office_bytes_as_pdf(extension, "source", b"Office source", |actual, bytes| {
                assert_eq!(actual, extension);
                assert_eq!(bytes, b"Office source");
                render_text_as_pdf("preview", "Converted Office preview")
            })
            .unwrap();
        validate_pdf_bytes(&pdf, "the converted PDF preview").unwrap();
    }
}

#[test]
fn renders_an_uploaded_image_as_a_one_page_pdf() {
    let image = RgbImage::from_pixel(120, 60, image::Rgb([20, 80, 160]));
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut encoded, ImageOutputFormat::Png)
        .unwrap();

    let pdf =
        render_image_bytes_as_pdf("site-photo.png", "image/png", &encoded.into_inner()).unwrap();

    validate_pdf_bytes(&pdf, "the image PDF").unwrap();
    let document = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(document.get_pages().len(), 1);
}

#[tokio::test]
async fn returns_the_canonical_raw_text_for_a_stored_docx() {
    let mut docx = Docx::default();
    docx.document
        .push(Paragraph::default().push_text("Canonical raw text from the stored DOCX."));
    let bytes = docx.write(Cursor::new(Vec::new())).unwrap().into_inner();

    let response = render_stored_document_as_text(
        StoredDocumentBlob {
            display_name: "raw.docx".to_string(),
            file_bytes: bytes,
            file_id: "file-raw".to_string(),
            mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .to_string(),
        },
        None,
    )
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

async fn preview_service() -> StoredDocumentService {
    StoredDocumentService::new(
        DocumentStore::new(
            crate::adapters::postgres::client::PostgresClient::test_database()
                .await
                .unwrap(),
        ),
        OfficeConverter::new(None),
    )
}

fn preview_blob(name: &str, mime: &str, bytes: Vec<u8>) -> StoredDocumentBlob {
    StoredDocumentBlob {
        file_id: "preview-fixture".into(),
        display_name: name.into(),
        mime_type: mime.into(),
        file_bytes: bytes,
    }
}

#[tokio::test]
async fn image_and_office_previews_share_cache_and_preserve_shutdown_errors() {
    let service = preview_service().await;
    let image = RgbImage::from_pixel(10, 10, image::Rgb([20, 80, 160]));
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut png, ImageOutputFormat::Png)
        .unwrap();
    let mut docx = Docx::default();
    docx.document
        .push(Paragraph::default().push_text("Cached Office fallback"));
    let inputs = [
        (
            preview_blob("image.png", "image/png", png.into_inner()),
            "Image",
        ),
        (
            preview_blob(
                "office.docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                docx.write(Cursor::new(Vec::new())).unwrap().into_inner(),
            ),
            "Office",
        ),
    ];
    let mut rendered = Vec::new();
    for (document, _) in &inputs {
        let pdf = service.render_pdf(document.clone()).await.unwrap();
        validate_pdf_bytes(&pdf, "the converted PDF").unwrap();
        rendered.push(pdf);
    }
    service.preview_semaphore.close();
    for ((document, _), pdf) in inputs.iter().zip(rendered) {
        assert_eq!(service.render_pdf(document.clone()).await.unwrap(), pdf);
    }
    service.evict_previews().await;
    for (document, label) in inputs {
        let error = service.render_pdf_inner(document, None).await.unwrap_err();
        assert_eq!(
            error,
            format!("{label} preview conversion is shutting down")
        );
    }
}

#[tokio::test]
async fn waiting_preview_rechecks_and_validates_cache_before_starting_conversion() {
    use std::time::Duration;
    let service = preview_service().await;
    // Invalid source bytes ensure any attempt to convert instead of using the cache fails.
    let document = preview_blob("queued.png", "image/png", b"not an image".to_vec());
    let cache_key = office_preview_cache_key(
        &document.mime_type,
        &document.display_name,
        &document.file_bytes,
    );
    let permits = service
        .preview_semaphore
        .acquire_many(MAX_CONCURRENT_OFFICE_PREVIEWS as u32)
        .await
        .unwrap();
    let render = service.render_pdf(document.clone());
    tokio::pin!(render);
    assert!(tokio::time::timeout(Duration::from_millis(20), &mut render)
        .await
        .is_err());
    let cached = render_text_as_pdf("Cached", "Another conversion finished").unwrap();
    service
        .preview_cache
        .lock()
        .await
        .insert(cache_key.clone(), cached.clone());
    drop(permits);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), &mut render)
            .await
            .unwrap()
            .unwrap(),
        cached
    );

    service
        .preview_cache
        .lock()
        .await
        .insert(cache_key, b"invalid PDF".to_vec());
    assert!(service.render_pdf(document).await.is_err());
}
