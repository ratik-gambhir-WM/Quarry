//! OpenAI Decisions transport (`POST /v1/decisions`).
//!
//! The installed Rig release has no Decisions endpoint, so this is the one documented
//! non-Rig OpenAI transport. It reuses bootstrap's shared HTTP client and server-side
//! key, targets one fixed HTTPS endpoint, bounds request and response bytes, applies no
//! hidden retries and never logs or returns upstream bodies.
use serde::{Deserialize, Serialize};
use std::{fmt, time::Duration};

const ENDPOINT: &str = "https://api.openai.com/v1/decisions";
pub const MAX_REQUEST_BYTES: usize = 128 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct DecisionsClient {
    http: reqwest::Client,
    api_key: ApiKey,
    endpoint: reqwest::Url,
}

#[derive(Clone)]
struct ApiKey(String);

impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl fmt::Debug for DecisionsClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecisionsClient")
            .field("api_key", &self.api_key)
            .field("endpoint", &self.endpoint.as_str())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
pub struct DecisionRequest<'a> {
    pub model: &'a str,
    pub input: Vec<InputMessage<'a>>,
    pub questions: Vec<Question>,
}

#[derive(Debug, Serialize)]
pub struct InputMessage<'a> {
    pub role: InputRole,
    pub content: &'a str,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRole {
    Developer,
    User,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    Choice {
        name: String,
        instructions: String,
        choices: Vec<Choice>,
    },
    Predicate {
        name: String,
        instructions: String,
    },
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub value: String,
    pub description: String,
}

/// Response envelope. Unknown additive fields are ignored for compatibility.
#[derive(Debug, Deserialize)]
pub struct DecisionResponse {
    pub answers: Vec<Answer>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}

/// One typed answer, matched to its question by `name`.
///
/// Decoded through `serde_json::Value` rather than an internally tagged derive: the
/// crate enables `arbitrary_precision`, which breaks numbers buffered by tagged enums.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Predicate {
        name: Option<String>,
        probability: f64,
    },
    Choice {
        name: Option<String>,
        choice: ChoiceValue,
        confidence: f64,
        probabilities: Vec<ChoiceProbability>,
    },
    Score {
        name: Option<String>,
    },
    Refusal {
        name: Option<String>,
    },
}

#[derive(Deserialize)]
struct PredicateWire {
    name: Option<String>,
    probability: f64,
}

#[derive(Deserialize)]
struct ChoiceWire {
    name: Option<String>,
    choice: ChoiceValue,
    confidence: f64,
    probabilities: Vec<ChoiceProbability>,
}

#[derive(Deserialize)]
struct NamedWire {
    name: Option<String>,
}

impl<'de> Deserialize<'de> for Answer {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        let kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| D::Error::custom("answer type is missing"))?
            .to_owned();
        let answer = match kind.as_str() {
            "predicate" => {
                serde_json::from_value::<PredicateWire>(value).map(|wire| Self::Predicate {
                    name: wire.name,
                    probability: wire.probability,
                })
            }
            "choice" => serde_json::from_value::<ChoiceWire>(value).map(|wire| Self::Choice {
                name: wire.name,
                choice: wire.choice,
                confidence: wire.confidence,
                probabilities: wire.probabilities,
            }),
            "score" => serde_json::from_value::<NamedWire>(value)
                .map(|wire| Self::Score { name: wire.name }),
            "refusal" => serde_json::from_value::<NamedWire>(value)
                .map(|wire| Self::Refusal { name: wire.name }),
            _ => return Err(D::Error::custom("unknown answer type")),
        };
        answer.map_err(D::Error::custom)
    }
}

impl Answer {
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Predicate { name, .. }
            | Self::Choice { name, .. }
            | Self::Score { name }
            | Self::Refusal { name } => name.as_deref(),
        }
    }
}

/// Choice values are strings or booleans on the wire.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ChoiceValue {
    Text(String),
    Bool(bool),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChoiceProbability {
    pub value: ChoiceValue,
    pub probability: f64,
}

/// Sanitized transport outcome categories. No variant carries upstream text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionsError {
    RequestTooLarge,
    Timeout,
    Transport,
    RateLimited { retry_after: Option<Duration> },
    Unavailable { retry_after: Option<Duration> },
    Unauthorized,
    Rejected { status: u16 },
    ResponseTooLarge,
    InvalidResponse,
}

impl DecisionsError {
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::Transport | Self::RateLimited { .. } | Self::Unavailable { .. }
        )
    }
}

impl fmt::Display for DecisionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RequestTooLarge => "decisions request exceeds the size limit",
            Self::Timeout => "decisions request timed out",
            Self::Transport => "decisions transport failed",
            Self::RateLimited { .. } => "decisions request was rate limited",
            Self::Unavailable { .. } => "decisions service unavailable",
            Self::Unauthorized => "decisions credentials were rejected",
            Self::Rejected { .. } => "decisions request was rejected",
            Self::ResponseTooLarge => "decisions response exceeds the size limit",
            Self::InvalidResponse => "decisions response is malformed",
        })
    }
}

impl std::error::Error for DecisionsError {}

impl DecisionsClient {
    pub fn new(http: reqwest::Client, api_key: impl Into<String>) -> Self {
        Self {
            http,
            api_key: ApiKey(api_key.into()),
            endpoint: reqwest::Url::parse(ENDPOINT).expect("fixed Decisions endpoint is valid"),
        }
    }

    /// One bounded request. `timeout` covers connect, send, body read and decoding.
    pub async fn decide(
        &self,
        request: &DecisionRequest<'_>,
        timeout: Duration,
    ) -> Result<DecisionResponse, DecisionsError> {
        let body = serde_json::to_vec(request).map_err(|_| DecisionsError::RequestTooLarge)?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(DecisionsError::RequestTooLarge);
        }
        tokio::time::timeout(timeout, self.send(body))
            .await
            .map_err(|_| DecisionsError::Timeout)?
    }

    async fn send(&self, body: Vec<u8>) -> Result<DecisionResponse, DecisionsError> {
        let mut response = self
            .http
            .post(self.endpoint.clone())
            .bearer_auth(&self.api_key.0)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    DecisionsError::Timeout
                } else {
                    DecisionsError::Transport
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| parse_retry_after(value, chrono::Utc::now()));
            return Err(match status.as_u16() {
                401 | 403 => DecisionsError::Unauthorized,
                429 => DecisionsError::RateLimited { retry_after },
                503 => DecisionsError::Unavailable { retry_after },
                408 | 500..=599 => DecisionsError::Unavailable { retry_after: None },
                code => DecisionsError::Rejected { status: code },
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(DecisionsError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| DecisionsError::Transport)?
        {
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(DecisionsError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| DecisionsError::InvalidResponse)
    }
}

/// Parse `Retry-After` as delta-seconds or an HTTP-date, bounded to one minute.
pub fn parse_retry_after(value: &str, now: chrono::DateTime<chrono::Utc>) -> Option<Duration> {
    let value = value.trim();
    let delay = if let Ok(seconds) = value.parse::<u64>() {
        Duration::from_secs(seconds)
    } else {
        let at = chrono::DateTime::parse_from_rfc2822(value).ok()?;
        let delta = at.with_timezone(&chrono::Utc) - now;
        Duration::from_millis(u64::try_from(delta.num_milliseconds()).unwrap_or(0))
    };
    Some(delay.min(MAX_RETRY_AFTER))
}

#[cfg(test)]
#[path = "../../../tests/unit/adapters/openai/decisions_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/integration/adapters/openai/decisions_support.rs"]
pub(crate) mod test_support;
