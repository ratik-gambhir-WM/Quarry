use std::env;

use reqwest::{
    header::{HeaderMap, HeaderValue},
    multipart::{Form, Part},
    Client, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const API_KEY_HEADER: &str = "x-api-key";

#[derive(Debug, Clone)]
pub struct WmUploadedFile {
    pub filename: String,
    pub relative_path: String,
    pub mime_type: &'static str,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileExtractResponse {
    pub files: Vec<ExtractedFile>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFile {
    pub file_id: Option<String>,
    pub filename: String,
    pub mime_type: Option<String>,
    pub size_bytes: Option<usize>,
    pub text: String,
    pub page_count: Option<u32>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateIndexPayload {
    pub name: Option<String>,
    pub files: Vec<IndexDocumentInput>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndexDocumentInput {
    pub file_id: Option<String>,
    pub filename: String,
    pub text: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateIndexResponse {
    pub index_id: String,
    pub file_ids: Vec<String>,
    pub run_id: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatusResponse {
    #[serde(alias = "Status")]
    pub status: String,
    #[serde(alias = "resourceId")]
    pub resource_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphRagQueryPayload {
    pub resource_id: String,
    pub question: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphRagQueryResponse {
    pub answer: String,
    #[serde(default)]
    pub sources: Vec<Value>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct FileUploadServiceClient {
    http_client: Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Clone)]
pub struct IndexServiceClient {
    http_client: Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Clone)]
pub struct GraphRagClient {
    http_client: Client,
    base_url: String,
    api_key: String,
    application_name: String,
}

impl FileUploadServiceClient {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self::new(
            env_var("WM_FILE_UPLOAD_SERVICE_URL")?,
            env_var("WM_FILE_UPLOAD_API_KEY")?,
        ))
    }

    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            api_key,
        }
    }

    pub async fn extract_files(
        &self,
        files: Vec<WmUploadedFile>,
    ) -> Result<FileExtractResponse, String> {
        if files.is_empty() {
            return Err("at least one file is required for extraction".to_string());
        }

        let mut form = Form::new();
        for file in files {
            let part = Part::bytes(file.bytes)
                .file_name(file.relative_path)
                .mime_str(file.mime_type)
                .map_err(|err| {
                    format!("failed to build multipart file {}: {err}", file.filename)
                })?;
            form = form.part("files", part);
        }

        let response = self
            .http_client
            .post(service_url(&self.base_url, "files/extract")?)
            .headers(auth_headers(&self.api_key)?)
            .multipart(form)
            .send()
            .await
            .map_err(|err| format!("failed to call WM File Upload Service: {err}"))?;

        parse_json_response(response, "WM File Upload Service").await
    }
}

impl IndexServiceClient {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self::new(
            env_var("WM_INDEX_SERVICE_URL")?,
            env_var("WM_INDEX_SERVICE_API_KEY")?,
        ))
    }

    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            api_key,
        }
    }

    pub async fn create_index(
        &self,
        payload: CreateIndexPayload,
    ) -> Result<CreateIndexResponse, String> {
        validate_create_index_payload(&payload)?;
        let request_body = build_create_index_request_body(&payload);

        let response = self
            .http_client
            .post(service_url(&self.base_url, "indexes")?)
            .headers(auth_headers(&self.api_key)?)
            .json(&request_body)
            .send()
            .await
            .map_err(|err| format!("failed to call WM Index Service: {err}"))?;

        parse_json_response(response, "WM Index Service").await
    }

    pub async fn status(&self, index_id: &str) -> Result<IndexStatusResponse, String> {
        validate_path_segment(index_id, "indexId")?;
        let path = format!("indexes/{index_id}/status");

        let response = self
            .http_client
            .get(service_url(&self.base_url, &path)?)
            .headers(auth_headers(&self.api_key)?)
            .send()
            .await
            .map_err(|err| format!("failed to call WM Index Service: {err}"))?;

        parse_json_response(response, "WM Index Service").await
    }
}

impl GraphRagClient {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self::new(
            env_var("WM_GRAPHRAG_URL")?,
            env_var("WM_GRAPHRAG_API_KEY")?,
            env_var("WM_GRAPHRAG_APPLICATION_NAME")?,
        ))
    }

    pub fn new(base_url: String, api_key: String, application_name: String) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            api_key,
            application_name,
        }
    }

    pub async fn query(
        &self,
        payload: GraphRagQueryPayload,
    ) -> Result<GraphRagQueryResponse, String> {
        validate_graph_rag_query_payload(&payload)?;
        validate_path_segment(&payload.resource_id, "resourceId")?;
        let path = format!("graph/{}/query", payload.resource_id);
        let request_body = build_graph_rag_query_request_body(&self.application_name, &payload);

        let response = self
            .http_client
            .post(service_url(&self.base_url, &path)?)
            .headers(auth_headers(&self.api_key)?)
            .json(&request_body)
            .send()
            .await
            .map_err(|err| format!("failed to call WM GraphRAG: {err}"))?;

        parse_json_response(response, "WM GraphRAG").await
    }
}

pub fn validate_create_index_payload(payload: &CreateIndexPayload) -> Result<(), String> {
    if payload.files.is_empty() {
        return Err("at least one extracted file is required".to_string());
    }

    for file in &payload.files {
        if file.filename.trim().is_empty() {
            return Err("filename cannot be empty".to_string());
        }
        if file.text.trim().is_empty() {
            return Err(format!(
                "extracted text cannot be empty for {}",
                file.filename
            ));
        }
    }

    Ok(())
}

pub fn validate_graph_rag_query_payload(payload: &GraphRagQueryPayload) -> Result<(), String> {
    if payload.resource_id.trim().is_empty() {
        return Err("resourceId cannot be empty".to_string());
    }
    if payload.question.trim().is_empty() {
        return Err("question cannot be empty".to_string());
    }

    Ok(())
}

fn build_create_index_request_body(payload: &CreateIndexPayload) -> Value {
    json!({
        "name": payload.name,
        "documents": payload.files.iter().map(|file| {
            json!({
                "fileId": file.file_id,
                "filename": file.filename,
                "text": file.text,
                "metadata": file.metadata,
            })
        }).collect::<Vec<_>>(),
    })
}

fn build_graph_rag_query_request_body(
    application_name: &str,
    payload: &GraphRagQueryPayload,
) -> Value {
    json!({
        "applicationName": application_name,
        "question": payload.question,
    })
}

fn env_var(name: &str) -> Result<String, String> {
    env::var(name)
        .map(|value| value.trim().to_string())
        .map_err(|_| format!("{name} environment variable is not set"))
        .and_then(|value| {
            if value.is_empty() {
                Err(format!("{name} environment variable is empty"))
            } else {
                Ok(value)
            }
        })
}

fn auth_headers(api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    let value = HeaderValue::from_str(api_key)
        .map_err(|_| "WM AI Services API key contains invalid header characters".to_string())?;
    headers.insert(API_KEY_HEADER, value);
    Ok(headers)
}

fn service_url(base_url: &str, path: &str) -> Result<Url, String> {
    let mut normalized_base = base_url.trim().to_string();
    if normalized_base.is_empty() {
        return Err("service URL cannot be empty".to_string());
    }
    if !normalized_base.ends_with('/') {
        normalized_base.push('/');
    }

    Url::parse(&normalized_base)
        .and_then(|url| url.join(path.trim_start_matches('/')))
        .map_err(|err| format!("failed to build WM AI Services URL: {err}"))
}

fn validate_path_segment(value: &str, name: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{name} cannot be empty"));
    }
    if value.contains('/') || value.contains('\\') {
        return Err(format!("{name} cannot contain path separators"));
    }

    Ok(())
}

async fn parse_json_response<T>(
    response: reqwest::Response,
    service_name: &str,
) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let response_body = response
        .text()
        .await
        .map_err(|err| format!("failed to read {service_name} response: {err}"))?;

    if !status.is_success() {
        return Err(format!("{service_name} returned {status}: {response_body}"));
    }

    serde_json::from_str(&response_body)
        .map_err(|err| format!("failed to parse {service_name} response: {err}"))
}

#[cfg(test)]
#[path = "../../../tests/core/clients/wm_ai_services_tests.rs"]
mod tests;
