use std::{
    env,
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    time::Duration,
};

use reqwest::{multipart::Form, Client, Url};
use serde_json::Value;

const DEFAULT_API_BASE_URL: &str = "http://127.0.0.1:3001";

#[derive(Clone)]
pub struct QuarryHttpClient {
    base_url: Url,
    headers: reqwest::header::HeaderMap,
    status: Arc<AtomicU16>,
    client: Client,
    stream_client: Client,
}

impl QuarryHttpClient {
    pub fn from_environment() -> Result<Self, String> {
        let configured =
            env::var("QUARRY_API_BASE_URL").unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_string());
        let base_url = Url::parse(configured.trim())
            .map_err(|error| format!("QUARRY_API_BASE_URL is invalid: {error}"))?;
        let safe_scheme = base_url.scheme() == "https"
            || (base_url.scheme() == "http"
                && matches!(base_url.host_str(), Some("127.0.0.1" | "localhost")));
        if !safe_scheme || base_url.query().is_some() || base_url.fragment().is_some() {
            return Err("QUARRY_API_BASE_URL must be HTTPS or an HTTP loopback URL".to_string());
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| format!("failed to initialize Quarry HTTP client: {error}"))?;
        let stream_client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(45))
            .build()
            .map_err(|error| format!("failed to initialize Quarry stream client: {error}"))?;
        Ok(Self {
            base_url,
            headers: reqwest::header::HeaderMap::new(),
            status: Arc::new(AtomicU16::new(0)),
            client,
            stream_client,
        })
    }

    pub fn for_request(
        &self,
        auth: Option<super::models::AuthCredentials>,
    ) -> Result<Self, String> {
        let mut client = self.clone();
        client.headers = reqwest::header::HeaderMap::new();
        client.status = Arc::new(AtomicU16::new(0));
        if let Some(auth) = auth {
            if auth.user_id <= 0
                || auth.user_id > 9_007_199_254_740_991
                || auth.token.len() != 43
                || !auth
                    .token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err("invalid authentication credentials".into());
            }
            let mut bearer =
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", auth.token))
                    .map_err(|_| "invalid credentials")?;
            bearer.set_sensitive(true);
            client
                .headers
                .insert(reqwest::header::AUTHORIZATION, bearer);
            client.headers.insert(
                "x-user-id",
                reqwest::header::HeaderValue::from_str(&auth.user_id.to_string())
                    .map_err(|_| "invalid user ID")?,
            );
        }
        Ok(client)
    }
    pub fn status(&self) -> Option<u16> {
        match self.status.load(Ordering::Relaxed) {
            0 => None,
            status => Some(status),
        }
    }
    async fn response_error(&self, response: reqwest::Response) -> String {
        self.status
            .store(response.status().as_u16(), Ordering::Relaxed);
        response_error(response).await
    }
    pub async fn get(&self, path: &str) -> Result<Value, String> {
        self.send_json(
            self.client
                .get(self.url(path)?)
                .headers(self.headers.clone()),
        )
        .await
    }

    pub async fn delete(&self, path: &str) -> Result<(), String> {
        let response = self
            .client
            .delete(self.url(path)?)
            .headers(self.headers.clone())
            .send()
            .await
            .map_err(|error| format!("Quarry API request failed: {error}"))?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(());
        }
        if !response.status().is_success() {
            return Err(self.response_error(response).await);
        }
        Err("Quarry API returned an invalid delete response".to_string())
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        self.send_json(
            self.client
                .post(self.url(path)?)
                .headers(self.headers.clone())
                .json(&body),
        )
        .await
    }

    pub async fn post_json_stream(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<reqwest::Response, String> {
        let response = self
            .client
            .post(self.url(path)?)
            .headers(self.headers.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| format!("Quarry API request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(self.response_error(response).await);
        }
        Ok(response)
    }

    pub async fn post_multipart(&self, path: &str, form: Form) -> Result<Value, String> {
        self.send_json(
            self.client
                .post(self.url(path)?)
                .headers(self.headers.clone())
                .multipart(form),
        )
        .await
    }

    pub async fn get_stream(&self, path: &str) -> Result<reqwest::Response, String> {
        let response = self
            .client
            .get(self.url(path)?)
            .headers(self.headers.clone())
            .send()
            .await
            .map_err(|error| format!("Quarry API request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(self.response_error(response).await);
        }
        Ok(response)
    }

    pub async fn post_query_stream(
        &self,
        path: &str,
        form: Form,
    ) -> Result<reqwest::Response, String> {
        let response = self
            .stream_client
            .post(self.url(path)?)
            .headers(self.headers.clone())
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .multipart(form)
            .send()
            .await
            .map_err(|error| format!("Quarry query request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(self.response_error(response).await);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim);
        if content_type != Some("text/event-stream") {
            return Err("Quarry API returned an invalid query stream".to_string());
        }
        Ok(response)
    }

    fn url(&self, path: &str) -> Result<Url, String> {
        self.base_url
            .join(path)
            .map_err(|error| format!("invalid Quarry API path: {error}"))
    }

    async fn send_json(&self, request: reqwest::RequestBuilder) -> Result<Value, String> {
        let response = request
            .send()
            .await
            .map_err(|error| format!("Quarry API request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(self.response_error(response).await);
        }
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        response
            .json()
            .await
            .map_err(|error| format!("Quarry API returned invalid JSON: {error}"))
    }
}

async fn response_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| {
                    error
                        .as_str()
                        .or_else(|| error.get("message").and_then(Value::as_str))
                })
                .map(str::to_string)
        })
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| format!("Quarry API returned HTTP {status}"));
    message
}

#[cfg(test)]
#[path = "../../tests/quarry_api/auth_tests.rs"]
mod tests;
