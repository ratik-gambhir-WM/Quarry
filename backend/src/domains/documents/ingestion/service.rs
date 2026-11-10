use super::error::IngestionError;

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
            image::{build_image_assembly, validate_image_by_bytes, ImageAssembly},
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
    pub sqlite_committed: bool,
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

    pub async fn authorize_upload(&self, deal: &str, actor: i64) -> ServiceResult<()> {
        self.files
            .authorize_upload(deal, actor)
            .await
            .map_err(Into::into)
    }

    pub async fn authorize_read(&self, deal: &str, actor: i64) -> ServiceResult<()> {
        self.files
            .authorize_read(deal, actor)
            .await
            .map_err(Into::into)
    }
    pub async fn process(
        &self,
        deal_id: &str,
        actor_id: i64,
        files: Vec<UploadedDocument>,
    ) -> ServiceResult<ProcessDocumentsResponse> {
        self.files.authorize_upload(deal_id, actor_id).await?;
        if self.openai.is_none() {
            return Err(ServiceError::unavailable(
                "OpenAI capability is not configured",
            ));
        }
        let total = files.len();
        let worker_service = self.clone();
        let worker_deal_id = deal_id.to_string();
        let worker_actor_id = actor_id;
        let mut indexed_documents = stream::iter(files.into_iter().enumerate())
            .map(|(index, file)| {
                let service = worker_service.clone();
                let deal_id = worker_deal_id.clone();
                let actor_id = worker_actor_id;
                let filename = file.filename.clone();
                async move {
                    let worker = tokio::spawn(async move {
                        service
                            .process_uploaded_document(file, deal_id, actor_id)
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
        actor_id: i64,
    ) -> ServiceResult<ProcessedDocument> {
        let filename = file.filename.clone();
        let content_hash = uploaded_document_content_hash(&file);
        let document_id = document_id_from_content(&deal_id, &content_hash);
        let attachment_lock_id = format!("{deal_id}\0{document_id}");
        let _processing_guard = self.lock_processing(&attachment_lock_id).await;
        let existing_attachment = match self
            .files
            .find_current_by_hash(&deal_id, &content_hash)
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
            if let Some(outcome) = self
                .existing_attachment_outcome(&deal_id, existing, &content_hash, &filename)
                .await?
            {
                return Ok(outcome);
            }
        }

        let openai = self
            .openai
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("OpenAI capability is not configured"))?;
        Ok(
            match self
                .process_document(
                    file,
                    deal_id,
                    actor_id,
                    existing_attachment.as_ref(),
                    openai,
                )
                .await
            {
                Ok((document_id, chunk_count)) => ProcessedDocument {
                    filename,
                    document_id: Some(document_id),
                    chunk_count,
                    success: true,
                    sqlite_committed: true,
                    skipped: false,
                    error: None,
                },
                Err(error) => failed_document(filename, error),
            },
        )
    }

    /// Only a complete projection of the canonical immutable version can skip processing.
    /// Missing or stale graph state falls through to repair; an unavailable graph fails closed.
    async fn existing_attachment_outcome(
        &self,
        deal_id: &str,
        existing: &PersistedFileIdentity,
        content_hash: &str,
        filename: &str,
    ) -> ServiceResult<Option<ProcessedDocument>> {
        let indexed = match self
            .graph_reader
            .current_document(deal_id, &existing.file_id)
            .await
        {
            Ok(Some(indexed))
                if indexed.version.version_id == existing.version_id
                    && indexed.version.content_sha256 == content_hash =>
            {
                indexed
            }
            Ok(_) => return Ok(None),
            Err(error) => {
                return Ok(Some(failed_document(
                    filename.to_owned(),
                    format!("failed to check the existing deal attachment index: {error}"),
                )))
            }
        };
        let manifest = self
            .files
            .manifest(existing.version_id.clone())
            .await?
            .ok_or_else(|| ServiceError::internal("identity manifest missing"))?;
        let active = manifest
            .chunks
            .iter()
            .filter(|chunk| chunk.index_generation == manifest.active_generation)
            .cloned()
            .collect::<Vec<_>>();
        let complete = match self
            .graph_reader
            .has_complete_chunks(deal_id, &active)
            .await
        {
            Ok(complete) => complete,
            Err(error) => {
                return Ok(Some(failed_document(
                    filename.to_owned(),
                    format!("failed to verify the existing deal attachment index: {error}"),
                )))
            }
        };
        if !complete || indexed.version.index_generation != manifest.active_generation {
            return Ok(None);
        }
        Ok(Some(ProcessedDocument {
            filename: filename.to_owned(),
            document_id: Some(manifest.document_id),
            chunk_count: 0,
            success: true,
            sqlite_committed: true,
            skipped: true,
            error: None,
        }))
    }

    async fn process_document(
        &self,
        file: UploadedDocument,
        deal_id: String,
        actor_id: i64,
        existing_attachment: Option<&PersistedFileIdentity>,
        openai: &OpenAiClient,
    ) -> Result<(String, usize), IngestionError> {
        let filename = file.filename.clone();
        let recovery = match existing_attachment {
            Some(existing)
                if infer_supported_image_mime_type(Path::new(&existing.display_name)).is_some() =>
            {
                Some(
                    self.files
                        .image_assembly(&existing.version_id)
                        .await
                        .map_err(|e| e.to_string())?
                        .ok_or(
                            "accepted image text is unavailable; maintenance recovery required",
                        )?,
                )
            }
            _ => None,
        };
        let mut graph = parse_document(
            file,
            deal_id.clone(),
            openai,
            &self.image_description_model,
            Arc::clone(&self.office_parse_semaphore),
            recovery,
        )
        .await?;
        if let Some(existing) = existing_attachment {
            graph.document.file_id.clone_from(&existing.file_id);
            graph.document.file_name.clone_from(&existing.display_name);
        }
        let manifest_key = crate::shared::ids::file_version_id(
            &graph.document.file_id,
            &graph.document.content_hash,
        );
        let document_id = self
            .files
            .manifest(manifest_key)
            .await
            .map_err(|e| e.to_string())?
            .map(|m| m.document_id)
            .unwrap_or_else(|| graph.document.document_id.clone());
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
            actor_id,
            graph.document,
            graph.chunks,
            graph.file_bytes,
        )
        .await?;
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

fn failed_document(filename: String, error: impl Into<IngestionError>) -> ProcessedDocument {
    let sqlite_committed = error.into().sqlite_committed();
    let message = if sqlite_committed {
        "Document was stored, but indexing failed. Refresh documents and retry the same file to recover indexing."
    } else {
        "Document processing failed. Check the file and your deal access, then retry."
    };
    ProcessedDocument {
        filename,
        document_id: None,
        chunk_count: 0,
        success: false,
        skipped: false,
        sqlite_committed,
        error: Some(message.into()),
    }
}

async fn parse_document(
    file: UploadedDocument,
    deal_id: String,
    openai: &OpenAiClient,
    image_description_model: &str,
    office_parse_semaphore: Arc<Semaphore>,
    recovery: Option<ImageAssembly>,
) -> Result<ParsedDocumentGraph, String> {
    if let Some(assembly) = recovery {
        if assembly.document.deal_id != deal_id
            || assembly.document.content_hash != uploaded_document_content_hash(&file)
        {
            return Err("stored image recovery does not match upload".into());
        }
        return Ok(ParsedDocumentGraph {
            document: assembly.document,
            chunks: assembly.chunks,
            file_bytes: file.bytes,
        });
    }
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
                build_image_assembly(validated.bytes, None, file.filename, &deal_id, &description)?;
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
            let parser_deal_id = deal_id.clone();
            let parsed = tokio::task::spawn_blocking(move || {
                let _office_permit = office_permit;
                QuarryFile::from_bytes(parser_filename, file.bytes)?.parse(&parser_deal_id)
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
