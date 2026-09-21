use std::{sync::Arc, time::Instant};

use base64::{engine::general_purpose, Engine as _};
use tokio::sync::mpsc;

use crate::{
    adapters::openai::client::{ChatMessageInput, ChatRole, OpenAiClient, ResponsesFileInput},
    domains::users::UserDirectory,
    shared::error::{RepositoryError, ServiceError, ServiceResult},
};

pub use super::repository::{AssistantThread, AssistantThreadDetail};

use super::{
    model::{
        into_provider_context, PersistentQueryInput, QueryModelInput, SendQueryEvent,
        MAX_CONTEXT_CHARS,
    },
    repository::{AssistantChatRepository, StartRunInput, StartRunResult},
    upload::ChatUploadKind,
};

const DEFAULT_CHAT_INSTRUCTIONS: &str = "You are a helpful assistant.";
const EVENT_CHANNEL_CAPACITY: usize = 32;

pub struct AssistantChatService {
    repository: Option<AssistantChatRepository>,
    users: Option<UserDirectory>,
    openai: Option<Arc<OpenAiClient>>,
    model: String,
}

impl AssistantChatService {
    pub fn new(openai: Option<Arc<OpenAiClient>>, model: String) -> Self {
        Self {
            repository: None,
            users: None,
            openai,
            model,
        }
    }

    pub fn with_persistence(
        openai: Option<Arc<OpenAiClient>>,
        model: String,
        repository: AssistantChatRepository,
        users: UserDirectory,
    ) -> Self {
        Self {
            repository: Some(repository),
            users: Some(users),
            openai,
            model,
        }
    }

    pub fn ask(&self, input: QueryModelInput) -> ServiceResult<mpsc::Receiver<SendQueryEvent>> {
        let client = self
            .openai
            .as_ref()
            .cloned()
            .ok_or_else(|| ServiceError::unavailable("OpenAI capability is not configured"))?;
        let model = input.model.unwrap_or_else(|| self.model.clone());
        let instructions = input
            .system_instructions
            .unwrap_or_else(|| DEFAULT_CHAT_INSTRUCTIONS.to_string());
        let context = into_provider_context(input.context);
        let prompt = input.prompt;
        let files = input.files;
        let run_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(run_id = %run_id, model = %model, context_messages = context.len(), file_count = files.len(), prompt_chars = prompt.chars().count(), "assistant ephemeral run accepted");
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        event_tx
            .try_send(SendQueryEvent::Started {
                model: model.clone(),
                thread_id: None,
                user_message_id: None,
                assistant_message_id: None,
            })
            .map_err(|_| ServiceError::internal("failed to initialize query event stream"))?;

        tokio::spawn(async move {
            let run_started_at = Instant::now();
            tracing::debug!(run_id = %run_id, "assistant ephemeral file encoding started");
            let encoded_files = match tokio::task::spawn_blocking(move || {
                files
                    .into_iter()
                    .map(|file| {
                        (
                            file.filename,
                            file.mime_type,
                            general_purpose::STANDARD.encode(file.bytes),
                            file.kind,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .await
            {
                Ok(files) => files,
                Err(error) => {
                    tracing::error!(run_id = %run_id, category = "encoding_worker", error = %error, "assistant ephemeral file encoding failed");
                    let _ = event_tx
                        .send(SendQueryEvent::Failed {
                            error: "query generation failed",
                        })
                        .await;
                    return;
                }
            };
            let inputs = encoded_files
                .iter()
                .map(|(filename, mime_type, data_base64, kind)| match kind {
                    ChatUploadKind::Image => ResponsesFileInput::ImageData {
                        mime_type,
                        data_base64,
                        detail: Some("auto"),
                    },
                    ChatUploadKind::File => ResponsesFileInput::FileData {
                        filename,
                        mime_type,
                        data_base64,
                    },
                })
                .collect::<Vec<_>>();
            let (delta_tx, mut delta_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
            let response = client.gen_chat_response_streaming(
                &context,
                &prompt,
                &instructions,
                &model,
                &inputs,
                delta_tx,
            );
            tokio::pin!(response);
            tracing::info!(run_id = %run_id, "assistant provider stream started");

            loop {
                tokio::select! {
                    biased;
                    _ = event_tx.closed() => {
                        tracing::info!(run_id = %run_id, elapsed_ms = run_started_at.elapsed().as_millis(), "assistant client disconnected");
                        break
                    },
                    result = &mut response => {
                        while let Ok(delta) = delta_rx.try_recv() {
                            if event_tx.send(SendQueryEvent::Delta { delta }).await.is_err() {
                                return;
                            }
                        }
                        let terminal = match result {
                            Ok(response) => {
                                tracing::info!(run_id = %run_id, response_chars = response.chars().count(), elapsed_ms = run_started_at.elapsed().as_millis(), "assistant ephemeral run completed");
                                SendQueryEvent::Completed { response }
                            },
                            Err(error) => {
                                tracing::error!(run_id = %run_id, category = %error.category(), elapsed_ms = run_started_at.elapsed().as_millis(), "assistant ephemeral provider failed");
                                SendQueryEvent::Failed { error: "query generation failed" }
                            }
                        };
                        let _ = event_tx.send(terminal).await;
                        break;
                    }
                    delta = delta_rx.recv() => {
                        let Some(delta) = delta else { continue };
                        if event_tx.send(SendQueryEvent::Delta { delta }).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(event_rx)
    }

    pub async fn create_thread(
        &self,
        email: &str,
        thread_id: String,
    ) -> ServiceResult<AssistantThread> {
        validate_identifier("threadId", &thread_id)?;
        let user_id = self.user_id(email).await?;
        self.repository()?
            .create_thread(user_id, thread_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list_threads(
        &self,
        email: &str,
        archived: bool,
        limit: usize,
        before: Option<String>,
    ) -> ServiceResult<Vec<AssistantThread>> {
        let user_id = self.user_id(email).await?;
        self.repository()?
            .list_threads(
                user_id,
                if archived { "archived" } else { "regular" }.to_string(),
                limit,
                before,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn get_thread(
        &self,
        email: &str,
        thread_id: String,
    ) -> ServiceResult<AssistantThreadDetail> {
        validate_identifier("threadId", &thread_id)?;
        let user_id = self.user_id(email).await?;
        self.repository()?
            .get_thread(user_id, thread_id)
            .await?
            .ok_or_else(|| ServiceError::not_found("assistant thread was not found"))
    }

    pub async fn rename_thread(
        &self,
        email: &str,
        thread_id: String,
        title: String,
    ) -> ServiceResult<()> {
        validate_identifier("threadId", &thread_id)?;
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 160 {
            return Err(ServiceError::validation("title is invalid"));
        }
        let user_id = self.user_id(email).await?;
        if !self
            .repository()?
            .rename_thread(user_id, thread_id, title.to_string())
            .await?
        {
            return Err(ServiceError::not_found("assistant thread was not found"));
        }
        Ok(())
    }

    pub async fn set_thread_archived(
        &self,
        email: &str,
        thread_id: String,
        archived: bool,
    ) -> ServiceResult<()> {
        validate_identifier("threadId", &thread_id)?;
        let user_id = self.user_id(email).await?;
        let status = if archived { "archived" } else { "regular" }.to_string();
        if !self
            .repository()?
            .set_thread_status(user_id, thread_id, status)
            .await?
        {
            return Err(ServiceError::not_found("assistant thread was not found"));
        }
        Ok(())
    }

    pub async fn delete_thread(&self, email: &str, thread_id: String) -> ServiceResult<()> {
        validate_identifier("threadId", &thread_id)?;
        let user_id = self.user_id(email).await?;
        if !self.repository()?.delete_thread(user_id, thread_id).await? {
            return Err(ServiceError::not_found("assistant thread was not found"));
        }
        Ok(())
    }

    pub async fn ask_persisted(
        &self,
        input: PersistentQueryInput,
    ) -> ServiceResult<mpsc::Receiver<SendQueryEvent>> {
        let client = self
            .openai
            .as_ref()
            .cloned()
            .ok_or_else(|| ServiceError::unavailable("OpenAI capability is not configured"))?;
        let repository = self.repository()?.clone();
        let user_id = self.user_id(&input.user_email).await?;
        let model = input.model.unwrap_or_else(|| self.model.clone());
        let instructions = input
            .system_instructions
            .unwrap_or_else(|| DEFAULT_CHAT_INSTRUCTIONS.to_string());
        let client_request_id = input.request_id.clone();
        tracing::info!(client_request_id = %client_request_id, thread_id = %input.thread_id, assistant_message_id = %input.assistant_message_id, user_message_id = %input.user_message_id, model = %model, file_count = input.files.len(), prompt_chars = input.prompt.chars().count(), "assistant persisted run starting");
        let start = repository
            .start_run(StartRunInput {
                assistant_message_id: input.assistant_message_id.clone(),
                model: model.clone(),
                parent_message_id: input.parent_message_id,
                prompt: input.prompt.clone(),
                request_id: input.request_id,
                thread_id: input.thread_id.clone(),
                user_id,
                user_message_id: input.user_message_id.clone(),
            })
            .await
            .map_err(|error| match error {
                RepositoryError::InvalidData(_) => {
                    ServiceError::not_found("assistant thread was not found")
                }
                other => other.into(),
            })?;
        if matches!(&start, StartRunResult::Conflict) {
            tracing::warn!(client_request_id = %client_request_id, thread_id = %input.thread_id, "assistant persisted run rejected due to message id conflict");
            return Err(ServiceError::Conflict(
                "assistant run message ids conflict with existing messages".to_string(),
            ));
        }
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        event_tx
            .try_send(SendQueryEvent::Started {
                model: model.clone(),
                thread_id: Some(input.thread_id.clone()),
                user_message_id: Some(input.user_message_id.clone()),
                assistant_message_id: Some(input.assistant_message_id.clone()),
            })
            .map_err(|_| ServiceError::internal("failed to initialize query event stream"))?;
        if let StartRunResult::Existing(message) = start {
            tracing::info!(client_request_id = %client_request_id, thread_id = %input.thread_id, status = %message.status, response_chars = message.content.chars().count(), "assistant persisted run replayed");
            let event = match message.status.as_str() {
                "completed" => SendQueryEvent::Completed {
                    response: message.content,
                },
                "failed" | "cancelled" => SendQueryEvent::Failed {
                    error: "query generation failed",
                },
                _ => {
                    return Err(ServiceError::Conflict(
                        "assistant run is already in progress".to_string(),
                    ))
                }
            };
            event_tx
                .try_send(event)
                .map_err(|_| ServiceError::internal("failed to replay query result"))?;
            return Ok(event_rx);
        }
        let StartRunResult::Started { prior_messages } = start else {
            unreachable!()
        };
        let context = bounded_provider_context(prior_messages);
        let prompt = input.prompt;
        let files = input.files;
        let assistant_message_id = input.assistant_message_id;
        let thread_id = input.thread_id;
        tokio::spawn(async move {
            let run_started_at = Instant::now();
            tracing::info!(client_request_id = %client_request_id, thread_id = %thread_id, assistant_message_id = %assistant_message_id, "assistant persisted provider stream started");
            let encoded_files = match tokio::task::spawn_blocking(move || {
                files
                    .into_iter()
                    .map(|file| {
                        (
                            file.filename,
                            file.mime_type,
                            general_purpose::STANDARD.encode(file.bytes),
                            file.kind,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .await
            {
                Ok(files) => files,
                Err(error) => {
                    tracing::error!(client_request_id = %client_request_id, error = %error, "assistant persisted file encoding failed");
                    let _ = repository
                        .finish_run(
                            assistant_message_id,
                            "failed",
                            String::new(),
                            Some("encoding_failed"),
                        )
                        .await;
                    let _ = event_tx
                        .send(SendQueryEvent::Failed {
                            error: "query generation failed",
                        })
                        .await;
                    return;
                }
            };
            let inputs = encoded_files
                .iter()
                .map(|(filename, mime_type, data_base64, kind)| match kind {
                    ChatUploadKind::Image => ResponsesFileInput::ImageData {
                        mime_type,
                        data_base64,
                        detail: Some("auto"),
                    },
                    ChatUploadKind::File => ResponsesFileInput::FileData {
                        filename,
                        mime_type,
                        data_base64,
                    },
                })
                .collect::<Vec<_>>();
            let (delta_tx, mut delta_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
            let response = client.gen_chat_response_streaming(
                &context,
                &prompt,
                &instructions,
                &model,
                &inputs,
                delta_tx,
            );
            tokio::pin!(response);
            let mut buffer = String::new();
            loop {
                tokio::select! {
                    biased;
                    _ = event_tx.closed() => {
                        tracing::info!(client_request_id = %client_request_id, elapsed_ms = run_started_at.elapsed().as_millis(), "assistant persisted client disconnected");
                        let _ = repository.finish_run(assistant_message_id, "cancelled", buffer, None).await;
                        break;
                    }
                    result = &mut response => {
                        while let Ok(delta) = delta_rx.try_recv() {
                            buffer.push_str(&delta);
                            if event_tx.send(SendQueryEvent::Delta { delta }).await.is_err() {
                                let _ = repository.finish_run(assistant_message_id, "cancelled", buffer, None).await;
                                return;
                            }
                        }
                        match result {
                            Ok(response) => {
                                tracing::info!(client_request_id = %client_request_id, response_chars = response.chars().count(), elapsed_ms = run_started_at.elapsed().as_millis(), "assistant persisted provider completed");
                                if repository.finish_run(assistant_message_id, "completed", response.clone(), None).await.is_err() {
                                    tracing::error!(client_request_id = %client_request_id, "assistant persisted completion failed to save");
                                    let _ = event_tx.send(SendQueryEvent::Failed { error: "query persistence failed" }).await;
                                } else {
                                    let _ = event_tx.send(SendQueryEvent::Completed { response }).await;
                                }
                            }
                            Err(error) => {
                                tracing::error!(client_request_id = %client_request_id, category = %error.category(), elapsed_ms = run_started_at.elapsed().as_millis(), "assistant persisted provider failed");
                                let _ = repository.finish_run(assistant_message_id, "failed", buffer, Some("provider_failed")).await;
                                let _ = event_tx.send(SendQueryEvent::Failed { error: "query generation failed" }).await;
                            }
                        }
                        break;
                    }
                    delta = delta_rx.recv() => {
                        let Some(delta) = delta else { continue };
                        buffer.push_str(&delta);
                        if event_tx.send(SendQueryEvent::Delta { delta }).await.is_err() {
                            let _ = repository.finish_run(assistant_message_id, "cancelled", buffer, None).await;
                            break;
                        }
                    }
                }
            }
        });
        Ok(event_rx)
    }

    fn repository(&self) -> ServiceResult<&AssistantChatRepository> {
        self.repository
            .as_ref()
            .ok_or_else(|| ServiceError::internal("assistant persistence is not configured"))
    }

    async fn user_id(&self, email: &str) -> ServiceResult<i64> {
        let email = email.trim();
        if email.is_empty() || email.chars().count() > 320 {
            return Err(ServiceError::validation("userEmail is invalid"));
        }
        self.users
            .as_ref()
            .ok_or_else(|| ServiceError::internal("assistant identity lookup is not configured"))?
            .by_email(email)
            .await?
            .map(|user| user.id)
            .ok_or_else(|| ServiceError::not_found("workspace user was not found"))
    }
}

fn validate_identifier(name: &str, value: &str) -> ServiceResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(ServiceError::validation(format!("{name} is invalid")));
    }
    Ok(())
}

fn bounded_provider_context(
    messages: Vec<super::repository::AssistantMessage>,
) -> Vec<ChatMessageInput> {
    let mut pairs = Vec::new();
    for pair in messages.windows(2) {
        if pair[0].role == "user"
            && pair[1].role == "assistant"
            && pair[1].sequence == pair[0].sequence + 1
            && pair[1].parent_message_id.as_deref() == Some(pair[0].message_id.as_str())
        {
            pairs.push((pair[0].content.clone(), pair[1].content.clone()));
        }
    }
    let mut selected = Vec::new();
    let mut chars = 0usize;
    for (user, assistant) in pairs.into_iter().rev() {
        let pair_chars = user.chars().count() + assistant.chars().count();
        if chars + pair_chars > MAX_CONTEXT_CHARS {
            break;
        }
        chars += pair_chars;
        selected.push((user, assistant));
    }
    selected.reverse();
    selected
        .into_iter()
        .flat_map(|(user, assistant)| {
            [
                ChatMessageInput {
                    role: ChatRole::User,
                    content: user,
                },
                ChatMessageInput {
                    role: ChatRole::Assistant,
                    content: assistant,
                },
            ]
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/assistant/chat/service_tests.rs"]
mod tests;
