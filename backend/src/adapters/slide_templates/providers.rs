//! OpenAI slide wire encoding/decoding, transport, and sanitized provider failures.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};

use crate::adapters::openai::client::rig_responses_model;
use rig::{
    completion::CompletionRequest,
    message::Message,
    providers::openai::{
        embedding::EncodingFormat,
        extension::{OpenAiOptions, OpenAiResponsesOptions},
        OpenAI, OpenAIConfig,
    },
    ProviderError,
};
use std::{future::Future, time::Duration};
use tracing::instrument::WithSubscriber;

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

pub fn provider(key: &str, base_url: Option<&str>) -> OpenAI {
    let config = OpenAIConfig::new(key);
    let config = match base_url {
        Some(url) => config.with_base_url(url),
        None => config,
    };
    config.client()
}

/// Bound the whole operation, including reading the response. Rig owns the send.
pub async fn run<T>(
    operation: impl Future<Output = Result<T, ProviderError>>,
    timeout: Duration,
    stage: &str,
) -> Result<T, SlideProviderError> {
    tokio::time::timeout(
        timeout,
        operation.with_subscriber(tracing::Dispatch::none()),
    )
    .await
    .map_err(|_| SlideProviderError::new(format!("{stage}_timeout"), true))?
    .map_err(|error| {
        if matches!(error, ProviderError::MismatchedDimensions { .. }) {
            return SlideProviderError::new(format!("{stage}_invalid_vector"), false);
        }
        match error.provider_response_status() {
            Some(status) => translate_http_error(status.as_u16(), stage),
            None => SlideProviderError::new(format!("{stage}_unavailable"), error.is_retryable()),
        }
    })
}

pub fn translate_http_error(status: u16, stage: &str) -> SlideProviderError {
    match status {
        429 => SlideProviderError::new(format!("{stage}_rate_limited"), true),
        500.. => SlideProviderError::new(format!("{stage}_unavailable"), true),
        _ => SlideProviderError::new(format!("{stage}_unavailable"), false),
    }
}

/// Provider inputs carry no domain models or persistence responsibilities.
pub(crate) struct ClassificationRequest<'a> {
    pub model: &'a str,
    pub instructions: &'a str,
    pub digest: &'a str,
    pub preview: Option<&'a [u8]>,
    pub schema: Value,
    pub max_output_tokens: usize,
    pub timeout: Duration,
}

pub(crate) struct SlideProvider {
    client: OpenAI,
}

impl SlideProvider {
    pub(crate) fn new(key: &str, base_url: Option<&str>) -> Self {
        Self {
            client: provider(key, base_url),
        }
    }

    pub(crate) async fn classify(
        &self,
        request: ClassificationRequest<'_>,
    ) -> Result<Value, SlideProviderError> {
        let model = rig_responses_model(&self.client, request.model);
        let response = run(
            model.call(classification_request(&request)?),
            request.timeout,
            "classification",
        )
        .await?;
        Ok(response.raw)
    }

    pub(crate) async fn embed(
        &self,
        model: &str,
        text: &str,
        dimensions: Option<usize>,
        timeout: Duration,
    ) -> Result<Vec<f64>, SlideProviderError> {
        let mut model = self.client.embedding(model, dimensions);
        model.wire = model.wire.with_encoding_format(EncodingFormat::Float);
        let response = run(model.call(vec![text.to_owned()]), timeout, "embedding").await?;
        parse_embedding_response(response.raw)
    }
}

fn classification_request(
    input: &ClassificationRequest<'_>,
) -> Result<CompletionRequest, SlideProviderError> {
    let mut content = vec![json!({"type":"input_text","text":input.digest})];
    if let Some(preview) = input.preview {
        content.push(json!({"type":"input_image","detail":"high","image_url":format!("data:image/png;base64,{}",STANDARD.encode(preview))}));
    }
    let max_tokens = u64::try_from(input.max_output_tokens)
        .map_err(|_| SlideProviderError::new("classification_unavailable", false))?;
    Ok(CompletionRequest::new(Message::system(input.instructions))
        .max_tokens(max_tokens)
        .provider_option(OpenAiOptions::new().store(false).responses(
            OpenAiResponsesOptions::default().input([json!({"role":"user","content":content})])
        ))
        .additional_params(json!({"text":{"format":{"type":"json_schema","name":"slide_retrieval_metadata","strict":true,"schema":input.schema}}})))
}

/// Normalize each text item before resolving refusals/status, retaining error precedence.
pub(crate) fn parse_classification_response<T>(
    response: Value,
    mut normalize: impl FnMut(Value) -> Result<T, SlideProviderError>,
) -> Result<T, SlideProviderError> {
    let unavailable = || SlideProviderError::new("classification_unavailable", false);
    let output = response["output"].as_array().ok_or_else(unavailable)?;
    // The SDK parses every text part before the adapter checks refusals and status.
    // Incomplete responses skip parsing because their JSON may be truncated.
    let status = &response["status"];
    let should_parse = match status {
        Value::Null => true,
        Value::Bool(value) => !value,
        Value::Number(value) => value.as_f64() == Some(0.0),
        Value::String(value) => value.is_empty() || value == "completed",
        _ => false,
    };
    let mut parsed = None;
    let mut refused = false;
    for item in output {
        if item.is_null() {
            return Err(unavailable());
        }
        if item["type"] != "message" {
            continue;
        }
        let content = item["content"].as_array().ok_or_else(unavailable)?;
        for part in content {
            if part.is_null() {
                return Err(unavailable());
            }
            refused |= part["type"] == "refusal";
            if should_parse && part["type"] == "output_text" {
                let text = provider_json_text(part.get("text"));
                let value = serde_json::from_str(&text).map_err(|_| unavailable())?;
                let metadata = normalize(value)?;
                if parsed.is_none() {
                    parsed = Some(metadata);
                }
            }
        }
    }
    if refused {
        return Err(SlideProviderError::new("classification_refused", false));
    }
    if response["status"] != "completed" {
        return Err(SlideProviderError::new("classification_incomplete", false));
    }
    parsed.ok_or_else(|| SlideProviderError::new("classification_missing_output", false))
}

// JSON.parse coerces a malformed non-string `text` field to a JavaScript string.
fn provider_json_text(value: Option<&Value>) -> String {
    match value {
        None => "undefined".to_owned(),
        Some(Value::Null) => "null".to_owned(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                if value.is_null() {
                    String::new()
                } else {
                    provider_json_text(Some(value))
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".to_owned(),
        Some(value) => value.to_string(),
    }
}

pub(crate) fn parse_embedding_response(response: Value) -> Result<Vec<f64>, SlideProviderError> {
    let invalid = || SlideProviderError::new("embedding_invalid_vector", false);
    if response["data"].is_null() {
        return Err(SlideProviderError::new("embedding_unavailable", false));
    }
    let data = response["data"]
        .as_array()
        .filter(|data| data.len() == 1)
        .ok_or_else(invalid)?;
    data[0]["embedding"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|v| v.as_f64().ok_or_else(invalid))
        .collect()
}
