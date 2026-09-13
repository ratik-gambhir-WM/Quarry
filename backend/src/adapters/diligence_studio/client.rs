use std::time::Duration;

use reqwest::{
    header::{HeaderMap, HeaderValue},
    Method, RequestBuilder, Response, Url,
};

pub(super) const DILIGENCE_STUDIO_APP_ID: &str = "Quarry_WestMonroe";
pub(super) const APP_ID_HEADER: &str = "X-App-Id";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct DiligenceStudioClient {
    http: reqwest::Client,
    base_url: Url,
}

impl DiligenceStudioClient {
    pub fn new(http: reqwest::Client, base_url: Url) -> Self {
        Self { http, base_url }
    }

    pub(super) fn endpoint(&self, relative_path: &str) -> Result<Url, String> {
        self.base_url
            .join(relative_path)
            .map_err(|error| format!("failed to build Diligence Studio endpoint: {error}"))
    }

    pub(super) async fn get(&self, endpoint: Url) -> reqwest::Result<Response> {
        self.send_with(Method::GET, endpoint, |request| request)
            .await
    }

    pub(super) async fn post<F>(&self, endpoint: Url, configure: F) -> reqwest::Result<Response>
    where
        F: FnOnce(RequestBuilder) -> RequestBuilder,
    {
        self.send_with(Method::POST, endpoint, configure).await
    }

    pub(super) async fn delete(&self, endpoint: Url) -> reqwest::Result<Response> {
        self.send_with(Method::DELETE, endpoint, |request| request)
            .await
    }

    pub(super) async fn send_with<F>(
        &self,
        method: Method,
        endpoint: Url,
        configure: F,
    ) -> reqwest::Result<Response>
    where
        F: FnOnce(RequestBuilder) -> RequestBuilder,
    {
        self.request(method, endpoint, configure).send().await
    }

    fn request<F>(&self, method: Method, endpoint: Url, configure: F) -> RequestBuilder
    where
        F: FnOnce(RequestBuilder) -> RequestBuilder,
    {
        let request = configure(self.http.request(method, endpoint).timeout(REQUEST_TIMEOUT));
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            APP_ID_HEADER,
            HeaderValue::from_static(DILIGENCE_STUDIO_APP_ID),
        );
        request.headers(default_headers)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/adapters/diligence_studio/client_tests.rs"]
mod tests;
