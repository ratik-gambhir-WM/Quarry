use serde::{Deserialize, Serialize};

use crate::{
    adapters::openai::client::{ChatMessageInput, ChatRole},
    app::http::error::{AppError, AppResult},
};

pub const MAX_CONTEXT_MESSAGES: usize = 64;
pub const MAX_MESSAGE_CHARS: usize = 100_000;
pub const MAX_CONTEXT_CHARS: usize = 400_000;
pub const MAX_SYSTEM_INSTRUCTIONS_CHARS: usize = 50_000;
pub const MAX_MODEL_CHARS: usize = 128;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatContextMessage {
    pub role: ChatContextRole,
    pub content: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatContextRole {
    User,
    Assistant,
}

#[derive(Debug)]
pub struct QueryModelInput {
    pub context: Vec<ChatContextMessage>,
    pub files: Vec<super::upload::ChatUpload>,
    pub model: Option<String>,
    pub prompt: String,
    pub system_instructions: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SendQueryEvent {
    Started { model: String },
    Delta { delta: String },
    Completed { response: String },
    Failed { error: &'static str },
}

impl SendQueryEvent {
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::Started { .. } => "started",
            Self::Delta { .. } => "delta",
            Self::Completed { .. } => "completed",
            Self::Failed { .. } => "failed",
        }
    }
}

pub fn validate_prompt(value: String) -> AppResult<String> {
    validate_preserved_text("prompt", value, MAX_MESSAGE_CHARS)
}

pub fn validate_system_instructions(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|value| {
            validate_preserved_text("systemInstructions", value, MAX_SYSTEM_INSTRUCTIONS_CHARS)
        })
        .transpose()
}

pub fn validate_model(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|value| {
            let model = value.trim();
            if model.is_empty()
                || model.chars().count() > MAX_MODEL_CHARS
                || model.chars().any(char::is_control)
            {
                return Err(AppError::bad_request("model is invalid"));
            }
            Ok(model.to_string())
        })
        .transpose()
}

pub fn parse_context(value: &str) -> AppResult<Vec<ChatContextMessage>> {
    let context: Vec<ChatContextMessage> = serde_json::from_str(value)
        .map_err(|_| AppError::bad_request("context must be a valid JSON message array"))?;
    if context.len() > MAX_CONTEXT_MESSAGES || !context.len().is_multiple_of(2) {
        return Err(AppError::bad_request(
            "context must contain at most 64 messages in complete user/assistant pairs",
        ));
    }
    let mut total_chars = 0usize;
    for (index, message) in context.iter().enumerate() {
        let expected_user = index % 2 == 0;
        if matches!(message.role, ChatContextRole::User) != expected_user {
            return Err(AppError::bad_request(
                "context roles must alternate user then assistant",
            ));
        }
        let count = message.content.chars().count();
        if message.content.trim().is_empty() || count > MAX_MESSAGE_CHARS {
            return Err(AppError::bad_request("context message content is invalid"));
        }
        total_chars = total_chars
            .checked_add(count)
            .ok_or_else(|| AppError::bad_request("context is too large"))?;
    }
    if total_chars > MAX_CONTEXT_CHARS {
        return Err(AppError::bad_request("context is too large"));
    }
    Ok(context)
}

pub fn into_provider_context(context: Vec<ChatContextMessage>) -> Vec<ChatMessageInput> {
    context
        .into_iter()
        .map(|message| ChatMessageInput {
            role: match message.role {
                ChatContextRole::User => ChatRole::User,
                ChatContextRole::Assistant => ChatRole::Assistant,
            },
            content: message.content,
        })
        .collect()
}

fn validate_preserved_text(name: &str, value: String, max_chars: usize) -> AppResult<String> {
    if value.trim().is_empty() || value.chars().count() > max_chars {
        return Err(AppError::bad_request(format!("{name} is invalid")));
    }
    Ok(value)
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/assistant/chat/model_tests.rs"]
mod tests;
