use super::*;
use std::{num::NonZeroUsize, sync::Arc};

use helix_db::dsl::{BatchQuery, QueryRequestType, QueryValue};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use crate::{
    adapters::helix::client::HelixClient,
    domains::documents::index::repository::DocumentIndexWriter,
};

fn file() -> FileNode {
    FileNode {
        workspace_id: "workspace-1".to_string(),
        file_id: "file-1".to_string(),
        display_name: "report.pdf".to_string(),
    }
}

fn version() -> FileVersionNode {
    FileVersionNode {
        workspace_id: "workspace-1".to_string(),
        file_id: "file-1".to_string(),
        version_id: "version-1".to_string(),
        mime_type: "application/pdf".to_string(),
        content_sha256: "document-hash".to_string(),
        byte_size: 4,
        index_generation: "version-1".to_string(),
        indexed_at: "2026-08-26T00:00:00.000Z".to_string(),
    }
}

fn chunk(index: i64) -> FileChunkNode {
    FileChunkNode {
        chunk_id: format!("chunk-{index}"),
        workspace_id: "workspace-1".to_string(),
        file_id: "file-1".to_string(),
        version_id: "version-1".to_string(),
        index_generation: "version-1".to_string(),
        chunk_index: index,
        text: format!("text-{index}"),
        embedding: vec![index as f32, 1.0],
        chunk_sha256: format!("hash-{index}"),
        token_count: 1,
        page_start: Some(index),
        page_end: Some(index),
        char_start: 0,
        char_end: 6,
        section_path: String::new(),
        created_at: "2026-08-26T00:00:00.000Z".to_string(),
    }
}

#[test]
fn builds_bounded_version_graph_writes() {
    let graph = insert_file_version_graph(file(), version(), vec![chunk(1), chunk(2)]).unwrap();
    let BatchQuery::Write(batch) = graph.setup_query.query() else {
        panic!("expected a write batch");
    };

    assert_eq!(
        batch
            .entries
            .iter()
            .filter(|entry| matches!(entry, BatchEntry::ForEach { param, .. } if param == "chunks"))
            .count(),
        0
    );
    assert_eq!(graph.setup_query.request_type(), QueryRequestType::Write);
    assert!(graph.setup_query.to_json_bytes().unwrap().len() <= HELIX_MAX_QUERY_BODY_BYTES);
    assert_eq!(graph.chunk_queries.len(), 1);
    let chunk_query = &graph.chunk_queries[0];
    assert_eq!(chunk_query.request_type(), QueryRequestType::Write);
    assert!(chunk_query.to_json_bytes().unwrap().len() <= HELIX_MAX_QUERY_BODY_BYTES);
    assert!(matches!(
        chunk_query
            .parameters()
            .and_then(|parameters| parameters.get("chunks")),
        Some(QueryValue::Array(values)) if values.len() == 2
    ));

    let setup_json = graph.setup_query.to_json_string().unwrap();
    let chunk_json = chunk_query.to_json_string().unwrap();
    let publish_json = graph.publish_query.to_json_string().unwrap();
    for value in [QUARRY_FILE_LABEL, FILE_VERSION_LABEL] {
        assert!(setup_json.contains(value));
    }
    for value in [FILE_CHUNK_LABEL, HAS_CHUNK_LABEL] {
        assert!(chunk_json.contains(value));
    }
    assert!(!setup_json.contains("\"Chunk\""));
    assert!(!setup_json.contains(HAS_VERSION_LABEL));
    assert!(!setup_json.contains(CURRENT_VERSION_LABEL));
    assert!(publish_json.contains(HAS_VERSION_LABEL));
    assert!(publish_json.contains(CURRENT_VERSION_LABEL));
    assert!(!chunk_json.contains("document_id"));
    assert!(!chunk_json.contains("ingestion_complete"));
}

#[test]
fn publishes_a_version_only_after_its_chunk_batches() {
    let graph = insert_file_version_graph(file(), version(), vec![chunk(1)]).unwrap();
    let setup_json = graph.setup_query.to_json_string().unwrap();
    let publish_json = graph.publish_query.to_json_string().unwrap();

    assert!(!setup_json.contains(HAS_VERSION_LABEL));
    assert!(!setup_json.contains(CURRENT_VERSION_LABEL));
    assert!(publish_json.contains("removed_has_version"));
    assert!(publish_json.contains("added_has_version"));
    assert!(publish_json.contains("removed_current_version"));
    assert!(publish_json.contains("added_current_version"));
    assert!(!publish_json.contains(HAS_CHUNK_LABEL));
}

#[tokio::test]
async fn failed_chunk_batch_is_not_published_and_a_retry_publishes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (requests_tx, requests_rx) = oneshot::channel();
    tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in [
            helix_success_response(),
            helix_failure_response(),
            helix_success_response(),
            helix_success_response(),
            helix_success_response(),
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            requests.push(read_http_request(&mut socket).await);
            socket.write_all(response).await.unwrap();
        }
        requests_tx.send(requests).unwrap();
    });

    let writer = DocumentIndexWriter::new(Arc::new(
        HelixClient::with_config(&format!("http://{address}"), None).unwrap(),
    ));
    let first_attempt = writer
        .preflight_graph(file(), version(), vec![chunk(1)])
        .unwrap();
    let error = writer
        .insert_prepared_graph(first_attempt)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("chunk batch 1 of 1"));

    let retry = writer
        .preflight_graph(file(), version(), vec![chunk(1)])
        .unwrap();
    writer.insert_prepared_graph(retry).await.unwrap();

    let requests = requests_rx.await.unwrap();
    assert_eq!(requests.len(), 5);
    assert!(requests[0].contains(FILE_VERSION_LABEL));
    assert!(requests[0].contains("removed_version_chunks"));
    assert!(requests[1].contains(FILE_CHUNK_LABEL));
    assert!(!requests[1].contains(HAS_VERSION_LABEL));
    assert!(requests[2].contains(FILE_VERSION_LABEL));
    assert!(requests[2].contains("removed_version_chunks"));
    assert!(requests[3].contains(FILE_CHUNK_LABEL));
    assert!(requests[4].contains(HAS_VERSION_LABEL));
    assert!(requests[4].contains(CURRENT_VERSION_LABEL));
}

async fn read_http_request(socket: &mut TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        let bytes_read = socket.read(&mut buffer).await.unwrap();
        assert!(
            bytes_read > 0,
            "Helix test server received an incomplete request"
        );
        request.extend_from_slice(&buffer[..bytes_read]);

        if let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let header_end = header_end + 4;
            let headers = std::str::from_utf8(&request[..header_end]).unwrap();
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse::<usize>()
                .unwrap();
            if request.len() >= header_end + content_length {
                return String::from_utf8(request).unwrap();
            }
        }
    }
}

fn helix_success_response() -> &'static [u8] {
    b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 4\r\nconnection: close\r\n\r\nnull"
}

fn helix_failure_response() -> &'static [u8] {
    b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
}

#[test]
fn chunk_batch_payload_accounting_matches_the_final_query_serialization() {
    let chunk_params = vec![file_chunk_params(&chunk(1)), file_chunk_params(&chunk(2))];
    let empty_query =
        insert_file_version_chunk_batch_query(&file(), &version(), Vec::new()).unwrap();
    let mut expected_payload_bytes = serialized_query_payload_bytes(&empty_query).unwrap();
    for (index, chunk_param) in chunk_params.iter().enumerate() {
        expected_payload_bytes = next_chunk_batch_payload_bytes(
            expected_payload_bytes,
            index > 0,
            serialized_chunk_param_contribution_bytes(
                &file(),
                &version(),
                chunk_param,
                serialized_query_payload_bytes(&empty_query).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
    let query = insert_file_version_chunk_batch_query(&file(), &version(), chunk_params).unwrap();

    assert_eq!(
        expected_payload_bytes,
        serialized_query_payload_bytes(&query).unwrap()
    );
}

#[test]
fn cleanup_is_scoped_to_the_indexed_version() {
    let graph = insert_file_version_graph(file(), version(), vec![chunk(1)]).unwrap();
    let json = graph.setup_query.to_json_string().unwrap();

    assert!(json.contains("removed_version_chunks"));
    assert!(json.contains("version_id"));
    assert!(!json.contains("stale_quarry_file"));
    assert!(!json.contains("drop\":{\"input\":{\"n\":{\"ids\":{\"var\":\"canonical_version"));
}

#[test]
fn rejects_mismatched_graph_identity() {
    let mut invalid = chunk(1);
    invalid.version_id = "another-version".to_string();

    let error = insert_file_version_graph(file(), version(), vec![invalid]).unwrap_err();
    assert!(error.contains("does not match"));
}

#[test]
fn rejects_a_chunk_that_cannot_fit_in_its_own_request() {
    let mut oversized = chunk(1);
    oversized.text = "x".repeat(HELIX_MAX_QUERY_BODY_BYTES);

    let error = insert_file_version_graph(file(), version(), vec![oversized]).unwrap_err();
    assert!(error.contains("Helix file chunk batch query"));
    assert!(error.contains(&HELIX_MAX_QUERY_BODY_BYTES.to_string()));
}

#[test]
fn splits_large_graph_chunks_into_individually_safe_requests() {
    let chunks = (1..=3)
        .map(|index| {
            let mut chunk = chunk(index);
            chunk.text = "x".repeat(900_000);
            chunk
        })
        .collect();

    let graph = insert_file_version_graph(file(), version(), chunks).unwrap();

    assert!(graph.chunk_queries.len() > 1);
    assert!(graph
        .chunk_queries
        .iter()
        .all(|query| query.to_json_bytes().unwrap().len() <= HELIX_MAX_QUERY_BODY_BYTES));
}

#[test]
fn indexes_versioned_file_graph_properties() {
    let query = create_document_indexes(NonZeroUsize::new(1536).unwrap());
    let json = query.to_json_string().unwrap();

    assert!(json.contains("file_id"));
    assert!(json.contains("version_id"));
    assert!(json.contains("chunk_id"));
    assert!(json.contains("content_sha256"));
    assert!(json.contains("embedding"));
    assert!(json.contains("workspace_id"));
    assert!(json.contains(FILE_CHUNK_LABEL));
    assert!(json.contains("cosine"));
    assert!(!json.contains("document_id"));
    assert_eq!(DOCUMENT_INDEX_NAMES.len(), 13);
    assert_eq!(query.request_type(), QueryRequestType::Write);
}

#[test]
fn reads_index_operation_status_with_a_read_query() {
    let query = get_document_index_operation("operation-1".to_string());

    assert_eq!(query.request_type(), QueryRequestType::Read);
    assert!(query
        .to_json_string()
        .unwrap()
        .contains("get_index_operation"));
}

#[test]
fn uses_the_conservative_helix_v3_preflight_body_limit() {
    assert_eq!(HELIX_MAX_QUERY_BODY_BYTES, 2_097_152);
}
