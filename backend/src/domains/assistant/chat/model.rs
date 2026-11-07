use serde::{Deserialize, Serialize};

use crate::adapters::openai::client::{ChatMessageInput, ChatRole};

#[derive(Debug)]
pub struct ChatUpload {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub kind: ChatUploadKind,
    pub mime_type: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatUploadKind {
    File,
    Image,
}

pub(super) struct RawUpload {
    pub(super) browser_mime: String,
    pub(super) bytes: Vec<u8>,
    pub(super) filename: String,
}

pub const MAX_CHAT_FILES: usize = 20;
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatFileReference {
    pub deal_id: String,
    pub file_id: String,
    pub name: String,
}

#[derive(Debug)]
pub struct QueryModelInput {
    pub file_references: Vec<ChatFileReference>,
    pub file_scope_deal_id: Option<String>,
    pub context: Vec<ChatContextMessage>,
    pub files: Vec<ChatUpload>,
    pub model: Option<String>,
    pub prompt: String,
    pub system_instructions: Option<String>,
}

#[derive(Debug)]
pub struct PersistentQueryInput {
    pub file_references: Vec<ChatFileReference>,
    pub file_scope_deal_id: Option<String>,
    pub assistant_message_id: String,
    pub files: Vec<ChatUpload>,
    pub model: Option<String>,
    pub parent_message_id: Option<String>,
    pub prompt: String,
    pub request_id: String,
    pub system_instructions: Option<String>,
    pub thread_id: String,
    pub user_email: String,
    pub user_message_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SendQueryEvent {
    Started {
        model: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        assistant_message_id: Option<String>,
    },
    Delta {
        delta: String,
    },
    Completed {
        response: String,
    },
    Failed {
        error: &'static str,
    },
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

#[cfg(test)]
#[path = "../../../../tests/unit/domains/assistant/chat/model_tests.rs"]
mod tests;

#[derive(Clone, Debug, Default)]
pub enum ThreadScope {
    #[default]
    All,
    Freeform,
    Deal(String),
}
