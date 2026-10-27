//! Bounded provider HTTP transport. No canvas or retrieval dependency.
use serde_json::Value;
use std::time::Duration;
#[derive(Clone, Debug, thiserror::Error)]
#[error("The slide provider operation failed.")]
pub struct SlideProviderError {
    pub code: String,
    pub retryable: bool,
}

impl SlideProviderError {
    pub fn new(code: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            retryable,
        }
    }
}

pub async fn post_json(
    client: &reqwest::Client,
    key: &str,
    base_url: Option<&str>,
    endpoint: &str,
    timeout: Duration,
    stage: &str,
    body: &Value,
) -> Result<Value, SlideProviderError> {
    let url = format!(
        "{}/{endpoint}",
        base_url
            .unwrap_or("https://api.openai.com/v1")
            .trim_end_matches('/')
    );
    let response = client
        .post(url)
        .bearer_auth(key)
        .timeout(timeout)
        .json(body)
        .send()
        .await
        .map_err(|error| translate_transport_error(&error, stage))?;
    let status = response.status();
    if !status.is_success() {
        return Err(translate_http_error(status.as_u16(), stage));
    }
    let mut response = response;
    if response
        .content_length()
        .is_some_and(|n| n > 4 * 1024 * 1024)
    {
        return Err(SlideProviderError::new(
            format!("{stage}_unavailable"),
            false,
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| translate_transport_error(&e, stage))?
    {
        if bytes.len().saturating_add(chunk.len()) > 4 * 1024 * 1024 {
            return Err(SlideProviderError::new(
                format!("{stage}_unavailable"),
                false,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| SlideProviderError::new(format!("{stage}_unavailable"), false))
}

pub fn translate_http_error(status: u16, stage: &str) -> SlideProviderError {
    match status {
        429 => SlideProviderError::new(format!("{stage}_rate_limited"), true),
        500.. => SlideProviderError::new(format!("{stage}_unavailable"), true),
        _ => SlideProviderError::new(format!("{stage}_unavailable"), false),
    }
}
fn translate_transport_error(error: &reqwest::Error, stage: &str) -> SlideProviderError {
    if error.is_timeout() {
        SlideProviderError::new(format!("{stage}_timeout"), true)
    } else {
        SlideProviderError::new(format!("{stage}_unavailable"), false)
    }
}
