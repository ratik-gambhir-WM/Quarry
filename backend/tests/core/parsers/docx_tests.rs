use super::*;
use crate::core::text_chunking::MAX_TOKEN_CHUNK;

#[test]
fn parses_docx_file_from_bytes_with_the_canonical_parser() {
    let mut empty_docx = Docx::default();
    let bytes = empty_docx
        .write(Cursor::new(Vec::new()))
        .unwrap()
        .into_inner();

    assert!(parse_docx_file_from_bytes(bytes.clone()).is_ok());
    assert_eq!(
        parse_docx_from_bytes(bytes).unwrap_err(),
        "DOCX did not contain readable text"
    );
}

#[test]
fn chunks_reconstruct_text_and_have_contiguous_offsets() {
    let text = format!(
        "{}🙂\n{}",
        "first paragraph ".repeat(MAX_TOKEN_CHUNK),
        "second paragraph ".repeat(MAX_TOKEN_CHUNK)
    );

    let chunks = chunk_nodes_from_text(&text, "document-1", "user-1");

    assert!(chunks.len() > 1);
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>(),
        text
    );

    for (index, chunk) in chunks.iter().enumerate() {
        assert_eq!(chunk.sequence_number, u32::try_from(index + 1).unwrap());
        assert_eq!(&text[chunk.start_offset..chunk.end_offset], chunk.text);
        assert!(chunk.token_count as usize <= MAX_TOKEN_CHUNK);
        assert_eq!(chunk.document_id, "document-1");
        assert_eq!(chunk.user_id, "user-1");
        assert!(chunk.page_numbers.is_none());
        assert_eq!(
            chunk.start_offset,
            chunks
                .get(index.wrapping_sub(1))
                .map(|previous| previous.end_offset)
                .unwrap_or(0)
        );
    }
    assert_eq!(chunks.last().unwrap().end_offset, text.len());
}

#[test]
fn chunk_metadata_is_stable_and_embedding_serializes_as_null() {
    let chunks = chunk_nodes_from_text("A short DOCX body.", "document-1", "user-1");
    let repeated = chunk_nodes_from_text("A short DOCX body.", "document-1", "user-1");
    let chunk = chunks.first().unwrap();
    let json = serde_json::to_value(chunk).unwrap();

    assert_eq!(chunk.start_offset, 0);
    assert_eq!(chunk.end_offset, chunk.text.len());
    assert_eq!(chunk.content_hash.len(), 64);
    assert_eq!(chunk.chunk_id.len(), 64);
    assert_eq!(chunk.chunk_id, repeated[0].chunk_id);
    assert!(json["embedding"].is_null());
    assert!(json["page_numbers"].is_null());
    assert_eq!(json["user_id"], "user-1");
}

#[test]
fn docx_assembly_contains_document_and_chunk_nodes() {
    let text = "A short DOCX body.";
    let assembly = parse_docx_file_with_metadata(
        Some(Path::new("/documents/report.docx")),
        "user-1",
        text,
        4_096,
        "file-content-hash".to_string(),
    );

    assert_eq!(assembly.document.user_id, "user-1");
    assert_eq!(assembly.document.file_name, "report.docx");
    assert_eq!(assembly.document.source_type, "docx");
    assert_eq!(
        assembly.document.local_path.as_deref(),
        Some("/documents/report.docx")
    );
    assert_eq!(assembly.document.file_size_bytes, 4_096);
    assert_eq!(assembly.document.content_hash, "file-content-hash");
    assert!(assembly.document.rendered_pdf_path.is_none());
    assert_eq!(assembly.chunks.len(), 1);
    assert_eq!(
        assembly.chunks[0].document_id,
        assembly.document.document_id
    );
    assert!(assembly.chunks[0].page_numbers.is_none());
    assert_eq!(
        assembly.document.token_count,
        u64::from(assembly.chunks[0].token_count)
    );
}

#[test]
fn docx_bytes_assembly_supports_no_local_path() {
    let mut docx = Docx::default();
    docx.document
        .push(Paragraph::default().push_text("Generated DOCX body."));
    let bytes = docx.write(Cursor::new(Vec::new())).unwrap().into_inner();
    let expected_size = u64::try_from(bytes.len()).unwrap();
    let expected_hash = sha256_bytes(&bytes);

    let assembly = parse_docx_chunks_from_bytes(bytes, None, "user-1").unwrap();

    assert_eq!(assembly.document.file_name, "Document.docx");
    assert!(assembly.document.local_path.is_none());
    assert_eq!(assembly.document.file_size_bytes, expected_size);
    assert_eq!(assembly.document.content_hash, expected_hash);
    assert_eq!(assembly.chunks.len(), 1);
    assert_eq!(assembly.chunks[0].text, "Generated DOCX body.");
    assert!(assembly.chunks[0].page_numbers.is_none());
}
