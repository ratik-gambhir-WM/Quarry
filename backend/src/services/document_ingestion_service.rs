use std::{env, time::Instant};

use futures_util::{stream, StreamExt};
use serde::Serialize;

pub use crate::core::models::document::{Document, DocumentChunk};

use crate::{
    core::{
        clients::openai::OpenAiClient,
        helix_queries::files::search_quarry_file::{
            FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit, VectorFileChunkHit,
        },
        parsers::{ParsedQuarryFile, QuarryFile},
    },
    repository::document_repository::{
        find_current_sqlite_file_by_content_hash, get_current_helix_document,
        search_document_chunks_by_keyword, search_document_chunks_by_vector,
    },
    services::document_service::persist_document_and_chunks,
    state::AppState,
    utils::{document_id_from_content, openai_api_key, sha256_hex},
};

const MAX_CONCURRENT_DOCUMENTS: usize = 8;
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

pub async fn process_uploaded_documents(
    state: &AppState,
    deal_id: &str,
    user_id: &str,
    files: Vec<UploadedDocument>,
) -> Result<ProcessDocumentsResponse, String> {
    let total = files.len();
    let worker_state = state.clone();
    let worker_deal_id = deal_id.to_string();
    let worker_user_id = user_id.to_string();
    let mut indexed_documents = stream::iter(files.into_iter().enumerate())
        .map(|(index, file)| {
            let state = worker_state.clone();
            let deal_id = worker_deal_id.clone();
            let user_id = worker_user_id.clone();
            let filename = file.filename.clone();

            async move {
                let worker = tokio::spawn(process_uploaded_document(state, file, deal_id, user_id));
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
        .buffer_unordered(MAX_CONCURRENT_DOCUMENTS)
        .collect::<Vec<_>>()
        .await;
    indexed_documents.sort_unstable_by_key(|(index, _)| *index);
    let documents = indexed_documents
        .into_iter()
        .map(|(_, document)| document)
        .collect::<Result<Vec<_>, _>>()?;
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
    state: AppState,
    file: UploadedDocument,
    deal_id: String,
    user_id: String,
) -> Result<ProcessedDocument, String> {
    let filename = file.filename.clone();
    let content_hash = uploaded_document_content_hash(&file);
    let document_id = document_id_from_content(&user_id, &content_hash);
    let attachment_lock_id = format!("{deal_id}\0{document_id}");
    let _processing_guard = state.lock_document_processing(&attachment_lock_id).await;
    let existing_attachment = match find_current_sqlite_file_by_content_hash(
        state.sqlite(),
        &deal_id,
        &user_id,
        &content_hash,
    )
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
        match get_current_helix_document(state.helix(), &user_id, &existing.file_id).await {
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

    let api_key = openai_api_key()?;
    let openai = OpenAiClient::new(&api_key);
    Ok(
        match process_document(
            &state,
            file,
            deal_id,
            user_id,
            existing_attachment.as_ref(),
            &openai,
        )
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

pub async fn search_chunks_by_vector(
    state: &AppState,
    search: FileChunkVectorSearch,
) -> Result<Vec<VectorFileChunkHit>, String> {
    search_document_chunks_by_vector(state.helix(), search).await
}

pub async fn search_chunks_by_keyword(
    state: &AppState,
    search: FileChunkKeywordSearch,
) -> Result<Vec<KeywordFileChunkHit>, String> {
    search_document_chunks_by_keyword(state.helix(), search).await
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

async fn process_document(
    state: &AppState,
    file: UploadedDocument,
    deal_id: String,
    user_id: String,
    existing_attachment: Option<&crate::core::models::file_persistence::PersistedFileIdentity>,
    openai: &OpenAiClient<'_>,
) -> Result<(String, usize), String> {
    let filename = file.filename.clone();
    let mut graph = parse_document(file, user_id)?;
    if let Some(existing) = existing_attachment {
        graph.document.file_id.clone_from(&existing.file_id);
    }
    let document_id = graph.document.document_id.clone();
    let chunk_count = graph.chunks.len();
    embed_chunks(
        &filename,
        graph.document.file_size_bytes,
        &mut graph.chunks,
        openai,
    )
    .await?;
    persist_document_and_chunks(
        state.sqlite(),
        state.helix(),
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
    filename: &str,
    file_size_bytes: u64,
    chunks: &mut [DocumentChunk],
    openai: &OpenAiClient<'_>,
) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }

    let contents = chunks
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<Vec<_>>();
    let model = env::var("OPENAI_EMBEDDING_MODEL").ok();
    let embeddings = openai
        .gen_embeddings_for_file(&contents, model.as_deref(), filename, file_size_bytes)
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

#[cfg(test)]
#[path = "../../tests/services/document_ingestion_service_tests.rs"]
mod tests;
