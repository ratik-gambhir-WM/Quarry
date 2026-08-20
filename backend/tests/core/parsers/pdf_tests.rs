use std::{
    collections::BTreeMap,
    env, process,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use super::*;
use crate::core::text_chunking::MAX_TOKEN_CHUNK;
use pdf_extract::{
    content::{Content, Operation},
    dictionary,
    encryption::crypt_filters::{Aes128CryptFilter, CryptFilter},
    EncryptionState, EncryptionVersion, Object, Permissions, StringFormat,
};

fn assemble_pdf_pages(path: &Path, user_id: &str, pages: &[PdfPage]) -> PdfDocumentAssembly {
    let (document_text, _) = document_text_with_page_ranges(pages);

    parse_pdf_file_with_metadata(
        Some(path),
        user_id,
        pages,
        u64::try_from(document_text.len()).unwrap(),
        deterministic_id(&document_text),
    )
}

fn pdf_bytes_with_text(text: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = document.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("Td", vec![100.into(), 700.into()]),
            Operation::new("Tj", vec![Object::string_literal(text)]),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = document.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn passwordless_encrypted_pdf_bytes_with_text(text: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdf_bytes_with_text(text)).unwrap();
    let document_id = vec![0x42; 16];
    document.trailer.set(
        "ID",
        Object::Array(vec![
            Object::String(document_id.clone(), StringFormat::Literal),
            Object::String(document_id, StringFormat::Literal),
        ]),
    );

    let crypt_filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
    let encryption = EncryptionVersion::V4 {
        document: &document,
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), crypt_filter)]),
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: "owner",
        user_password: "",
        permissions: Permissions::all(),
    };
    let encryption_state = EncryptionState::try_from(encryption).unwrap();
    document.encrypt(&encryption_state).unwrap();

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn parses_pdf_bytes_into_document_and_chunk_nodes_without_a_local_path() {
    let bytes = pdf_bytes_with_text("Quarterly quarry report.");
    let expected_size = u64::try_from(bytes.len()).unwrap();
    let expected_hash = sha256_bytes(&bytes);

    let assembly = parse_pdf_by_bytes(bytes, None, "user-1").unwrap();

    assert_eq!(assembly.document.user_id, "user-1");
    assert_eq!(assembly.document.file_name, "Document.pdf");
    assert_eq!(assembly.document.source_type, "pdf");
    assert_eq!(assembly.document.local_path, None);
    assert_eq!(assembly.document.rendered_pdf_path, None);
    assert_eq!(assembly.document.file_size_bytes, expected_size);
    assert_eq!(assembly.document.content_hash, expected_hash);
    assert_eq!(assembly.chunks.len(), 1);
    assert_eq!(
        assembly.chunks[0].document_id,
        assembly.document.document_id
    );
    assert_eq!(assembly.chunks[0].text, "Quarterly quarry report.");
    assert_eq!(assembly.chunks[0].page_numbers, Some(vec![1]));
}

#[test]
fn extracts_passwordless_encrypted_pdf_without_decrypting_it_twice() {
    let bytes = passwordless_encrypted_pdf_bytes_with_text("Encrypted quarry report.");

    let pages = extract_pdf_text_from_bytes(&bytes).unwrap();

    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].text, "Encrypted quarry report.");
}

#[test]
fn assembles_ordered_document_chunks_with_global_offsets() {
    let first_page_text = format!("{}🙂", "First page content. ".repeat(MAX_TOKEN_CHUNK));
    let pdf_pages = vec![
        PdfPage {
            page_number: 1,
            text: first_page_text.clone(),
        },
        PdfPage {
            page_number: 2,
            text: "Second page body.".to_string(),
        },
    ];

    let assembly = assemble_pdf_pages(Path::new("/documents/report.pdf"), "user-1", &pdf_pages);

    assert_eq!(assembly.document.user_id, "user-1");
    assert_eq!(assembly.document.file_name, "report.pdf");
    assert_eq!(assembly.document.source_type, "pdf");
    assert_eq!(
        assembly.document.local_path.as_deref(),
        Some("/documents/report.pdf")
    );
    assert!(assembly.chunks.len() > 1);
    let expected_text = format!("{first_page_text}\n\nSecond page body.");
    assert_eq!(
        assembly.document.file_size_bytes,
        u64::try_from(expected_text.len()).unwrap()
    );
    assert_eq!(
        assembly.document.token_count,
        assembly
            .chunks
            .iter()
            .map(|chunk| u64::from(chunk.token_count))
            .sum::<u64>()
    );
    assert_eq!(
        assembly.document.content_hash,
        deterministic_id(&expected_text)
    );
    assert_eq!(
        assembly
            .chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>(),
        expected_text
    );

    for (chunk_index, chunk) in assembly.chunks.iter().enumerate() {
        assert_eq!(chunk.document_id, assembly.document.document_id);
        assert_eq!(chunk.user_id, "user-1");
        assert_eq!(
            chunk.sequence_number,
            u32::try_from(chunk_index + 1).unwrap()
        );
        assert_eq!(
            &expected_text[chunk.start_offset..chunk.end_offset],
            chunk.text
        );
        assert!(chunk.token_count as usize <= MAX_TOKEN_CHUNK);
        assert!(chunk
            .page_numbers
            .as_ref()
            .is_some_and(|page_numbers| !page_numbers.is_empty()));
        assert_eq!(
            chunk.start_offset,
            assembly
                .chunks
                .get(chunk_index.wrapping_sub(1))
                .map(|previous| previous.end_offset)
                .unwrap_or(0)
        );
    }
    assert_eq!(
        assembly.chunks.last().unwrap().end_offset,
        expected_text.len()
    );
}

#[test]
fn a_chunk_crossing_pages_records_each_page_number() {
    let pdf_pages = vec![
        PdfPage {
            page_number: 4,
            text: "End of page four.".to_string(),
        },
        PdfPage {
            page_number: 5,
            text: "Start of page five.".to_string(),
        },
    ];

    let assembly = assemble_pdf_pages(Path::new("/documents/report.pdf"), "user-1", &pdf_pages);

    assert_eq!(assembly.chunks.len(), 1);
    assert_eq!(
        assembly.chunks[0].text,
        "End of page four.\n\nStart of page five."
    );
    assert_eq!(assembly.chunks[0].page_numbers, Some(vec![4, 5]));
    assert_eq!(assembly.chunks[0].start_offset, 0);
    assert_eq!(assembly.chunks[0].end_offset, assembly.chunks[0].text.len());
}

#[test]
fn assembly_ids_are_stable_and_embeddings_serialize_as_null() {
    let pdf_pages = vec![PdfPage {
        page_number: 1,
        text: "A short PDF page.".to_string(),
    }];

    let first = assemble_pdf_pages(Path::new("/documents/report.pdf"), "user-1", &pdf_pages);
    let second = assemble_pdf_pages(Path::new("/documents/report.pdf"), "user-1", &pdf_pages);
    let chunk = &first.chunks[0];
    let json = serde_json::to_value(chunk).unwrap();

    assert_eq!(first.document.document_id, second.document.document_id);
    assert_eq!(chunk.chunk_id, second.chunks[0].chunk_id);
    assert_eq!(chunk.content_hash.len(), 64);
    assert_eq!(chunk.page_numbers, Some(vec![1]));
    assert_eq!(json["page_numbers"], serde_json::json!([1]));
    assert!(json["embedding"].is_null());
}

#[test]
fn hashes_actual_file_bytes_with_sha256() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "quarry-pdf-hash-test-{}-{unique}.pdf",
        process::id()
    ));
    fs::write(&path, b"mock pdf bytes").unwrap();

    let hash = sha256_file(&path).unwrap();
    fs::remove_file(&path).unwrap();

    assert_eq!(
        hash,
        "58b499442dcfe0024d5b87c73d10cf3f43831f621a443f5c23532c49a8056761"
    );
}

#[test]
fn cleans_page_text_without_destroying_line_order() {
    let text = "Title  \r\n\r\n\r\n  Indented line  \nNext line\n\n";

    assert_eq!(
        clean_pdf_page_text(text),
        "Title\n\n  Indented line\nNext line"
    );
}

#[test]
fn converts_cmyk_pixels_to_rgb() {
    let cmyk = [0, 255, 255, 0, 255, 0, 255, 0];

    assert_eq!(cmyk_to_rgb(&cmyk).unwrap(), vec![255, 0, 0, 0, 255, 0]);
}

#[test]
fn encodes_rgb_pixels_as_png() {
    let rgb = [255, 0, 0, 0, 255, 0];

    let png = encode_png(&rgb, 2, 1, ColorType::Rgb8).unwrap();

    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn builds_numbered_pages_with_clean_text() {
    let raw_pages = vec![
        "First page  \r\n\r\nBody\n".to_string(),
        "\r\n  \r\n".to_string(),
        "Third page\n\n\nDetail  ".to_string(),
    ];

    let pages = pdf_pages_from_texts(&raw_pages);

    assert_eq!(
        pages,
        vec![
            PdfPage {
                page_number: 1,
                text: "First page\n\nBody".to_string(),
            },
            PdfPage {
                page_number: 2,
                text: String::new(),
            },
            PdfPage {
                page_number: 3,
                text: "Third page\n\nDetail".to_string(),
            },
        ]
    );
}
