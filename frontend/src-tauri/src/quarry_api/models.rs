use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipartFile {
    pub data_base64: String,
    pub field_name: String,
    pub filename: String,
    pub mime_type: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipartRequest {
    pub fields: Vec<MultipartField>,
    pub files: Vec<MultipartFile>,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipartField {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentJobEventPayload {
    pub data: String,
    pub event_name: String,
    pub subscription_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPointExportPayload {
    pub data_base64: String,
    pub file_name: String,
    pub mime_type: String,
    pub warning_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatContextMessage {
    pub content: String,
    pub role: ChatContextRole,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatContextRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryStreamRequest {
    pub context: Vec<ChatContextMessage>,
    pub files: Vec<MultipartFile>,
    pub model: Option<String>,
    pub prompt: String,
    pub system_instructions: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum QueryServerEvent {
    Started { model: String },
    Delta { delta: String },
    Completed { response: String },
    Failed { error: String },
}

impl QueryServerEvent {
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::Started { .. } => "started",
            Self::Delta { .. } => "delta",
            Self::Completed { .. } => "completed",
            Self::Failed { .. } => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum QueryEventPayload {
    ServerEvent {
        event: QueryServerEvent,
        subscription_id: String,
    },
    ConnectionError {
        message: String,
        subscription_id: String,
    },
}
