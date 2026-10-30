//! Notebook-specific PATCH and cancellable SSE transport; no product persistence or policy.
use super::{models::DocumentJobEventPayload, service::QuarryApiService};
use futures_util::StreamExt;
use serde_json::Value;
use std::time::Duration;

pub(super) const MAX_NOTEBOOK_JSON: usize = 2 * 1024 * 1024;
pub(super) fn canonical_id(id: &str) -> Result<(), String> {
    if uuid::Uuid::parse_str(id).is_ok_and(|value| value.to_string() == id) {
        Ok(())
    } else {
        Err("invalid notebook resource identifier".into())
    }
}
pub(super) fn validate_patch(path: &str, body: &Value) -> Result<(), String> {
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() != 5 && parts.len() != 7 {
        return Err("notebook PATCH path is not allowed".into());
    }
    if parts[0..4] != ["", "api", "v1", "notebooks"] {
        return Err("notebook PATCH path is not allowed".into());
    }
    canonical_id(parts[4])?;
    if parts.len() == 7 {
        if !["folders", "notes"].contains(&parts[5]) {
            return Err("notebook PATCH path is not allowed".into());
        }
        canonical_id(parts[6])?;
    }
    validate_body(body)
}
pub(super) fn validate_body(body: &Value) -> Result<(), String> {
    if !body.is_object()
        || serde_json::to_vec(body)
            .map_err(|_| "invalid notebook JSON")?
            .len()
            > MAX_NOTEBOOK_JSON
    {
        return Err("notebook JSON exceeds the 2 MiB limit or is not an object".into());
    }
    Ok(())
}
impl QuarryApiService {
    pub async fn patch(&self, path: &str, body: Value) -> Result<Value, String> {
        validate_patch(path, &body)?;
        self.client.patch(path, body).await
    }
    pub async fn notebook_job_events(
        &self,
        notebook: &str,
        job: &str,
        subscription: &str,
        mut emit: impl FnMut(DocumentJobEventPayload) -> Result<(), String>,
    ) -> Result<(), String> {
        canonical_id(notebook)?;
        canonical_id(job)?;
        canonical_id(subscription)?;
        let path = format!("/api/v1/notebooks/{notebook}/save-jobs/{job}/events");
        let response = self.client.get_stream(&path).await?;
        if !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/event-stream"))
        {
            return Err("invalid notebook event content type".into());
        }
        let mut stream = response.bytes_stream();
        let mut parser = NotebookRelayParser::default();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
        loop {
            let next = tokio::time::timeout_at(deadline, stream.next())
                .await
                .map_err(|_| "notebook stream exceeded maximum duration")?;
            let Some(bytes) = next else {
                return Err("notebook stream disconnected; check job status".into());
            };
            for (event_name, data) in parser.push(&bytes.map_err(|_| "notebook stream failed")?)? {
                let value: Value =
                    serde_json::from_str(&data).map_err(|_| "invalid notebook event")?;
                if value["notebookId"] != notebook
                    || value["jobId"] != job
                    || value["status"] != event_name
                {
                    return Err("notebook event scope mismatch".into());
                }
                let terminal = event_name == "completed" || event_name == "failed";
                emit(DocumentJobEventPayload {
                    data,
                    event_name,
                    subscription_id: subscription.to_owned(),
                })?;
                if terminal {
                    return Ok(());
                }
            }
        }
    }
}
#[derive(Default)]
struct NotebookRelayParser {
    buffer: Vec<u8>,
}
impl NotebookRelayParser {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
        if self.buffer.len() + bytes.len() > 65536 {
            return Err("notebook event exceeds size limit".into());
        }
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        loop {
            let found = self
                .buffer
                .windows(2)
                .position(|v| v == b"\n\n")
                .map(|v| (v, 2))
                .or_else(|| {
                    self.buffer
                        .windows(4)
                        .position(|v| v == b"\r\n\r\n")
                        .map(|v| (v, 4))
                });
            let Some((end, separator)) = found else {
                break;
            };
            let text =
                std::str::from_utf8(&self.buffer[..end]).map_err(|_| "invalid event UTF-8")?;
            let mut event = String::new();
            let mut data = Vec::new();
            for line in text.lines() {
                if let Some(value) = line.strip_prefix("event:") {
                    event = value.trim().into();
                }
                if let Some(value) = line.strip_prefix("data:") {
                    data.push(value.strip_prefix(' ').unwrap_or(value));
                }
            }
            if !data.is_empty() {
                if !["queued", "processing", "completed", "failed"].contains(&event.as_str()) {
                    return Err("invalid notebook event name".into());
                }
                events.push((event, data.join("\n")));
            }
            self.buffer.drain(..end + separator);
        }
        Ok(events)
    }
}
#[cfg(test)]
#[path = "../../tests/unit/quarry_api/notebooks_tests.rs"]
mod tests;
