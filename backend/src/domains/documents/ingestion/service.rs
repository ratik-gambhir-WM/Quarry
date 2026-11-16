use std::{path::Path, sync::Arc, time::Instant};

use base64::{engine::general_purpose, Engine as _};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

pub use crate::domains::documents::model::{Document, DocumentChunk};

use crate::{
    adapters::openai::client::{OpenAiClient, ResponsesFileInput},
    domains::documents::{
        formats::{
            image::{build_image_assembly, validate_image_by_bytes, ImageAssembly},
            ParsedQuarryFile, QuarryFile,
        },
        ingestion::persistence::persist_document_and_chunks,
        store::repository::DocumentStore,
    },
    shared::{
        error::{ServiceError, ServiceResult},
        file_policy::infer_supported_image_mime_type,
        ids::sha256_hex,
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

struct ParsedDocument {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    lifecycle: Option<crate::domains::deals::deletion::lifecycle::DealLifecycle>,
    files: DocumentStore,
    jobs: super::repository::DocumentJobRepository,
    openai: Option<Arc<OpenAiClient>>,
    embedding_model: String,
    image_description_model: String,
    office_parse_semaphore: Arc<Semaphore>,
    embedding_request_semaphore: Arc<Semaphore>,
    max_concurrent_documents: usize,
}

impl DocumentIngestionService {
    pub(crate) fn new(
        files: DocumentStore,
        jobs: super::repository::DocumentJobRepository,
        openai: Option<Arc<OpenAiClient>>,
        embedding_model: String,
        image_description_model: String,
        max_concurrent_documents: usize,
    ) -> Self {
        Self {
            lifecycle: None,
            files,
            jobs,
            openai,
            embedding_model,
            image_description_model,
            office_parse_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_OFFICE_PARSERS)),
            embedding_request_semaphore: Arc::new(Semaphore::new(
                MAX_CONCURRENT_EMBEDDING_REQUESTS,
            )),
            max_concurrent_documents,
        }
    }

    pub(crate) async fn admit_deal(
        &self,
        deal: &str,
    ) -> Option<crate::domains::deals::deletion::lifecycle::DealAdmission> {
        match &self.lifecycle {
            Some(gate) => Some(gate.admit(deal).await),
            None => None,
        }
    }
    pub(crate) fn with_lifecycle(
        mut self,
        lifecycle: crate::domains::deals::deletion::lifecycle::DealLifecycle,
    ) -> Self {
        self.lifecycle = Some(lifecycle);
        self
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
    pub(crate) fn queue(&self) -> super::repository::DocumentJobRepository {
        self.jobs.clone()
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
        let mut indexed = stream::iter(files.into_iter().enumerate())
            .map(|(index, file)| async move {
                let queue = self.queue();
                let (id, _) = queue.admit(deal_id.into(), actor_id, file, false).await?;
                if let Some(mut completed) = queue.job(&id).await?.result {
                    if completed.success {
                        completed.skipped = true;
                        completed.chunk_count = 0;
                    }
                    return Ok::<_, ServiceError>((index, completed));
                }
                loop {
                    if let Some(claim) = queue.claim(Some(&id)).await? {
                        if self.process_claim(claim).await.is_err() {
                            // The attempt owns failure publication; cancelled HTTP futures
                            // simply leave an expired request-owned attempt for explicit retry.
                        }
                    }
                    let job = queue.job(&id).await?;
                    if let Some(result) = job.result {
                        return Ok::<_, ServiceError>((index, result));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            })
            .buffer_unordered(self.max_concurrent_documents)
            .try_collect::<Vec<_>>()
            .await?;
        indexed.sort_by_key(|(index, _)| *index);
        let documents: Vec<_> = indexed.into_iter().map(|(_, document)| document).collect();
        let succeeded = documents.iter().filter(|d| d.success).count();
        let skipped = documents.iter().filter(|d| d.skipped).count();
        Ok(ProcessDocumentsResponse {
            total,
            succeeded,
            skipped,
            failed: total - succeeded,
            documents,
        })
    }
    pub(crate) async fn process_claim(
        &self,
        claim: super::repository::Claimed,
    ) -> ServiceResult<()> {
        let _admission = self.admit_deal(&claim.attempt.deal).await;
        let attempt = claim.attempt.clone();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(570),
            self.prepare_and_commit(claim),
        )
        .await;
        match result {
            Ok(Ok(())) => Ok(()),
            _ => {
                tracing::warn!(job_id=%attempt.id,stage="preparing_or_committing",category="attempt_failed","document attempt failed");
                self.queue().fail(&attempt).await?;
                Err(ServiceError::unavailable("document processing failed"))
            }
        }
    }
    async fn prepare_and_commit(&self, claim: super::repository::Claimed) -> ServiceResult<()> {
        let attempt = claim.attempt;
        self.files
            .authorize_upload(&attempt.deal, attempt.actor)
            .await?;
        if attempt.profile_id != self.files.profile().id {
            return Err(ServiceError::unavailable(
                "document embedding profile changed",
            ));
        }
        if self.files.complete_current_attempt(attempt.clone()).await? {
            return Ok(());
        }
        let openai = self
            .openai
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("OpenAI capability is not configured"))?;
        let mut parsed = parse_document(
            claim.file,
            attempt.deal.clone(),
            openai,
            &self.image_description_model,
            self.office_parse_semaphore.clone(),
            claim.image,
        )
        .await
        .map_err(ServiceError::internal)?;
        parsed.document.file_id = attempt.file_id.clone();
        if infer_supported_image_mime_type(Path::new(&attempt.filename)).is_some() {
            let assembly = ImageAssembly {
                document: parsed.document.clone(),
                chunks: parsed.chunks.clone(),
            };
            self.queue()
                .checkpoint_image(
                    &attempt,
                    serde_json::to_string(&assembly)
                        .map_err(|_| ServiceError::internal("image checkpoint encoding failed"))?,
                )
                .await?;
        }
        self.embed_chunks(
            &attempt.filename,
            parsed.document.file_size_bytes,
            &mut parsed.chunks,
            openai,
        )
        .await
        .map_err(ServiceError::internal)?;
        persist_document_and_chunks(
            &self.files,
            &attempt.deal,
            attempt.actor,
            parsed.document,
            parsed.chunks,
            parsed.file_bytes,
            Some(attempt.clone()),
        )
        .await
        .map_err(|_| ServiceError::internal("document commit failed"))?;
        Ok(())
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
        let total_batches = batches.len();
        let input_chunks = &*chunks;
        let batch_results = stream::iter(batches)
            .map(|batch| {
                self.embed_batch(
                    batch,
                    input_chunks,
                    openai,
                    filename,
                    file_size_bytes,
                    total_batches,
                )
            })
            .buffer_unordered(MAX_CONCURRENT_EMBEDDING_REQUESTS)
            .try_collect::<Vec<_>>()
            .await?;

        for (batch, embeddings) in batch_results {
            for (chunk, embedding) in chunks[batch.start..batch.end].iter_mut().zip(embeddings) {
                chunk.embedding = Some(
                    crate::adapters::postgres::vector::CosineVector::from_f64(
                        &embedding,
                        self.files.profile().dimensions,
                    )
                    .map_err(|_| "invalid provider embedding".to_string())?
                    .as_slice()
                    .to_vec(),
                );
            }
        }
        Ok(())
    }

    async fn embed_batch(
        &self,
        batch: EmbeddingBatch,
        chunks: &[DocumentChunk],
        openai: &OpenAiClient,
        filename: &str,
        file_size_bytes: u64,
        total_batches: usize,
    ) -> Result<(EmbeddingBatch, Vec<Vec<f64>>), String> {
        let total_chunks = chunks.len();
        let contents = chunks[batch.start..batch.end]
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>();
        let _permit = self
            .embedding_request_semaphore
            .acquire()
            .await
            .map_err(|_| "embedding request concurrency control is shutting down".to_string())?;
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

async fn parse_document(
    file: UploadedDocument,
    deal_id: String,
    openai: &OpenAiClient,
    image_description_model: &str,
    office_parse_semaphore: Arc<Semaphore>,
    recovery: Option<ImageAssembly>,
) -> Result<ParsedDocument, String> {
    if let Some(assembly) = recovery {
        if assembly.document.deal_id != deal_id
            || assembly.document.content_hash != uploaded_document_content_hash(&file)
        {
            return Err("stored image recovery does not match upload".into());
        }
        return Ok(ParsedDocument {
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
        let (document, chunks) =
            if infer_supported_image_mime_type(Path::new(&file.filename)).is_some() {
                parse_image_document(file, &deal_id, openai, image_description_model).await?
            } else {
                parse_file_document(file, &deal_id, office_parse_semaphore).await?
            };
        Ok(ParsedDocument {
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

async fn parse_image_document(
    file: UploadedDocument,
    deal_id: &str,
    openai: &OpenAiClient,
    image_description_model: &str,
) -> Result<(Document, Vec<DocumentChunk>), String> {
    let validated = validate_image_by_bytes(file.bytes, &file.filename).await?;
    let description = describe_image(
        &validated.bytes,
        validated.mime_type,
        openai,
        image_description_model,
    )
    .await?;
    let assembly =
        build_image_assembly(validated.bytes, None, file.filename, deal_id, &description)?;
    Ok((assembly.document, assembly.chunks))
}

async fn parse_file_document(
    file: UploadedDocument,
    deal_id: &str,
    office_parse_semaphore: Arc<Semaphore>,
) -> Result<(Document, Vec<DocumentChunk>), String> {
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
    let parser_deal_id = deal_id.to_owned();
    let parsed = tokio::task::spawn_blocking(move || {
        let _office_permit = office_permit;
        QuarryFile::from_bytes(parser_filename, file.bytes)?.parse(&parser_deal_id)
    })
    .await
    .map_err(|_| "document parsing worker failed".to_string())??;
    Ok(match parsed {
        ParsedQuarryFile::Pdf(assembly) => (assembly.document, assembly.chunks),
        ParsedQuarryFile::Docx(assembly) => (assembly.document, assembly.chunks),
        ParsedQuarryFile::Powerpoint(assembly) => (assembly.document, assembly.chunks),
        ParsedQuarryFile::Spreadsheet(assembly) => (assembly.document, assembly.chunks),
    })
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
