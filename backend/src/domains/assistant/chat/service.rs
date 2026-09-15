use std::sync::Arc;

use base64::{engine::general_purpose, Engine as _};
use tokio::sync::mpsc;

use crate::{
    adapters::openai::client::{OpenAiClient, ResponsesFileInput},
    shared::error::{ServiceError, ServiceResult},
};

use super::{
    model::{into_provider_context, QueryModelInput, SendQueryEvent},
    upload::ChatUploadKind,
};

const DEFAULT_CHAT_INSTRUCTIONS: &str = "You are a helpful assistant.";
const EVENT_CHANNEL_CAPACITY: usize = 32;

pub struct AssistantChatService {
    openai: Option<Arc<OpenAiClient>>,
    model: String,
}

impl AssistantChatService {
    pub fn new(openai: Option<Arc<OpenAiClient>>, model: String) -> Self {
        Self { openai, model }
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
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        event_tx
            .try_send(SendQueryEvent::Started {
                model: model.clone(),
            })
            .map_err(|_| ServiceError::internal("failed to initialize query event stream"))?;

        tokio::spawn(async move {
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
                Err(_) => {
                    tracing::error!(category = "encoding_worker", "assistant query failed");
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

            loop {
                tokio::select! {
                    biased;
                    _ = event_tx.closed() => break,
                    result = &mut response => {
                        while let Ok(delta) = delta_rx.try_recv() {
                            if event_tx.send(SendQueryEvent::Delta { delta }).await.is_err() {
                                return;
                            }
                        }
                        let terminal = match result {
                            Ok(response) => SendQueryEvent::Completed { response },
                            Err(error) => {
                                tracing::error!(category = %error.category(), "assistant query failed");
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
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/assistant/chat/service_tests.rs"]
mod tests;
