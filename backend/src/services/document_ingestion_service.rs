use std::{
    collections::HashMap,
    sync::{Arc, Weak},
    time::Instant,
};

use futures_util::{stream, StreamExt};
use serde::Serialize;
use tokio::sync::{Mutex, OwnedMutexGuard};

pub use crate::core::models::document::{Document, DocumentChunk};

use crate::{
    core::{
        clients::openai::OpenAiClient,
        models::file_persistence::PersistedFileIdentity,
        parsers::{ParsedQuarryFile, QuarryFile},
    },
    repository::document_repository::{DocumentFileRepository, DocumentIndexRepository},
    services::{
        document_service::persist_document_and_chunks,
        error::{ServiceError, ServiceResult},
    },
    utils::{document_id_from_content, sha256_hex},
};

const DOCUMENT_PARSE_API: &str = "document.parse";

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
    files: DocumentFileRepository,
    index: DocumentIndexRepository,
    openai: Option<Arc<OpenAiClient>>,
    embedding_model: String,
    processing_locks: Arc<Mutex<HashMap<String, Weak<Mutex<()>>>>>,
    max_concurrent_documents: usize,
}

impl DocumentIngestionService {
    pub fn new(
        files: DocumentFileRepository,
        index: DocumentIndexRepository,
        openai: Option<Arc<OpenAiClient>>,
        embedding_model: String,
        max_concurrent_documents: usize,
    ) -> Self {
        Self {
            files,
            index,
            openai,
            embedding_model,
            processing_locks: Arc::new(Mutex::new(HashMap::new())),
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
                .index
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
        let mut graph = parse_document(file, user_id)?;
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
            &self.index,
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
        let contents = chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>();
        let embeddings = openai
            .gen_embeddings_for_file(
                &contents,
                Some(&self.embedding_model),
                filename,
                file_size_bytes,
            )
            .await
            .map_err(|error| format!("failed to embed chunks for `{filename}`: {error}"))?;
        if chunks.len() != embeddings.len() {
            return Err(format!(
                "OpenAI returned {} embeddings for {} chunks in `{filename}`",
                embeddings.len(),
                chunks.len()
            ));
        }
        for (chunk, embedding) in chunks.iter_mut().zip(embeddings) {
            chunk.embedding = Some(embedding.into_iter().map(|value| value as f32).collect());
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

fn parse_document(file: UploadedDocument, user_id: String) -> Result<ParsedDocumentGraph, String> {
    let filename = file.filename.clone();
    let file_bytes = file.bytes.clone();
    let file_size_bytes = u64::try_from(file.bytes.len())
        .map_err(|_| format!("file size for `{filename}` does not fit in u64"))?;
    let started_at = Instant::now();
    let result = (|| {
        let parsed = QuarryFile::from_bytes(file.filename, file.bytes)?.parse(&user_id)?;
        let (document, chunks) = match parsed {
            ParsedQuarryFile::Pdf(assembly) => (assembly.document, assembly.chunks),
            ParsedQuarryFile::Docx(assembly) => (assembly.document, assembly.chunks),
        };
        Ok(ParsedDocumentGraph {
            document,
            chunks,
            file_bytes,
        })
    })();
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

#[cfg(test)]
#[path = "../../tests/services/document_ingestion_service_tests.rs"]
mod tests;
