use std::{
    fmt, fs,
    future::Future,
    num::NonZeroUsize,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use base64::{engine::general_purpose, Engine as _};
use futures_util::StreamExt;
use rig::{
    catalog::{Catalog, ModelFacts},
    completion::CompletionRequest,
    embeddings::EmbeddingResponse,
    message::Message,
    observe::{Action, AdapterContext, AdapterEvent, Observation, Subject, Witness},
    providers::openai::{
        embedding::EncodingFormat,
        extension::{OpenAiOptions, OpenAiResponsesOptions},
        responses_api::wire::Responses,
        OpenAI, OpenAIConfig as RigOpenAiConfig,
    },
    streaming::{Item, StreamEvent},
    Model, ProviderError,
};
use serde_json::{json, Value};
use tracing::instrument::WithSubscriber;

use crate::{
    app::config::OpenAiConfig,
    shared::prompts::{PromptCatalog, PromptId},
};

const DEFAULT_RESPONSES_MODEL: &str = "gpt-5.5";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_EMBEDDING_DIMENSIONS: NonZeroUsize =
    NonZeroUsize::new(1536).expect("default embedding dimensions are non-zero");
const OPENAI_EMBEDDINGS_API: &str = "openai.embeddings";
const OPENAI_RESPONSES_API: &str = "openai.responses";
pub(super) const CHAT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const CHAT_IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const CHAT_MAX_DURATION: Duration = Duration::from_secs(10 * 60);
const MAX_CHAT_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct OpenAiClient {
    provider: OpenAI,
    embedding_dimensions: NonZeroUsize,
    prompts: Arc<PromptCatalog>,
}

#[derive(Clone, Debug)]
pub struct ChatMessageInput {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Clone, Copy, Debug)]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug)]
pub struct ChatStreamError {
    category: &'static str,
    detail: String,
}

impl ChatStreamError {
    pub fn category(&self) -> &'static str {
        self.category
    }

    fn new(category: &'static str, detail: impl Into<String>) -> Self {
        Self {
            category,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ChatStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

pub enum ResponsesFileInput<'a> {
    FileId(&'a str),
    FileUrl(&'a str),
    FileData {
        filename: &'a str,
        mime_type: &'a str,
        data_base64: &'a str,
    },
    ImageData {
        mime_type: &'a str,
        data_base64: &'a str,
        detail: Option<&'a str>,
    },
    FilePath(&'a Path),
}

impl OpenAiClient {
    pub fn new(api_key: impl Into<String>, prompts: Arc<PromptCatalog>) -> Self {
        Self {
            provider: RigOpenAiConfig::new(api_key.into()).client(),
            embedding_dimensions: DEFAULT_EMBEDDING_DIMENSIONS,
            prompts,
        }
    }

    pub fn from_config(config: &OpenAiConfig, prompts: Arc<PromptCatalog>) -> Self {
        Self {
            provider: RigOpenAiConfig::new(config.api_key.expose()).client(),
            embedding_dimensions: config.embedding_dimensions,
            prompts,
        }
    }

    pub fn prompts(&self) -> &PromptCatalog {
        &self.prompts
    }

    fn prepare_responses_request(
        &self,
        prompt: Option<&str>,
        system_instructions: Option<&str>,
        model: Option<&str>,
        file_inputs: Option<&[ResponsesFileInput<'_>]>,
    ) -> Result<(Model<Responses>, CompletionRequest), String> {
        let prompt = prompt
            .unwrap_or_else(|| self.prompts.prompt(PromptId::DefaultResponse))
            .trim();
        let instructions = system_instructions
            .unwrap_or_else(|| self.prompts.prompt(PromptId::DefaultInstructions))
            .trim();
        let model = model.unwrap_or(DEFAULT_RESPONSES_MODEL).trim();
        if prompt.is_empty() {
            return Err("prompt cannot be empty".into());
        }
        if instructions.is_empty() {
            return Err("system instructions cannot be empty".into());
        }
        if model.is_empty() {
            return Err("model cannot be empty".into());
        }
        Ok((
            self.responses_model(model),
            completion_request(&[], prompt, instructions, file_inputs, false)?,
        ))
    }

    fn responses_model(&self, model: &str) -> Model<Responses> {
        rig_responses_model(&self.provider, model).with_idle_timeout(Some(CHAT_IDLE_TIMEOUT))
    }

    async fn stream_response<F, Fut>(
        model: Model<Responses>,
        request: CompletionRequest,
        mut on_delta: F,
    ) -> Result<(String, String), ChatStreamError>
    where
        F: FnMut(String) -> Fut + Send,
        Fut: Future<Output = Result<(), ChatStreamError>> + Send,
    {
        let connection = Arc::new(StreamConnection::default());
        let observation = AdapterContext::new(
            connection.clone(),
            Subject::scoped("openai.responses"),
            uuid::Uuid::new_v4().to_string(),
        );
        let mut stream = tracing::subscriber::with_default(
            tracing::subscriber::NoSubscriber::default(),
            || {
                model
                    .stream_observed(request, observation)
                    .map_err(chat_provider_error)
            },
        )?;
        let mut streamed = String::new();
        let connect_deadline = tokio::time::Instant::now() + CHAT_CONNECT_TIMEOUT;
        loop {
            let next = stream.next().with_subscriber(tracing::Dispatch::none());
            tokio::pin!(next);
            let item = if connection.received_headers.load(Ordering::Relaxed) {
                next.await
            } else {
                tokio::select! {
                    item = &mut next => item,
                    _ = tokio::time::sleep_until(connect_deadline) => {
                        if !connection.received_headers.load(Ordering::Relaxed) {
                            return Err(ChatStreamError::new("connect_timeout", "OpenAI chat connection timed out"));
                        }
                        next.await
                    }
                }
            };
            let Some(item) = item else { break };
            let item = item.map_err(chat_provider_error)?;
            if let Item::Event(StreamEvent::Text { text, .. }) = item {
                if streamed.len().saturating_add(text.len()) > MAX_CHAT_RESPONSE_BYTES {
                    return Err(ChatStreamError::new(
                        "provider_stream",
                        "OpenAI chat response exceeded its size limit",
                    ));
                }
                streamed.push_str(&text);
                on_delta(text).await?;
            }
        }
        let response = stream
            .finish()
            .with_subscriber(tracing::Dispatch::none())
            .await
            .map_err(chat_provider_error)?;
        if matches!(
            response.raw["status"].as_str(),
            Some("failed" | "incomplete" | "cancelled")
        ) {
            return Err(ChatStreamError::new(
                "provider_stream",
                "OpenAI chat stream reported a provider failure",
            ));
        }
        let completed = response.text();
        if completed.len() > MAX_CHAT_RESPONSE_BYTES {
            return Err(ChatStreamError::new(
                "provider_stream",
                "OpenAI chat response exceeded its size limit",
            ));
        }
        if completed.trim().is_empty() && streamed.trim().is_empty() {
            return Err(ChatStreamError::new(
                "provider_stream",
                "OpenAI chat stream completed without output text",
            ));
        }
        Ok((streamed, completed))
    }

    pub async fn gen_model_response(
        &self,
        prompt: Option<&str>,
        system_instructions: Option<&str>,
        model: Option<&str>,
    ) -> Result<String, String> {
        self.gen_model_response_with_files(prompt, system_instructions, model, None)
            .await
    }

    pub async fn gen_model_response_with_files(
        &self,
        prompt: Option<&str>,
        system_instructions: Option<&str>,
        model: Option<&str>,
        file_inputs: Option<&[ResponsesFileInput<'_>]>,
    ) -> Result<String, String> {
        let (model, request) =
            self.prepare_responses_request(prompt, system_instructions, model, file_inputs)?;
        let started = Instant::now();
        let response = model
            .call(request)
            .with_subscriber(tracing::Dispatch::none())
            .await
            .map_err(|error| provider_error("responses", error, None, None, false, started))?;
        log_openai_success(OPENAI_RESPONSES_API, None, None, started);
        let text = response.text();
        let text = text.trim();
        if text.is_empty() {
            Err("OpenAI responses API did not include output text".into())
        } else {
            Ok(text.to_owned())
        }
    }

    pub async fn gen_model_response_with_files_streaming<F>(
        &self,
        prompt: Option<&str>,
        system_instructions: Option<&str>,
        model: Option<&str>,
        file_inputs: Option<&[ResponsesFileInput<'_>]>,
        mut on_text_delta: F,
    ) -> Result<String, String>
    where
        F: FnMut(&str) + Send,
    {
        let (model, request) =
            self.prepare_responses_request(prompt, system_instructions, model, file_inputs)?;
        let (streamed, completed) = tokio::time::timeout(
            CHAT_MAX_DURATION,
            Self::stream_response(model, request, |text| {
                on_text_delta(&text);
                std::future::ready(Ok(()))
            }),
        )
        .await
        .map_err(|_| "OpenAI response exceeded its maximum duration".to_string())?
        .map_err(|error| error.to_string())?;
        Ok(if streamed.trim().is_empty() {
            completed
        } else {
            streamed
        })
    }

    pub async fn gen_chat_response_streaming(
        &self,
        context: &[ChatMessageInput],
        prompt: &str,
        system_instructions: &str,
        model: &str,
        file_inputs: &[ResponsesFileInput<'_>],
        delta_sender: tokio::sync::mpsc::Sender<String>,
    ) -> Result<String, ChatStreamError> {
        if prompt.trim().is_empty()
            || system_instructions.trim().is_empty()
            || model.trim().is_empty()
        {
            return Err(ChatStreamError::new("validation", "chat input is invalid"));
        }
        let model = self.responses_model(model);
        let request = completion_request(
            context,
            prompt,
            system_instructions,
            Some(file_inputs),
            true,
        )
        .map_err(|error| ChatStreamError::new("validation", error))?;
        let (streamed, completed) = tokio::time::timeout(
            CHAT_MAX_DURATION,
            Self::stream_response(model, request, |text| {
                let sender = delta_sender.clone();
                async move {
                    sender
                        .send(text)
                        .await
                        .map_err(|_| ChatStreamError::new("cancelled", "chat receiver closed"))
                }
            }),
        )
        .await
        .map_err(|_| {
            ChatStreamError::new(
                "max_duration",
                "OpenAI chat stream exceeded its maximum duration",
            )
        })??;
        Ok(if completed.trim().is_empty() {
            streamed
        } else {
            completed
        })
    }

    pub async fn gen_embedding(
        &self,
        content: &str,
        model: Option<&str>,
    ) -> Result<Vec<f64>, String> {
        if content.trim().is_empty() {
            return Err("cannot embed empty document content".to_string());
        }

        let contents = [content];
        let mut embeddings = self.gen_embeddings(&contents, model).await?;
        Ok(embeddings
            .pop()
            .expect("a successful single-input embedding request should return one embedding"))
    }

    /// Embeds a document's chunks in one request, preserving input order.
    pub async fn gen_embeddings(
        &self,
        contents: &[&str],
        model: Option<&str>,
    ) -> Result<Vec<Vec<f64>>, String> {
        self.gen_embeddings_with_filename(contents, model, None, None, false)
            .await
    }

    pub async fn gen_embeddings_for_file(
        &self,
        contents: &[&str],
        model: Option<&str>,
        filename: &str,
        file_size_bytes: u64,
    ) -> Result<Vec<Vec<f64>>, String> {
        self.gen_embeddings_with_filename(
            contents,
            model,
            Some(filename),
            Some(file_size_bytes),
            false,
        )
        .await
    }

    /// Embeds private notebook content without including provider error bodies in diagnostics.
    pub async fn gen_private_embeddings(
        &self,
        contents: &[&str],
        model: Option<&str>,
    ) -> Result<Vec<Vec<f64>>, String> {
        self.gen_embeddings_with_filename(contents, model, None, None, true)
            .await
    }

    async fn gen_embeddings_with_filename(
        &self,
        contents: &[&str],
        model: Option<&str>,
        filename: Option<&str>,
        file_size_bytes: Option<u64>,
        private: bool,
    ) -> Result<Vec<Vec<f64>>, String> {
        let model = embedding_model_name(contents, model)?;
        let mut embedder = self
            .provider
            .embedding(model, Some(self.embedding_dimensions.get()));
        embedder.wire = embedder.wire.with_encoding_format(EncodingFormat::Float);
        let started = Instant::now();
        let response = embedder
            .call(
                contents
                    .iter()
                    .map(|text| (*text).to_owned())
                    .collect::<Vec<_>>(),
            )
            .with_subscriber(tracing::Dispatch::none())
            .await
            .map_err(|error| {
                provider_error(
                    "embeddings",
                    error,
                    filename,
                    file_size_bytes,
                    private,
                    started,
                )
            })?;
        log_openai_success(OPENAI_EMBEDDINGS_API, filename, file_size_bytes, started);
        validate_embeddings(response, contents.len(), self.embedding_dimensions)
    }
}

pub(crate) fn rig_responses_model(provider: &OpenAI, model: &str) -> Model<Responses> {
    let mut model = provider.responses(model);
    // Quarry requires arbitrary_precision; Rig's builtin catalog cannot decode it.
    model.wire.facts = ModelFacts::default().with_catalog(&Catalog::default());
    model
}

/// Rig's response-header observation separates connection timeout from model
/// thinking time without replacing or wrapping Rig's HTTP client.
#[derive(Default)]
struct StreamConnection {
    received_headers: AtomicBool,
}

impl Witness for StreamConnection {
    fn observe(&self, observation: Observation) {
        if matches!(observation.action, Action::Adapter { observation } if matches!(observation.event, AdapterEvent::Response { .. }))
        {
            self.received_headers.store(true, Ordering::Relaxed);
        }
    }
}

fn embedding_model_name<'a>(contents: &[&str], model: Option<&'a str>) -> Result<&'a str, String> {
    let model = model.unwrap_or(DEFAULT_EMBEDDING_MODEL).trim();
    if contents.is_empty() {
        return Err("cannot embed an empty list of document contents".into());
    }
    if let Some(index) = contents.iter().position(|text| text.trim().is_empty()) {
        return Err(format!(
            "cannot embed empty document content at input index {index}"
        ));
    }
    if model.is_empty() {
        return Err("model cannot be empty".into());
    }
    Ok(model)
}

fn provider_error(
    api: &str,
    error: ProviderError,
    filename: Option<&str>,
    file_size_bytes: Option<u64>,
    private: bool,
    started: Instant,
) -> String {
    tracing::error!(api, filename, file_size_bytes, kind = ?error.kind(), elapsed_seconds = started.elapsed().as_secs_f64(), "OpenAI call failed");
    if let ProviderError::MismatchedDimensions { .. } = error {
        return "OpenAI embedding dimension does not match the configured index".into();
    }
    if let Some(status) = error.provider_response_status() {
        return if private {
            format!("OpenAI {api} API returned {status}")
        } else {
            format!(
                "OpenAI {api} API returned {status}: {}",
                error.provider_response_body().unwrap_or_default()
            )
        };
    }
    format!("failed to call OpenAI {api} API")
}

fn chat_provider_error(error: ProviderError) -> ChatStreamError {
    let category = if error.provider_response_status().is_some() {
        "provider_status"
    } else if matches!(error, ProviderError::Stalled(_)) {
        "idle_timeout"
    } else {
        "provider_stream"
    };
    let detail = if let Some(status) = error.provider_response_status() {
        format!(
            "OpenAI responses API returned {status}: {}",
            error.provider_response_body().unwrap_or_default()
        )
    } else {
        "OpenAI response stream failed".into()
    };
    ChatStreamError::new(category, detail)
}

fn log_openai_success(
    api: &str,
    filename: Option<&str>,
    file_size_bytes: Option<u64>,
    started_at: Instant,
) {
    tracing::info!(
        api,
        filename = filename.unwrap_or("-"),
        file_size_bytes = file_size_bytes.unwrap_or_default(),
        elapsed_seconds = started_at.elapsed().as_secs_f64(),
    );
}

/// Rig's typed Responses input extension carries the file formats and original
/// names that its generic document type cannot yet represent. No HTTP mapping is
/// needed: Rig encodes, sends, and decodes this request through its normal model.
fn completion_request(
    context: &[ChatMessageInput],
    prompt: &str,
    instructions: &str,
    files: Option<&[ResponsesFileInput<'_>]>,
    private: bool,
) -> Result<CompletionRequest, String> {
    let mut options = OpenAiOptions::new();
    if private {
        options = options.store(false);
    }
    let request = if let Some(files) = files.filter(|files| !files.is_empty()) {
        let mut input = context.iter().map(|message| json!({
            "type": "message",
            "role": match message.role { ChatRole::User => "user", ChatRole::Assistant => "assistant" },
            "content": [{"type": match message.role { ChatRole::User => "input_text", ChatRole::Assistant => "output_text" }, "text": message.content}],
        })).collect::<Vec<_>>();
        let mut content = files
            .iter()
            .map(build_input_item)
            .collect::<Result<Vec<_>, _>>()?;
        content.push(json!({"type": "input_text", "text": prompt}));
        input.push(json!({"type": "message", "role": "user", "content": content}));
        options = options.responses(OpenAiResponsesOptions::default().input(input));
        // Rig lifts this system message into top-level instructions. The typed
        // provider input above already contains the complete ordered conversation.
        CompletionRequest::new(Message::system(instructions))
    } else {
        CompletionRequest::new(prompt)
            .preamble(instructions)
            .messages(context.iter().map(|message| match message.role {
                ChatRole::User => Message::user(message.content.clone()),
                ChatRole::Assistant => Message::assistant(message.content.clone()),
            }))
    };
    Ok(request.provider_option(options))
}

fn build_input_item(file_input: &ResponsesFileInput<'_>) -> Result<Value, String> {
    match file_input {
        ResponsesFileInput::FileId(file_id) => {
            let file_id = file_id.trim();
            if file_id.is_empty() {
                return Err("file_id cannot be empty".to_string());
            }

            Ok(json!({
                "type": "input_file",
                "file_id": file_id,
            }))
        }
        ResponsesFileInput::FileUrl(file_url) => {
            let file_url = file_url.trim();
            if file_url.is_empty() {
                return Err("file_url cannot be empty".to_string());
            }

            Ok(json!({
                "type": "input_file",
                "file_url": file_url,
            }))
        }
        ResponsesFileInput::FileData {
            filename,
            mime_type,
            data_base64,
        } => {
            let filename = filename.trim();
            let mime_type = mime_type.trim();
            let data_base64 = data_base64.trim();

            if filename.is_empty() {
                return Err("filename cannot be empty".to_string());
            }

            if mime_type.is_empty() {
                return Err("mime_type cannot be empty".to_string());
            }

            if data_base64.is_empty() {
                return Err("file_data cannot be empty".to_string());
            }

            Ok(json!({
                "type": "input_file",
                "filename": filename,
                "file_data": build_base64_data_url(mime_type, data_base64),
            }))
        }
        ResponsesFileInput::ImageData {
            mime_type,
            data_base64,
            detail,
        } => {
            let mime_type = mime_type.trim();
            let data_base64 = data_base64.trim();
            let detail = detail.unwrap_or("auto").trim();

            if mime_type.is_empty() {
                return Err("mime_type cannot be empty".to_string());
            }

            if data_base64.is_empty() {
                return Err("image_data cannot be empty".to_string());
            }

            if detail.is_empty() {
                return Err("image detail cannot be empty".to_string());
            }

            Ok(json!({
                "type": "input_image",
                "image_url": build_base64_data_url(mime_type, data_base64),
                "detail": detail,
            }))
        }
        ResponsesFileInput::FilePath(path) => {
            let file_bytes = fs::read(path)
                .map_err(|err| format!("failed to read file input {}: {err}", path.display()))?;
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?;
            let mime_type = infer_file_mime_type(path);
            let file_data =
                build_base64_data_url(mime_type, &general_purpose::STANDARD.encode(file_bytes));

            Ok(json!({
                "type": "input_file",
                "filename": filename,
                "file_data": file_data,
            }))
        }
    }
}

fn build_base64_data_url(mime_type: &str, data_base64: &str) -> String {
    format!("data:{mime_type};base64,{data_base64}")
}

fn infer_file_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        Some("json") => "application/json",
        Some("html") => "text/html",
        Some("csv") => "text/csv",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    }
}

// Rig 0.46 decodes vectors in wire order and discards their indices. Read only
// that metadata from raw; numeric decoding belongs to Rig. These upstream-data
// integrity checks must run here, after the provider reply, before storage.
fn validate_embeddings(
    response: EmbeddingResponse,
    expected_count: usize,
    expected_dimensions: NonZeroUsize,
) -> Result<Vec<Vec<f64>>, String> {
    let items = response.raw["data"]
        .as_array()
        .ok_or_else(|| "OpenAI embeddings response did not include embeddings".to_string())?;
    let mut embeddings = vec![None; expected_count];

    if items.len() != response.embeddings.len() {
        return Err("OpenAI embeddings response count does not match its decoded vectors".into());
    }
    for (item, embedding) in items.iter().zip(response.embeddings) {
        let index = item["index"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                "OpenAI embeddings response included an invalid embedding index".to_string()
            })?;
        let destination = embeddings.get_mut(index).ok_or_else(|| {
            format!("OpenAI embeddings response included out-of-range embedding index {index}")
        })?;
        if destination.is_some() {
            return Err(format!(
                "OpenAI embeddings response included duplicate embedding index {index}"
            ));
        }
        if embedding.vec.len() != expected_dimensions.get() {
            return Err("OpenAI embedding dimension does not match the configured index".into());
        }
        if embedding
            .vec
            .iter()
            .any(|number| !number.is_finite() || !(*number as f32).is_finite())
        {
            return Err("OpenAI embedding contained a non-finite or out-of-range value".into());
        }
        *destination = Some(embedding.vec);
    }

    embeddings
        .into_iter()
        .enumerate()
        .map(|(index, embedding)| {
            embedding.ok_or_else(|| {
                format!("OpenAI embeddings response did not include embedding at index {index}")
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit/adapters/openai/client_tests.rs"]
mod tests;
