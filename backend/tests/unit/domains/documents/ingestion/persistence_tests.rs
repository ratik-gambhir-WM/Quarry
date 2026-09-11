use super::*;

const OWNER: &str = "analyst@example.com";

fn document(file_id: &str, owner: &str, filename: &str, bytes: &[u8]) -> Document {
    let content_hash = sha256_hex(bytes);
    Document {
        file_id: file_id.to_string(),
        document_id: document_id_from_content(owner, &content_hash),
        user_id: owner.to_string(),
        file_name: filename.to_string(),
        source_type: Path::new(filename)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase(),
        local_path: Some(format!("/documents/{filename}")),
        file_size_bytes: bytes.len() as u64,
        token_count: 42,
        content_hash,
        rendered_pdf_path: Some(format!("/rendered/{filename}.pdf")),
    }
}

fn insert_blob_result(document: &Document) -> PersistedFileIdentity {
    PersistedFileIdentity {
        file_id: document.file_id.clone(),
        workspace_id: document.user_id.clone(),
        display_name: document.file_name.clone(),
        version_id: file_version_id(&document.file_id, &document.content_hash),
    }
}

fn document_chunk(document: &Document) -> DocumentChunk {
    DocumentChunk {
        chunk_id: "transient-chunk".to_string(),
        document_id: document.document_id.clone(),
        user_id: document.user_id.clone(),
        text: "graph".to_string(),
        embedding: Some(vec![0.25, 0.5]),
        sequence_number: 7,
        page_numbers: Some(vec![3, 2, 3]),
        start_offset: 1,
        end_offset: 6,
        token_count: 1,
        content_hash: sha256_hex(b"graph"),
        section_title: Some("Overview".to_string()),
    }
}

#[test]
fn build_file_node_maps_the_insert_blob_result() {
    let document = document("logical-file", OWNER, "report.pdf", b"file node");
    let insert_blob_result = insert_blob_result(&document);

    let file_node = build_file_node(&insert_blob_result);

    assert_eq!(file_node.workspace_id, insert_blob_result.workspace_id);
    assert_eq!(file_node.file_id, insert_blob_result.file_id);
    assert_eq!(file_node.display_name, insert_blob_result.display_name);
}

#[test]
fn build_file_version_node_maps_document_and_insert_blob_metadata() {
    let bytes = b"file version node";
    let document = document("logical-file", OWNER, "report.pdf", bytes);
    let insert_blob_result = insert_blob_result(&document);

    let version_node = build_file_version_node(&insert_blob_result, &document).unwrap();

    assert_eq!(version_node.workspace_id, insert_blob_result.workspace_id);
    assert_eq!(version_node.file_id, insert_blob_result.file_id);
    assert_eq!(version_node.version_id, insert_blob_result.version_id);
    assert_eq!(version_node.mime_type, "application/pdf");
    assert_eq!(version_node.content_sha256, document.content_hash);
    assert_eq!(version_node.byte_size, bytes.len() as i64);
    assert_eq!(version_node.index_generation, insert_blob_result.version_id);
    assert!(chrono::DateTime::parse_from_rfc3339(&version_node.indexed_at).is_ok());
}

#[test]
fn build_file_chunk_node_maps_chunk_metadata_and_derives_its_id() {
    let document = document("logical-file", OWNER, "report.pdf", b"file chunk node");
    let insert_blob_result = insert_blob_result(&document);
    let version_node = build_file_version_node(&insert_blob_result, &document).unwrap();
    let document_chunk = document_chunk(&document);

    let chunk_node =
        build_file_chunk_node(&insert_blob_result, &version_node, &document_chunk).unwrap();

    let expected_chunk_id = deterministic_file_chunk_id(
        &insert_blob_result.workspace_id,
        &insert_blob_result.file_id,
        &insert_blob_result.version_id,
        &version_node.index_generation,
        i64::from(document_chunk.sequence_number),
        &document_chunk.content_hash,
    );
    assert_eq!(chunk_node.chunk_id, expected_chunk_id);
    assert_eq!(chunk_node.workspace_id, insert_blob_result.workspace_id);
    assert_eq!(chunk_node.file_id, insert_blob_result.file_id);
    assert_eq!(chunk_node.version_id, insert_blob_result.version_id);
    assert_eq!(chunk_node.index_generation, version_node.index_generation);
    assert_eq!(chunk_node.chunk_index, 7);
    assert_eq!(chunk_node.text, document_chunk.text);
    assert_eq!(chunk_node.embedding, vec![0.25, 0.5]);
    assert_eq!(chunk_node.chunk_sha256, document_chunk.content_hash);
    assert_eq!(chunk_node.token_count, 1);
    assert_eq!(chunk_node.page_start, Some(2));
    assert_eq!(chunk_node.page_end, Some(3));
    assert_eq!(chunk_node.char_start, 1);
    assert_eq!(chunk_node.char_end, 6);
    assert_eq!(chunk_node.section_path, "Overview");
    assert_eq!(chunk_node.created_at, version_node.indexed_at);
}

#[test]
fn maps_service_chunks_to_deterministic_version_scoped_graph_nodes() {
    let bytes = b"graph document";
    let document = document("logical-file", OWNER, "report.pdf", bytes);
    let insert_blob_result = PersistedFileIdentity {
        file_id: document.file_id.clone(),
        workspace_id: document.user_id.clone(),
        display_name: document.file_name.clone(),
        version_id: file_version_id(&document.file_id, &document.content_hash),
    };
    let service_chunk = DocumentChunk {
        chunk_id: "transient-chunk".to_string(),
        document_id: document.document_id.clone(),
        user_id: document.user_id.clone(),
        text: "graph".to_string(),
        embedding: Some(vec![0.25, 0.5]),
        sequence_number: 7,
        page_numbers: Some(vec![3, 2, 3]),
        start_offset: 1,
        end_offset: 6,
        token_count: 1,
        content_hash: sha256_hex(b"graph"),
        section_title: Some("Overview".to_string()),
    };

    let (file_node, version_node, first_chunk_nodes) = build_helix_graph_nodes(
        &insert_blob_result,
        &document,
        std::slice::from_ref(&service_chunk),
    )
    .unwrap();
    let (_, _, retry_chunk_nodes) =
        build_helix_graph_nodes(&insert_blob_result, &document, &[service_chunk]).unwrap();

    assert_eq!(file_node.file_id, insert_blob_result.file_id);
    assert_eq!(version_node.version_id, insert_blob_result.version_id);
    assert_eq!(version_node.index_generation, insert_blob_result.version_id);
    assert_eq!(first_chunk_nodes[0].chunk_id, retry_chunk_nodes[0].chunk_id);
    assert_eq!(first_chunk_nodes[0].page_start, Some(2));
    assert_eq!(first_chunk_nodes[0].page_end, Some(3));
    assert_eq!(first_chunk_nodes[0].section_path, "Overview");
    assert_eq!(first_chunk_nodes[0].created_at, version_node.indexed_at);

    let another_id = deterministic_file_chunk_id(
        OWNER,
        "another-file",
        &insert_blob_result.version_id,
        &insert_blob_result.version_id,
        7,
        &first_chunk_nodes[0].chunk_sha256,
    );
    assert_ne!(first_chunk_nodes[0].chunk_id, another_id);
}

#[test]
fn graph_mapping_rejects_invalid_chunk_metadata_before_building_a_query() {
    let bytes = b"invalid graph document";
    let document = document("logical-file", OWNER, "report.docx", bytes);
    let insert_blob_result = PersistedFileIdentity {
        file_id: document.file_id.clone(),
        workspace_id: document.user_id.clone(),
        display_name: document.file_name.clone(),
        version_id: file_version_id(&document.file_id, &document.content_hash),
    };
    let chunk = |sequence_number, embedding: Option<Vec<f32>>| DocumentChunk {
        chunk_id: format!("chunk-{sequence_number}"),
        document_id: document.document_id.clone(),
        user_id: document.user_id.clone(),
        text: "text".to_string(),
        embedding,
        sequence_number,
        page_numbers: None,
        start_offset: 0,
        end_offset: 4,
        token_count: 1,
        content_hash: sha256_hex(b"text"),
        section_title: None,
    };

    assert!(build_helix_graph_nodes(&insert_blob_result, &document, &[chunk(1, None)]).is_err());
    assert!(build_helix_graph_nodes(
        &insert_blob_result,
        &document,
        &[chunk(1, Some(vec![1.0])), chunk(1, Some(vec![1.0]))],
    )
    .is_err());
    assert!(build_helix_graph_nodes(
        &insert_blob_result,
        &document,
        &[chunk(1, Some(vec![1.0])), chunk(2, Some(vec![1.0, 2.0]))],
    )
    .is_err());

    let (_, _, docx_chunk_nodes) =
        build_helix_graph_nodes(&insert_blob_result, &document, &[chunk(1, Some(vec![1.0]))])
            .unwrap();
    assert_eq!(docx_chunk_nodes[0].page_start, None);
    assert_eq!(docx_chunk_nodes[0].page_end, None);
}

#[test]
fn rejects_a_chunk_for_another_document_before_persisting() {
    let document = Document {
        file_id: "file-1".to_string(),
        document_id: "doc-1".to_string(),
        user_id: "user-1".to_string(),
        file_name: "test.pdf".to_string(),
        source_type: "pdf".to_string(),
        local_path: None,
        file_size_bytes: 1,
        token_count: 1,
        content_hash: "hash".to_string(),
        rendered_pdf_path: None,
    };

    let chunk = DocumentChunk {
        chunk_id: "chunk-1".to_string(),
        document_id: "doc-2".to_string(),
        user_id: "user-1".to_string(),
        text: "text".to_string(),
        embedding: Some(vec![1.0]),
        sequence_number: 1,
        page_numbers: None,
        start_offset: 0,
        end_offset: 4,
        token_count: 1,
        content_hash: "hash".to_string(),
        section_title: None,
    };

    assert!(ensure_chunk_belongs_to_document(&document, &chunk).is_err());
}
