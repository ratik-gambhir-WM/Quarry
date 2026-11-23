use futures_util::StreamExt;
use serde_json::Value;

use super::{
    models::{DocumentJobEventPayload, MultipartRequest},
    service::{build_multipart_form, validate_identifier, QuarryApiService},
};

impl QuarryApiService {
    pub async fn upload_document(
        &self,
        request: MultipartRequest,
        subscription_id: &str,
        mut emit: impl FnMut(DocumentJobEventPayload) -> Result<(), String>,
    ) -> Result<Value, String> {
        validate_identifier("subscriptionId", subscription_id)?;
        let parts: Vec<_> = request.path.split('/').collect();
        if parts.len() != 7
            || parts[1..4] != ["api", "v1", "deals"]
            || parts[4].is_empty()
            || parts[5..] != ["documents", "upload"]
            || request.path.contains(['\r', '\n', '#', '?'])
            || request.path.contains("..")
            || request.path.len() > 2048
            || request.files.len() != 1
            || !request.fields.is_empty()
        {
            return Err("document upload request is invalid".into());
        }
        let path = request.path.clone();
        let form = tauri::async_runtime::spawn_blocking(move || build_multipart_form(request))
            .await
            .map_err(|_| "document upload preparation interrupted".to_string())??;
        let response = self.client.post_query_stream(&path, form).await?;
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();
        while let Some(bytes) = stream.next().await {
            buffer.extend_from_slice(
                &bytes.map_err(|_| "document upload stream interrupted".to_string())?,
            );
            if buffer.len() > 65536 {
                return Err("upload progress event exceeds size limit".into());
            }
            while let Some((end, length)) = boundary(&buffer) {
                let text = std::str::from_utf8(&buffer[..end])
                    .map_err(|_| "invalid upload progress UTF-8".to_string())?;
                let data = text
                    .lines()
                    .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !data.is_empty() {
                    let event = parse_progress(&data)?;
                    let status = event["status"].as_str().ok_or("missing upload status")?;
                    emit(DocumentJobEventPayload {
                        data,
                        event_name: status.into(),
                        subscription_id: subscription_id.into(),
                    })?;
                    if status == "accepted" {
                        return Ok(event);
                    }
                    if status == "failed" {
                        return Err("document preparation failed; retry the same file".into());
                    }
                }
                buffer.drain(..end + length);
            }
        }
        Err("upload stream ended before acceptance; retry the same file".into())
    }
}

fn boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    bytes
        .windows(2)
        .position(|part| part == b"\n\n")
        .map(|end| (end, 2))
        .or_else(|| {
            bytes
                .windows(4)
                .position(|part| part == b"\r\n\r\n")
                .map(|end| (end, 4))
        })
}

fn parse_progress(data: &str) -> Result<Value, String> {
    let event: Value =
        serde_json::from_str(data).map_err(|_| "invalid upload progress JSON".to_string())?;
    if event["jobId"].as_str().is_none_or(str::is_empty)
        || event["filename"].as_str().is_none()
        || !matches!(
            event["status"].as_str(),
            Some("parsing" | "classifying" | "accepted" | "failed")
        )
    {
        return Err("invalid upload progress event".into());
    }
    Ok(event)
}

#[cfg(test)]
#[path = "../../tests/quarry_api/upload_tests.rs"]
mod tests;
