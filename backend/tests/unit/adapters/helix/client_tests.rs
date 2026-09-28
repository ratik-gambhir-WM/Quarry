use std::time::Duration;

use helix_db::{dsl::prelude::*, HelixError};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::oneshot,
    time::timeout,
};

use super::*;

#[test]
fn categorizes_remote_errors_without_inspecting_the_server_message() {
    let conflict = HelixError::RemoteError {
        details: r#"{"error":"request conflicted with a concurrent write"}"#.to_string(),
    };
    let unique_constraint = HelixError::RemoteError {
        details: r#"{"error":"unique constraint violation"}"#.to_string(),
    };

    assert_eq!(helix_error_kind(&conflict), "remote");
    assert_eq!(helix_error_kind(&unique_constraint), "remote");
}

#[tokio::test]
async fn rejects_a_request_whose_explicit_facade_intent_does_not_match_its_query_type() {
    let client = HelixClient::with_config("http://127.0.0.1:1", None).unwrap();
    let error = client
        .execute_read_query::<serde_json::Value>("helix.test", QueryRequest::write(write_batch()))
        .await
        .unwrap_err();

    assert_eq!(error, "Helix query intent mismatch: expected read request");
}

#[tokio::test]
async fn resolves_a_base_url_to_v2_query_and_uses_sdk_bearer_authentication() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 4096];
        let read = socket.read(&mut request).await.unwrap();
        request.truncate(read);
        request_tx
            .send(String::from_utf8(request).unwrap())
            .expect("test request receiver is available");
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}")
            .await
            .unwrap();
    });

    let client = HelixClient::with_config(&format!("http://{address}"), Some("test-key")).unwrap();
    let response: serde_json::Value = client
        .execute_read_query("helix.test.capture", QueryRequest::read(read_batch()))
        .await
        .unwrap();

    assert_eq!(response, serde_json::json!({}));
    let request = request_rx.await.unwrap();
    assert!(request.starts_with("POST /v2/query HTTP/1.1\r\n"));
    assert!(request.contains("\r\nauthorization: Bearer test-key\r\n"));
}

#[tokio::test]
async fn does_not_replay_an_unknown_write_outcome() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (connection_count_tx, connection_count_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut first_socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 4096];
        let _ = first_socket.read(&mut request).await.unwrap();
        drop(first_socket);
        let connection_count = 1 + usize::from(
            timeout(Duration::from_millis(250), listener.accept())
                .await
                .is_ok(),
        );
        connection_count_tx.send(connection_count).unwrap();
    });

    let client = HelixClient::with_config(&format!("http://{address}"), None).unwrap();
    let error = client
        .execute_write_query::<serde_json::Value>(
            "helix.test.unknown_write_outcome",
            QueryRequest::write(write_batch()),
        )
        .await
        .unwrap_err();

    assert_eq!(error, "failed to execute Helix query");
    assert_eq!(connection_count_rx.await.unwrap(), 1);
}
