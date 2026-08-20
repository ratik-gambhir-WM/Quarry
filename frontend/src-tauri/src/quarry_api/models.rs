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
