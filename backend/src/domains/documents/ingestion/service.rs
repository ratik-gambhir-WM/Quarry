use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Weak},
    time::Instant,
};

use base64::{engine::general_purpose, Engine as _};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::Serialize;
use tokio::sync::{Mutex, OwnedMutexGuard, Semaphore};

pub use crate::domains::documents::model::{Document, DocumentChunk};

use crate::{
    adapters::openai::client::{OpenAiClient, ResponsesFileInput},
    domains::documents::{
        formats::{
            image::{build_image_assembly, validate_image_by_bytes},
            ParsedQuarryFile, QuarryFile,
        },
        graph::repository::{DocumentGraphReader, DocumentGraphWriter},
        ingestion::persistence::persist_document_and_chunks,
        store::{model::PersistedFileIdentity, sqlite::DocumentStore},
    },
    shared::{
        error::{ServiceError, ServiceResult},
        file_policy::infer_supported_image_mime_type,
        ids::{document_id_from_content, sha256_hex},
        prompts::PromptId,
    },
};

const DOCUMENT_PARSE_API: &str = "document.parse";
const MAX_CONCURRENT_OFFICE_PARSERS: usize = 2;
const MAX_CONCURRENT_EMBEDDING_REQUESTS: usize = 2;
const MAX_EMBEDDING_INPUTS_PER_REQUEST: usize = 2_048;
const MAX_EMBEDDING_TOKENS_PER_REQUEST: u64 = 300_000;

#[derive(Debug)]
pub struct UploadedDocument {
    pub filename: String,
    pub bytes: Vec<u8>,
}

struct ParsedDocumentGraph {
    document: Document,
    chunks: Vec<DocumentChunk>,
    file_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EmbeddingBatch {
    start: usize,
    end: usize,
    token_count: u64,
}

impl EmbeddingBatch {
    fn input_count(self) -> usize {
        self.end - self.start
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedDocument {
    pub filename: String,
    pub document_id: Option<String>,
    pub chunk_count: usize,
    pub success: bool,
    pub skipped: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessDocumentsResponse {
    pub total: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub documents: Vec<ProcessedDocument>,
}

#[derive(Clone)]
pub struct DocumentIngestionService {
    files: DocumentStore,
    graph_reader: DocumentGraphReader,
    graph_writer: DocumentGraphWriter,
    openai: Option<Arc<OpenAiClient>>,
    embedding_model: String,
    image_description_model: String,
    processing_locks: Arc<Mutex<HashMap<String, Weak<Mutex<()>>>>>,
    office_parse_semaphore: Arc<Semaphore>,
    embedding_request_semaphore: Arc<Semaphore>,
    max_concurrent_documents: usize,
}

impl DocumentIngestionService {
    pub fn new(
        files: DocumentStore,
        graph_reader: DocumentGraphReader,
        graph_writer: DocumentGraphWriter,
        openai: Option<Arc<OpenAiClient>>,
        embedding_model: String,
        image_description_model: String,
        max_concurrent_documents: usize,
    ) -> Self {
        Self {
            files,
            graph_reader,
            graph_writer,
            openai,
            embedding_model,
            image_description_model,
            processing_locks: Arc::new(Mutex::new(HashMap::new())),
            office_parse_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_OFFICE_PARSERS)),
            embedding_request_semaphore: Arc::new(Semaphore::new(
                MAX_CONCURRENT_EMBEDDING_REQUESTS,
            )),
            max_concurrent_documents,
        }
    }

    pub async fn process(
        &self,
        deal_id: &str,
        user_id: &str,
        files: Vec<UploadedDocument>,
    ) -> ServiceResult<ProcessDocumentsResponse> {
        if self.openai.is_none() {
            return Err(ServiceError::unavailable(
                "OpenAI capability is not configured",
            ));
        }
        let total = files.len();
        let worker_service = self.clone();
        let worker_deal_id = deal_id.to_string();
        let worker_user_id = user_id.to_string();
        let mut indexed_documents = stream::iter(files.into_iter().enumerate())
            .map(|(index, file)| {
                let service = worker_service.clone();
                let deal_id = worker_deal_id.clone();
                let user_id = worker_user_id.clone();
                let filename = file.filename.clone();
                async move {
                    let worker = tokio::spawn(async move {
                        service
                            .process_uploaded_document(file, deal_id, user_id)
                            .await
                    });
                    let document = match worker.await {
                        Ok(document) => document,
                        Err(error) => Ok(failed_document(
                            filename,
                            format!("document processing task failed: {error}"),
                        )),
                    };
                    (index, document)
                }
            })
            .buffer_unordered(self.max_concurrent_documents)
            .collect::<Vec<_>>()
            .await;
        indexed_documents.sort_unstable_by_key(|(index, _)| *index);
        let documents = indexed_documents
            .into_iter()
            .map(|(_, document)| document)
            .collect::<ServiceResult<Vec<_>>>()?;
        let succeeded = documents.iter().filter(|document| document.success).count();
        let skipped = documents.iter().filter(|document| document.skipped).count();

        Ok(ProcessDocumentsResponse {
            total,
            succeeded,
            skipped,
            failed: total - succeeded,
            documents,
        })
    }

    async fn process_uploaded_document(
        &self,
        file: UploadedDocument,
        deal_id: String,
        user_id: String,
    ) -> ServiceResult<ProcessedDocument> {
        let filename = file.filename.clone();
        let content_hash = uploaded_document_content_hash(&file);
        let document_id = document_id_from_content(&user_id, &content_hash);
        let attachment_lock_id = format!("{deal_id}\0{document_id}");
        let _processing_guard = self.lock_processing(&attachment_lock_id).await;
        let existing_attachment = match self
            .files
            .find_current_by_hash(&deal_id, &user_id, &content_hash)
            .await
        {
            Ok(attachment) => attachment,
            Err(error) => {
                return Ok(failed_document(
                    filename,
                    format!("failed to check for an existing deal attachment: {error}"),
                ));
            }
        };
        if let Some(existing) = &existing_attachment {
            match self
                .graph_reader
                .current_document(&user_id, &existing.file_id)
                .await
            {
                Ok(Some(indexed))
                    if indexed.version.version_id == existing.version_id
                        && indexed.version.content_sha256 == content_hash =>
                {
                    return Ok(ProcessedDocument {
                        filename,
                        document_id: Some(document_id),
                        chunk_count: 0,
                        success: true,
                        skipped: true,
                        error: None,
                    });
                }
                Ok(Some(_)) | Ok(None) => {}
                Err(error) => {
                    return Ok(failed_document(
                        filename,
                        format!("failed to check the existing deal attachment index: {error}"),
                    ));
                }
            }
        }

        let openai = self
            .openai
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("OpenAI capability is not configured"))?;
        Ok(
            match self
                .process_document(file, deal_id, user_id, existing_attachment.as_ref(), openai)
                .await
            {
                Ok((document_id, chunk_count)) => ProcessedDocument {
                    filename,
                    document_id: Some(document_id),
                    chunk_count,
                    success: true,
                    skipped: false,
                    error: None,
                },
                Err(error) => failed_document(filename, error),
            },
        )
    }

    async fn process_document(
        &self,
        file: UploadedDocument,
        deal_id: String,
        user_id: String,
        existing_attachment: Option<&PersistedFileIdentity>,
        openai: &OpenAiClient,
    ) -> Result<(String, usize), String> {
        let filename = file.filename.clone();
        let mut graph = parse_document(
            file,
            user_id,
            openai,
            &self.image_description_model,
            Arc::clone(&self.office_parse_semaphore),
        )
        .await?;
        if let Some(existing) = existing_attachment {
            graph.document.file_id.clone_from(&existing.file_id);
        }
        let document_id = graph.document.document_id.clone();
        let chunk_count = graph.chunks.len();
        self.embed_chunks(
            &filename,
            graph.document.file_size_bytes,
            &mut graph.chunks,
            openai,
        )
        .await?;
        persist_document_and_chunks(
            &self.files,
            &self.graph_writer,
            &deal_id,
            graph.document,
            graph.chunks,
            graph.file_bytes,
        )
        .await
        .map_err(|error| format!("failed to persist `{filename}`: {error}"))?;
        Ok((document_id, chunk_count))
    }

    async fn embed_chunks(
        &self,
        filename: &str,
        file_size_bytes: u64,
        chunks: &mut [DocumentChunk],
        openai: &OpenAiClient,
    ) -> Result<(), String> {
        if chunks.is_empty() {
            return Ok(());
        }
        let batches = embedding_batches(chunks)?;
        let total_chunks = chunks.len();
        let total_batches = batches.len();
        let batch_results = stream::iter(batches)
            .map(|batch| {
                let contents = chunks[batch.start..batch.end]
                    .iter()
                    .map(|chunk| chunk.text.as_str())
                    .collect::<Vec<_>>();
                let embedding_request_semaphore = Arc::clone(&self.embedding_request_semaphore);
                async move {
                    let _permit = embedding_request_semaphore.acquire().await.map_err(|_| {
                        "embedding request concurrency control is shutting down".to_string()
                    })?;
                    tracing::info!(
                        api = "document.embed",
                        filename,
                        batch_start = batch.start + 1,
                        batch_end = batch.end,
                        batch_input_count = batch.input_count(),
                        batch_token_count = batch.token_count,
                        total_batches,
                        "embedding document chunk batch",
                    );
                    let embeddings = openai
                        .gen_embeddings_for_file(
                            &contents,
                            Some(&self.embedding_model),
                            filename,
                            file_size_bytes,
                        )
                        .await
                        .map_err(|error| {
                            format!(
                                "failed to embed chunks {}-{} of {total_chunks} for `{filename}`: {error}",
                                batch.start + 1,
                                batch.end,
                            )
                        })?;
                    if batch.input_count() != embeddings.len() {
                        return Err(format!(
                            "OpenAI returned {} embeddings for chunks {}-{} of {total_chunks} in `{filename}`",
                            embeddings.len(),
                            batch.start + 1,
                            batch.end,
                        ));
                    }
                    Ok((batch, embeddings))
                }
            })
            .buffer_unordered(MAX_CONCURRENT_EMBEDDING_REQUESTS)
            .try_collect::<Vec<_>>()
            .await?;

        for (batch, embeddings) in batch_results {
            for (chunk, embedding) in chunks[batch.start..batch.end].iter_mut().zip(embeddings) {
                chunk.embedding = Some(embedding.into_iter().map(|value| value as f32).collect());
            }
        }
        Ok(())
    }

    async fn lock_processing(&self, document_id: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.processing_locks.lock().await;
            locks.retain(|_, lock| lock.strong_count() > 0);
            match locks.get(document_id).and_then(Weak::upgrade) {
                Some(lock) => lock,
                None => {
                    let lock = Arc::new(Mutex::new(()));
                    locks.insert(document_id.to_string(), Arc::downgrade(&lock));
                    lock
                }
            }
        };
        lock.lock_owned().await
    }
}

fn uploaded_document_content_hash(file: &UploadedDocument) -> String {
    sha256_hex(&file.bytes)
}

fn embedding_batches(chunks: &[DocumentChunk]) -> Result<Vec<EmbeddingBatch>, String> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let mut batches = Vec::new();
    let mut batch_start = 0;
    let mut batch_token_count = 0_u64;

    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_token_count = u64::from(chunk.token_count);
        if chunk_token_count > MAX_EMBEDDING_TOKENS_PER_REQUEST {
            return Err(format!(
                "chunk {} has {chunk_token_count} tokens, exceeding the {MAX_EMBEDDING_TOKENS_PER_REQUEST}-token OpenAI embedding request limit",
                index + 1,
            ));
        }
        let combined_token_count = batch_token_count
            .checked_add(chunk_token_count)
            .ok_or_else(|| "embedding request token count overflowed".to_string())?;
        let batch_input_count = index - batch_start;
        if batch_input_count == MAX_EMBEDDING_INPUTS_PER_REQUEST
            || combined_token_count > MAX_EMBEDDING_TOKENS_PER_REQUEST
        {
            batches.push(EmbeddingBatch {
                start: batch_start,
                end: index,
                token_count: batch_token_count,
            });
            batch_start = index;
            batch_token_count = 0;
        }
        batch_token_count = batch_token_count
            .checked_add(chunk_token_count)
            .ok_or_else(|| "embedding request token count overflowed".to_string())?;
    }

    batches.push(EmbeddingBatch {
        start: batch_start,
        end: chunks.len(),
        token_count: batch_token_count,
    });
    Ok(batches)
}

fn failed_document(filename: String, error: String) -> ProcessedDocument {
    ProcessedDocument {
        filename,
        document_id: None,
        chunk_count: 0,
        success: false,
        skipped: false,
        error: Some(error),
    }
}

async fn parse_document(
    file: UploadedDocument,
    user_id: String,
    openai: &OpenAiClient,
    image_description_model: &str,
    office_parse_semaphore: Arc<Semaphore>,
) -> Result<ParsedDocumentGraph, String> {
    let filename = file.filename.clone();
    let file_bytes = file.bytes.clone();
    let file_size_bytes = u64::try_from(file.bytes.len())
        .map_err(|_| format!("file size for `{filename}` does not fit in u64"))?;
    let started_at = Instant::now();
    let result = async {
        let (document, chunks) = if infer_supported_image_mime_type(Path::new(&file.filename))
            .is_some()
        {
            let validated = validate_image_by_bytes(file.bytes, &file.filename).await?;
            let description = describe_image(
                &validated.bytes,
                validated.mime_type,
                openai,
                image_description_model,
            )
            .await?;
            let assembly =
                build_image_assembly(validated.bytes, None, file.filename, &user_id, &description)?;
            (assembly.document, assembly.chunks)
        } else {
            let requires_office_permit = matches!(
                Path::new(&file.filename)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("xlsx" | "pptx")
            );
            let office_permit = if requires_office_permit {
                Some(
                    office_parse_semaphore
                        .acquire_owned()
                        .await
                        .map_err(|_| "Office document parsing is shutting down".to_string())?,
                )
            } else {
                None
            };
            let parser_filename = file.filename;
            let parser_user_id = user_id.clone();
            let parsed = tokio::task::spawn_blocking(move || {
                let _office_permit = office_permit;
                QuarryFile::from_bytes(parser_filename, file.bytes)?.parse(&parser_user_id)
            })
            .await
            .map_err(|_| "document parsing worker failed".to_string())??;
            match parsed {
                ParsedQuarryFile::Pdf(assembly) => (assembly.document, assembly.chunks),
                ParsedQuarryFile::Docx(assembly) => (assembly.document, assembly.chunks),
                ParsedQuarryFile::Powerpoint(assembly) => (assembly.document, assembly.chunks),
                ParsedQuarryFile::Spreadsheet(assembly) => (assembly.document, assembly.chunks),
            }
        };
        Ok(ParsedDocumentGraph {
            document,
            chunks,
            file_bytes,
        })
    }
    .await;
    match &result {
        Ok(_) => tracing::info!(
            api = DOCUMENT_PARSE_API,
            filename,
            file_size_bytes,
            elapsed_seconds = started_at.elapsed().as_secs_f64(),
        ),
        Err(error) => tracing::error!(
            api = DOCUMENT_PARSE_API,
            filename,
            file_size_bytes,
            reason = %error,
            elapsed_seconds = started_at.elapsed().as_secs_f64(),
        ),
    }
    result
}

async fn describe_image(
    image: &[u8],
    mime_type: &str,
    openai_client: &OpenAiClient,
    description_model: &str,
) -> Result<String, String> {
    if image.is_empty() {
        return Err("cannot describe an empty image".to_string());
    }
    let description_model = description_model.trim();
    if description_model.is_empty() {
        return Err("image description model cannot be empty".to_string());
    }

    let image_base64 = general_purpose::STANDARD.encode(image);
    let file_inputs = [ResponsesFileInput::ImageData {
        mime_type,
        data_base64: image_base64.as_str(),
        detail: Some("auto"),
    }];
    let description = openai_client
        .gen_model_response_with_files(
            Some(openai_client.prompts().prompt(PromptId::ImageDescription)),
            None,
            Some(description_model),
            Some(&file_inputs),
        )
        .await?;
    let description = description.trim().to_string();
    if description.is_empty() {
        return Err("OpenAI image analysis returned an empty description".to_string());
    }
    Ok(description)
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/ingestion/service_tests.rs"]
mod tests;
