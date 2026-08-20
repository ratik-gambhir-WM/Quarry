use std::{env, time::Duration};

use reqwest::{multipart::Form, Client, Url};
use serde_json::Value;

const DEFAULT_API_BASE_URL: &str = "http://127.0.0.1:3001";

#[derive(Clone)]
pub struct QuarryHttpClient {
    base_url: Url,
    client: Client,
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
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| format!("failed to initialize Quarry HTTP client: {error}"))?;
        Ok(Self { base_url, client })
    }

    pub async fn get(&self, path: &str) -> Result<Value, String> {
        self.send_json(self.client.get(self.url(path)?)).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        self.send_json(self.client.post(self.url(path)?).json(&body))
            .await
    }

    pub async fn post_multipart(&self, path: &str, form: Form) -> Result<Value, String> {
        self.send_json(self.client.post(self.url(path)?).multipart(form))
            .await
    }

    pub async fn get_stream(&self, path: &str) -> Result<reqwest::Response, String> {
        let response = self
            .client
            .get(self.url(path)?)
            .send()
            .await
            .map_err(|error| format!("Quarry API request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
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
            return Err(response_error(response).await);
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
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| format!("Quarry API returned HTTP {status}"));
    message
}
