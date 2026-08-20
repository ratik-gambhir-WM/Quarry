use base64::{engine::general_purpose, Engine as _};
use futures_util::StreamExt;
use reqwest::multipart::{Form, Part};
use serde_json::Value;

use super::{
    client::QuarryHttpClient,
    models::{DocumentJobEventPayload, MultipartRequest},
};

const MAX_PROXY_FILE_BYTES: usize = 50 * 1024 * 1024;
const MAX_PROXY_TOTAL_BYTES: usize = 50 * 1024 * 1024;

#[derive(Clone)]
pub struct QuarryApiService {
    client: QuarryHttpClient,
}

impl QuarryApiService {
    pub fn from_environment() -> Result<Self, String> {
        Ok(Self {
            client: QuarryHttpClient::from_environment()?,
        })
    }

    pub async fn get(&self, path: &str) -> Result<Value, String> {
        validate_api_path(path)?;
        self.client.get(path).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        validate_api_path(path)?;
        self.client.post(path, body).await
    }

    pub async fn post_multipart(&self, request: MultipartRequest) -> Result<Value, String> {
        validate_api_path(&request.path)?;
        let mut total_bytes = 0usize;
        let mut form = Form::new();
        for field in request.fields {
            if field.name.trim().is_empty() || field.name.len() > 64 {
                return Err("multipart field name is invalid".to_string());
            }
            form = form.text(field.name, field.value);
        }
        for file in request.files {
            if file.field_name.trim().is_empty()
                || file.filename.trim().is_empty()
                || file.filename.starts_with(['/', '\\'])
                || file.filename.split(['/', '\\']).any(|part| part == "..")
                || file.filename.chars().any(char::is_control)
            {
                return Err("multipart file metadata is invalid".to_string());
            }
            let bytes = general_purpose::STANDARD
                .decode(file.data_base64)
                .map_err(|_| "multipart file data is not valid base64".to_string())?;
            if bytes.is_empty() || bytes.len() > MAX_PROXY_FILE_BYTES {
                return Err("multipart file is empty or exceeds 50 MB".to_string());
            }
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .ok_or_else(|| "multipart request is too large".to_string())?;
            if total_bytes > MAX_PROXY_TOTAL_BYTES {
                return Err("multipart files exceed the 50 MB request limit".to_string());
            }
            let part = Part::bytes(bytes)
                .file_name(file.filename)
                .mime_str(&file.mime_type)
                .map_err(|_| "multipart MIME type is invalid".to_string())?;
            form = form.part(file.field_name, part);
        }
        self.client.post_multipart(&request.path, form).await
    }

    pub async fn document_job_events(
        &self,
        job_id: &str,
        subscription_id: &str,
        mut emit: impl FnMut(DocumentJobEventPayload) -> Result<(), String>,
    ) -> Result<(), String> {
        validate_identifier("jobId", job_id)?;
        validate_identifier("subscriptionId", subscription_id)?;
        let path = format!("/api/v1/documents/process_file/{job_id}/events");
        let response = self.client.get_stream(&path).await?;
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| format!("Quarry event stream failed: {error}"))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(boundary) = buffer.find("\n\n") {
                let event = buffer[..boundary].to_string();
                buffer.drain(..boundary + 2);
                let mut event_name = "message".to_string();
                let mut data = String::new();
                for line in event.lines() {
                    if let Some(value) = line.strip_prefix("event:") {
                        event_name = value.trim().to_string();
                    } else if let Some(value) = line.strip_prefix("data:") {
                        data.push_str(value.trim());
                    }
                }
                if !data.is_empty() {
                    emit(DocumentJobEventPayload {
                        data,
                        event_name,
                        subscription_id: subscription_id.to_string(),
                    })?;
                }
            }
        }
        Ok(())
    }
}

fn validate_api_path(path: &str) -> Result<(), String> {
    if !path.starts_with("/api/v1/")
        || path.contains("..")
        || path.contains(['\r', '\n', '#'])
        || path.len() > 2048
    {
        return Err("Quarry API path is not allowed".to_string());
    }
    Ok(())
}

fn validate_identifier(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(format!("{name} is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restricts_proxy_paths_to_the_versioned_quarry_api() {
        assert!(validate_api_path("/api/v1/deals").is_ok());
        assert!(validate_api_path("https://example.com/api/v1/deals").is_err());
        assert!(validate_api_path("/api/v1/../secrets").is_err());
    }
}
