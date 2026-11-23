//! Owned, loopback-only Decisions transport shared by unit and integration suites.
use crate::adapters::openai::decisions::DecisionsClient;
use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::{sync::oneshot, task::JoinHandle};

impl DecisionsClient {
    /// Test-only endpoint override; production always uses the fixed HTTPS endpoint.
    pub(crate) fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = reqwest::Url::parse(endpoint).unwrap();
        self
    }
}

#[derive(Clone)]
pub(crate) struct ProviderResponse {
    pub status: StatusCode,
    pub headers: Vec<(&'static str, &'static str)>,
    pub body: String,
}

pub(crate) struct FakeDecisionsProvider {
    endpoint: String,
    pub requests: Arc<Mutex<Vec<(HeaderMap, Bytes)>>>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl FakeDecisionsProvider {
    /// Consume responses in order, repeating the last one for any additional attempts.
    pub(crate) async fn scripted(responses: Vec<ProviderResponse>) -> Self {
        assert!(
            !responses.is_empty(),
            "provider script must contain a response"
        );
        let script = Arc::new(Mutex::new(VecDeque::from(responses)));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&requests);
        let app = Router::new().route(
            "/v1/decisions",
            post(move |headers: HeaderMap, body: Bytes| {
                sink.lock().unwrap().push((headers, body));
                let response = {
                    let mut script = script.lock().unwrap();
                    if script.len() > 1 {
                        script.pop_front().unwrap()
                    } else {
                        script.front().unwrap().clone()
                    }
                };
                async move {
                    let mut reply = (response.status, response.body).into_response();
                    for (name, value) in response.headers {
                        reply
                            .headers_mut()
                            .insert(name, HeaderValue::from_static(value));
                    }
                    reply
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            endpoint: format!("http://{address}/v1/decisions"),
            requests,
            stop: Some(stop),
            task: Some(task),
        }
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(std::time::Duration::from_secs(2), &mut task).await {
                Ok(outcome) => outcome.unwrap(),
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    panic!("fake Decisions provider failed to stop gracefully");
                }
            }
        }
    }
}

impl Drop for FakeDecisionsProvider {
    fn drop(&mut self) {
        // Assertion failures and early returns must also release the listener.
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

pub(crate) async fn fake_provider(
    status: StatusCode,
    headers: Vec<(&'static str, &'static str)>,
    body: String,
) -> FakeDecisionsProvider {
    FakeDecisionsProvider::scripted(vec![ProviderResponse {
        status,
        headers,
        body,
    }])
    .await
}

#[tokio::test]
async fn scripted_responses_are_ordered_and_shutdown_releases_the_listener() {
    let provider = FakeDecisionsProvider::scripted(vec![
        ProviderResponse {
            status: StatusCode::TOO_MANY_REQUESTS,
            headers: vec![("retry-after", "1")],
            body: "retry".into(),
        },
        ProviderResponse {
            status: StatusCode::OK,
            headers: vec![],
            body: "done".into(),
        },
    ])
    .await;
    let endpoint = reqwest::Url::parse(provider.endpoint()).unwrap();
    let address = format!("127.0.0.1:{}", endpoint.port().unwrap());
    let http = reqwest::Client::new();
    for (status, body) in [
        (StatusCode::TOO_MANY_REQUESTS, "retry"),
        (StatusCode::OK, "done"),
        (StatusCode::OK, "done"),
    ] {
        let response = http
            .post(provider.endpoint())
            .body("evidence")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        if status == StatusCode::TOO_MANY_REQUESTS {
            assert_eq!(response.headers()["retry-after"], "1");
        }
        assert_eq!(response.text().await.unwrap(), body);
    }
    assert_eq!(provider.requests.lock().unwrap().len(), 3);
    provider.shutdown().await;
    assert!(tokio::net::TcpStream::connect(&address).await.is_err());
}

#[tokio::test]
async fn dropping_the_fixture_aborts_its_server_and_releases_the_listener() {
    let provider = fake_provider(StatusCode::OK, vec![], "done".into()).await;
    let endpoint = reqwest::Url::parse(provider.endpoint()).unwrap();
    let address = format!("127.0.0.1:{}", endpoint.port().unwrap());
    let task = provider.task.as_ref().unwrap().abort_handle();
    drop(provider);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(tokio::net::TcpStream::connect(&address).await.is_err());
}
