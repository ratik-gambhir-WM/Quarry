use super::*;
use crate::quarry_api::models::AuthCredentials;

#[tokio::test]
async fn scoped_credentials_are_validated_and_status_is_preserved_without_global_headers() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = [0u8; 4096];
        let count = stream.read(&mut bytes).await.unwrap();
        let request = String::from_utf8_lossy(&bytes[..count]).to_ascii_lowercase();
        assert!(request.contains(&format!("authorization: bearer {}", "a".repeat(43))));
        assert!(request.contains("x-user-id: 7"));
        stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 24\r\nConnection: close\r\n\r\n{\"error\":\"unauthorized\"}").await.unwrap();
    });
    let client = QuarryHttpClient {
        base_url: reqwest::Url::parse(&format!("http://{address}")).unwrap(),
        client: reqwest::Client::new(),
        stream_client: reqwest::Client::new(),
        headers: reqwest::header::HeaderMap::new(),
        status: Arc::new(AtomicU16::new(0)),
    };
    let scoped = client
        .for_request(Some(AuthCredentials {
            token: "a".repeat(43),
            user_id: 7,
        }))
        .unwrap();
    assert!(scoped.get("/api/v1/auth/me").await.is_err());
    assert_eq!(scoped.status(), Some(401));
    assert_eq!(client.status(), None);
    assert!(client.headers.is_empty());
    assert!(client
        .for_request(Some(AuthCredentials {
            token: "injected\r\nheader".into(),
            user_id: 7
        }))
        .is_err());
    assert!(client
        .for_request(Some(AuthCredentials {
            token: "a".repeat(43),
            user_id: 0
        }))
        .is_err());
    server.await.unwrap();
}

#[tokio::test]
async fn classified_import_relay_posts_raw_bytes_and_preserves_request_credentials() {
    use crate::quarry_api::{
        models::{MultipartFile, MultipartRequest},
        service::QuarryApiService,
    };
    use base64::Engine as _;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for (route, mode, count) in [("import", "single", 1), ("batch-import", "batch", 12)] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let expected_path = format!("/api/v1/templates/slide-templates/{route}/classified");
        let path = expected_path.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let boundary = loop {
                let mut part = [0; 4096];
                let size = stream.read(&mut part).await.unwrap();
                assert_ne!(size, 0);
                request.extend_from_slice(&part[..size]);
                if let Some(index) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = String::from_utf8_lossy(&request[..boundary]).to_ascii_lowercase();
            assert!(headers.starts_with(&format!("post {expected_path} http/1.1")));
            assert!(headers.contains("content-type: application/vnd.openxmlformats-officedocument.presentationml.presentation"));
            assert!(headers.contains("content-length: 4"));
            assert!(headers.contains(&format!("authorization: bearer {}", "a".repeat(43))));
            assert!(headers.contains("x-user-id: 7"));
            while request.len() < boundary + 4 {
                let mut part = [0; 4];
                let size = stream.read(&mut part).await.unwrap();
                assert_ne!(size, 0);
                request.extend_from_slice(&part[..size]);
            }
            assert_eq!(&request[boundary..], b"pptx");
            stream.write_all(format!("HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 0\r\nx-template-id: slide-one\r\nx-template-preview-status: ready\r\nx-imported-template-count: {count}\r\nx-powerpoint-warning-count: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        });
        let client = QuarryHttpClient {
            base_url: reqwest::Url::parse(&format!("http://{address}")).unwrap(),
            client: reqwest::Client::new(),
            stream_client: reqwest::Client::new(),
            headers: reqwest::header::HeaderMap::new(),
            status: Arc::new(AtomicU16::new(0)),
        }
        .for_request(Some(AuthCredentials {
            token: "a".repeat(43),
            user_id: 7,
        }))
        .unwrap();
        let service = QuarryApiService { client };
        let result = service
            .post_multipart(MultipartRequest {
                path,
                fields: vec![],
                files: vec![MultipartFile {
                    field_name: "files".into(),
                    filename: "fixture.pptx".into(),
                    mime_type:
                        "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                            .into(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(b"pptx"),
                }],
            })
            .await
            .unwrap();
        assert_eq!(
            result,
            serde_json::json!({"importMode": mode, "importedCount": count, "warningCount": 0})
        );
        server.await.unwrap();
    }
}
