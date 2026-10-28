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
