use std::time::Duration;

use base64::{engine::general_purpose, Engine as _};
use futures_util::StreamExt;
use reqwest::multipart::{Form, Part};
use tokio::sync::oneshot;

use super::{
    models::{ChatContextRole, QueryEventPayload, QueryServerEvent, QueryStreamRequest},
    service::QuarryApiService,
};

const MAX_FILES: usize = 20;
const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
const MAX_TOTAL_FILE_BYTES: usize = 50 * 1024 * 1024;
const MAX_BASE64_FILE_CHARS: usize = MAX_FILE_BYTES.div_ceil(3) * 4;
const MAX_CONTEXT_MESSAGES: usize = 64;
const MAX_MESSAGE_CHARS: usize = 100_000;
const MAX_CONTEXT_CHARS: usize = 400_000;
const MAX_INSTRUCTIONS_CHARS: usize = 50_000;
const MAX_MODEL_CHARS: usize = 128;
const MAX_EVENT_BYTES: usize = 1024 * 1024;
const MAX_STREAM_DURATION: Duration = Duration::from_secs(10 * 60);

impl QuarryApiService {
    pub async fn query_events(
        &self,
        request: QueryStreamRequest,
        subscription_id: &str,
        mut cancelled: oneshot::Receiver<()>,
        mut emit: impl FnMut(QueryEventPayload) -> Result<(), String>,
    ) -> Result<(), String> {
        validate_subscription_id(subscription_id)?;
        let form = tokio::select! {
            _ = &mut cancelled => return Ok(()),
            result = build_query_form(request) => result?,
        };
        let response = tokio::select! {
            _ = &mut cancelled => return Ok(()),
            result = self.client.post_query_stream(form) => result?,
        };
        let mut stream = response.bytes_stream();
        let mut parser = QueryRelayParser::default();
        let deadline = tokio::time::Instant::now() + MAX_STREAM_DURATION;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err("Quarry query stream exceeded its maximum duration".to_string());
            }
            let next = tokio::time::timeout(remaining, stream.next());
            tokio::select! {
                _ = &mut cancelled => return Ok(()),
                result = next => {
                    let chunk = result.map_err(|_| "Quarry query stream exceeded its maximum duration".to_string())?;
                    let Some(chunk) = chunk else { break };
                    let bytes = chunk.map_err(|error| format!("Quarry query stream failed: {error}"))?;
                    for event in parser.push(&bytes)? {
                        emit(QueryEventPayload::ServerEvent {
                            event,
                            subscription_id: subscription_id.to_string(),
                        })?;
                    }
                }
            }
        }
        parser.finish()
    }
}

async fn build_query_form(request: QueryStreamRequest) -> Result<Form, String> {
    tauri::async_runtime::spawn_blocking(move || build_query_form_sync(request))
        .await
        .map_err(|error| format!("query upload preparation failed: {error}"))?
}

fn build_query_form_sync(request: QueryStreamRequest) -> Result<Form, String> {
    validate_text("prompt", &request.prompt, MAX_MESSAGE_CHARS)?;
    validate_context(&request.context)?;
    if let Some(model) = request.model.as_deref() {
        let trimmed = model.trim();
        if trimmed.is_empty()
            || trimmed.chars().count() > MAX_MODEL_CHARS
            || trimmed.chars().any(char::is_control)
        {
            return Err("model is invalid".to_string());
        }
    }
    if let Some(instructions) = request.system_instructions.as_deref() {
        validate_text("systemInstructions", instructions, MAX_INSTRUCTIONS_CHARS)?;
    }
    if request.files.len() > MAX_FILES {
        return Err("at most 20 files are allowed".to_string());
    }
    let context =
        serde_json::to_string(&request.context).map_err(|_| "context is invalid".to_string())?;
    let mut form = Form::new()
        .text("prompt", request.prompt)
        .text("context", context);
    if let Some(model) = request.model {
        form = form.text("model", model);
    }
    if let Some(instructions) = request.system_instructions {
        form = form.text("systemInstructions", instructions);
    }
    let mut total = 0usize;
    for file in request.files {
        if file.field_name != "files" || !valid_filename(&file.filename) {
            return Err("query file metadata is invalid".to_string());
        }
        if file.data_base64.len() > MAX_BASE64_FILE_CHARS {
            return Err("query file data exceeds its encoded size limit".to_string());
        }
        let bytes = general_purpose::STANDARD
            .decode(file.data_base64)
            .map_err(|_| "query file data is not valid base64".to_string())?;
        if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES {
            return Err("query file is empty or exceeds 50 MB".to_string());
        }
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| "query files are too large".to_string())?;
        if total > MAX_TOTAL_FILE_BYTES {
            return Err("query files exceed the 50 MB request limit".to_string());
        }
        let part = Part::bytes(bytes)
            .file_name(file.filename)
            .mime_str(if file.mime_type.trim().is_empty() {
                "application/octet-stream"
            } else {
                &file.mime_type
            })
            .map_err(|_| "query file MIME type is invalid".to_string())?;
        form = form.part("files", part);
    }
    Ok(form)
}

fn validate_context(context: &[super::models::ChatContextMessage]) -> Result<(), String> {
    if context.len() > MAX_CONTEXT_MESSAGES || !context.len().is_multiple_of(2) {
        return Err("context must contain complete user/assistant pairs".to_string());
    }
    let mut total = 0usize;
    for (index, message) in context.iter().enumerate() {
        if matches!(message.role, ChatContextRole::User) != (index % 2 == 0) {
            return Err("context roles must alternate user then assistant".to_string());
        }
        validate_text("context message", &message.content, MAX_MESSAGE_CHARS)?;
        total = total
            .checked_add(message.content.chars().count())
            .ok_or_else(|| "context is too large".to_string())?;
    }
    if total > MAX_CONTEXT_CHARS {
        return Err("context is too large".to_string());
    }
    Ok(())
}

fn validate_text(name: &str, value: &str, max: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.chars().count() > max {
        Err(format!("{name} is invalid"))
    } else {
        Ok(())
    }
}

fn valid_filename(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.trim() == value
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_control)
}

fn validate_subscription_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        Err("subscriptionId is invalid".to_string())
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct QueryRelayParser {
    buffer: Vec<u8>,
    started: bool,
    terminal: bool,
}

impl QueryRelayParser {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<QueryServerEvent>, String> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some((end, separator)) = boundary(&self.buffer) {
            if end > MAX_EVENT_BYTES {
                return Err("Quarry query stream event is too large".to_string());
            }
            let frame = self.buffer[..end].to_vec();
            self.buffer.drain(..end + separator);
            if let Some(event) = self.frame(&frame)? {
                events.push(event);
            }
        }
        if self.buffer.len() > MAX_EVENT_BYTES {
            return Err("Quarry query stream event is too large".to_string());
        }
        Ok(events)
    }

    fn finish(&self) -> Result<(), String> {
        if !self.buffer.iter().all(u8::is_ascii_whitespace) {
            return Err("Quarry query stream ended with an incomplete event".to_string());
        }
        if !self.terminal {
            return Err("Quarry query stream ended before a terminal event".to_string());
        }
        Ok(())
    }

    fn frame(&mut self, bytes: &[u8]) -> Result<Option<QueryServerEvent>, String> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| "Quarry query stream contained invalid UTF-8".to_string())?;
        let mut name = None;
        let mut data = Vec::new();
        for source_line in text.lines() {
            let line = source_line.trim_end_matches('\r');
            if line.starts_with(':') {
                continue;
            }
            if let Some(value) = line.strip_prefix("event:") {
                name = Some(value.trim());
            }
            if let Some(value) = line.strip_prefix("data:") {
                data.push(value.strip_prefix(' ').unwrap_or(value));
            }
        }
        if name.is_none() && data.is_empty() {
            return Ok(None);
        }
        let name = name.ok_or_else(|| "Quarry query stream event omitted its name".to_string())?;
        if self.terminal {
            return Err("Quarry query stream sent data after its terminal event".to_string());
        }
        let event: QueryServerEvent = serde_json::from_str(&data.join("\n"))
            .map_err(|_| "Quarry query stream returned an invalid event".to_string())?;
        match &event {
            QueryServerEvent::Started { model } if model.is_empty() => {
                return Err("Quarry query stream returned an invalid started event".to_string());
            }
            QueryServerEvent::Completed { response } if response.trim().is_empty() => {
                return Err("Quarry query stream returned an empty completion".to_string());
            }
            QueryServerEvent::Failed { error } if error.is_empty() => {
                return Err("Quarry query stream returned an invalid failure".to_string());
            }
            _ => {}
        }
        if event.event_name() != name {
            return Err("Quarry query event name did not match its payload".to_string());
        }
        if !self.started && !matches!(event, QueryServerEvent::Started { .. }) {
            return Err("Quarry query stream did not start correctly".to_string());
        }
        if self.started && matches!(event, QueryServerEvent::Started { .. }) {
            return Err("Quarry query stream started more than once".to_string());
        }
        if matches!(event, QueryServerEvent::Started { .. }) {
            self.started = true;
        }
        if event.is_terminal() {
            self.terminal = true;
        }
        Ok(Some(event))
    }
}

fn boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    let lf = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2));
    let crlf = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(test)]
#[path = "../../tests/quarry_api/query_stream_tests.rs"]
mod tests;
