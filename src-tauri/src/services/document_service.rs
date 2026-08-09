use std::{env, fs, path::Path};

use crate::core::parsers::pdf::parse_pdf_document as parse_pdf_document_from_parser;
pub use crate::core::parsers::pdf::PdfDocumentAssembly;
use crate::core::parsers::{ParsedQuarryFile, QuarryFile};
use crate::core::{
    clients::openai::OpenAiClient,
    nodes::document_node::{ChunkNode, DocumentNode},
};
use crate::repository::document_repository::{
    ensure_document_indexes, find_existing_document_id_by_content_hash, persist_document_and_chunks,
};
use crate::state::AppState;
use crate::utils::{document_id_from_content, sha256_hex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedLocalDocument {
    pub document_id: String,
    pub chunk_count: usize,
    pub skipped: bool,
}

pub async fn process_local_document(
    state: &AppState,
    path: &Path,
    user_id: &str,
) -> Result<ProcessedLocalDocument, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read selected document: {error}"))?;
    if bytes.is_empty() {
        return Err("selected document is empty".to_string());
    }
    let content_hash = sha256_hex(&bytes);
    let document_id = document_id_from_content(user_id, &content_hash);
    let _guard = state.document_jobs().lock_document(&document_id).await;
    let helix = state.gen_helix_db_client();

    ensure_document_indexes(helix)
        .await
        .map_err(|error| format!("failed to initialize Helix document indexes: {error}"))?;
    if let Some(existing_document_id) =
        find_existing_document_id_by_content_hash(helix, user_id, &content_hash).await?
    {
        return Ok(ProcessedLocalDocument {
            document_id: existing_document_id,
            chunk_count: 0,
            skipped: true,
        });
    }

    let file = QuarryFile::from_local_path(path)?;
    let parsed = file.parse_for_user(user_id).await?;
    let (document, mut chunks) = document_graph_parts(parsed);
    if document.document_id != document_id {
        return Err("document identity did not match the selected file content".to_string());
    }
    let chunk_count = chunks.len();
    let openai = OpenAiClient::new()?;
    embed_chunks(path, &mut chunks, &openai).await?;
    persist_document_and_chunks(helix, document, chunks)
        .await
        .map_err(|error| format!("failed to persist document graph: {error}"))?;

    Ok(ProcessedLocalDocument {
        document_id,
        chunk_count,
        skipped: false,
    })
}

fn document_graph_parts(parsed: ParsedQuarryFile) -> (DocumentNode, Vec<ChunkNode>) {
    match parsed {
        ParsedQuarryFile::Pdf(assembly) => (assembly.document, assembly.chunks),
        ParsedQuarryFile::Docx(assembly) => (assembly.document, assembly.chunks),
    }
}

/// Parses a PDF and bulk-embeds its chunks for graph storage.
pub async fn parse_pdf_document(
    path: &Path,
    user_id: impl Into<String>,
) -> Result<PdfDocumentAssembly, String> {
    let assembly = parse_pdf_document_from_parser(path, user_id)?;
    if assembly.chunks.is_empty() {
        return Ok(assembly);
    }

    let openai_client = OpenAiClient::new()?;
    embed_pdf_chunks(path, assembly, &openai_client).await
}

async fn embed_pdf_chunks(
    path: &Path,
    mut assembly: PdfDocumentAssembly,
    openai_client: &OpenAiClient,
) -> Result<PdfDocumentAssembly, String> {
    embed_chunks(path, &mut assembly.chunks, openai_client).await?;
    Ok(assembly)
}

async fn embed_chunks(
    path: &Path,
    chunks: &mut [ChunkNode],
    openai_client: &OpenAiClient,
) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }

    let contents = chunks
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<Vec<_>>();
    let model = env::var("OPENAI_EMBEDDING_MODEL").ok();
    let embeddings = openai_client
        .gen_embeddings(&contents, model.as_deref())
        .await
        .map_err(|err| format!("failed to embed chunks for {}: {err}", path.display()))?;

    attach_embeddings_to_chunks(chunks, embeddings)
}

#[cfg(test)]
fn attach_chunk_embeddings(
    assembly: &mut PdfDocumentAssembly,
    embeddings: Vec<Vec<f64>>,
) -> Result<(), String> {
    attach_embeddings_to_chunks(&mut assembly.chunks, embeddings)
}

fn attach_embeddings_to_chunks(
    chunks: &mut [ChunkNode],
    embeddings: Vec<Vec<f64>>,
) -> Result<(), String> {
    if chunks.len() != embeddings.len() {
        return Err(format!(
            "OpenAI returned {} embeddings for {} PDF chunks",
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
#[path = "../../tests/services/document_service_tests.rs"]
mod tests;
