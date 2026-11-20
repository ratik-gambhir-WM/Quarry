//! Classification inside each document worker, plus explicit reclassification.
//!
//! The document task samples its parsed chunks, then runs [`ClassificationService::classify`]
//! beside embedding. The outcome is saved in the same transaction as the document. A
//! classification failure never fails the document.
use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Semaphore;

use super::{
    model::{ClassificationResult, DocumentClassification, ErrorCode, RunStatus, SAMPLE_VERSION},
    policy::{self, Provenance},
    repository::{ClassificationRecord, ClassificationRepository, CommittedVersion, StoredChunk},
    sample::{self, ChunkMeta, EvidenceKind, EvidenceSample, SampleContext, SampleOutcome},
};
use crate::{
    adapters::openai::decisions::{DecisionsClient, DecisionsError},
    domains::documents::model::{Document, DocumentChunk},
    shared::{
        error::{RepositoryError, ServiceError, ServiceResult},
        file_policy::infer_supported_image_mime_type,
        prompts::{PromptCatalog, PromptId},
    },
};

/// Per attempt: connect, send, body read and decoding.
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_ATTEMPTS: i64 = 3;
const BASE_RETRY_SECONDS: u64 = 2;
const MAX_RETRY_SECONDS: u64 = 60;
const MAX_RESULT_BYTES: usize = 16 * 1024;

/// Bounded evidence sampled from parsed chunks before embedding takes them mutably.
#[derive(Clone, Debug)]
pub(crate) struct PreparedEvidence {
    sample: EvidenceSample,
    fingerprint: String,
    empty: bool,
}

pub struct ClassificationService {
    repository: ClassificationRepository,
    decisions: Option<DecisionsClient>,
    prompts: Arc<PromptCatalog>,
    enabled: bool,
    /// Concurrent Decisions requests across every document task; held only per request.
    requests: Arc<Semaphore>,
}

impl ClassificationService {
    pub(crate) fn new(
        repository: ClassificationRepository,
        decisions: Option<DecisionsClient>,
        prompts: Arc<PromptCatalog>,
        enabled: bool,
        concurrency: usize,
    ) -> Self {
        Self {
            repository,
            decisions,
            prompts,
            enabled,
            requests: Arc::new(Semaphore::new(concurrency.clamp(1, 8))),
        }
    }

    /// Provider work runs only when enabled and a server-side key is configured.
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.decisions.is_some()
    }

    /// Sample a parsed document without copying its text (CPU-bounded: ≤32 KiB tokenized).
    pub(crate) fn prepare(
        &self,
        filename: &str,
        document: &Document,
        chunks: &[DocumentChunk],
    ) -> PreparedEvidence {
        let pages: Vec<Option<(i64, i64)>> = chunks.iter().map(page_range).collect();
        let meta: Vec<ChunkMeta<'_>> = chunks
            .iter()
            .zip(&pages)
            .map(|(chunk, pages)| ChunkMeta {
                index: i64::from(chunk.sequence_number),
                token_count: u64::from(chunk.token_count),
                sha256: &chunk.content_hash,
                section: chunk.section_title.as_deref().filter(|s| !s.is_empty()),
                pages: *pages,
            })
            .collect();
        let texts: HashMap<i64, &str> = chunks
            .iter()
            .map(|chunk| (i64::from(chunk.sequence_number), chunk.text.as_str()))
            .collect();
        evidence(
            context(filename, &document.source_type),
            &meta,
            &texts,
            &document.content_hash,
        )
    }

    /// Classify one document inside its worker. Always returns a record to save with the
    /// document: completed, failed (after bounded retries) or disabled.
    pub(crate) async fn classify(&self, prepared: PreparedEvidence) -> ClassificationRecord {
        let provenance = Provenance {
            fingerprint: prepared.fingerprint.clone(),
            sampled: prepared.sample.sampled,
        };
        if prepared.empty {
            return completed(prepared, &policy::insufficient(&provenance));
        }
        if !self.is_enabled() {
            return record(prepared, RunStatus::Disabled, None, None);
        }
        match self.decide(&prepared.sample.text, &provenance).await {
            Ok(result) => completed(prepared, &result),
            Err(code) => record(prepared, RunStatus::Failed, None, Some(code)),
        }
    }

    /// One Decisions request with up to three attempts. The request permit is released
    /// while waiting between attempts.
    async fn decide(
        &self,
        evidence: &str,
        provenance: &Provenance,
    ) -> Result<ClassificationResult, ErrorCode> {
        let client = self.decisions.as_ref().ok_or(ErrorCode::Unavailable)?;
        let instructions = self.prompts.prompt(PromptId::DocumentClassification);
        let request = policy::request(instructions, evidence);
        for attempt in 1..=MAX_ATTEMPTS {
            let response = {
                let _permit = self
                    .requests
                    .acquire()
                    .await
                    .map_err(|_| ErrorCode::Unavailable)?;
                let started = Instant::now();
                let response = client.decide(&request, PROVIDER_TIMEOUT).await;
                let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                match &response {
                    Ok(response) => tracing::info!(
                        api = "openai.decisions",
                        elapsed_ms,
                        attempt,
                        input_tokens = response.usage.map_or(0, |usage| usage.input_tokens),
                        "document classified"
                    ),
                    Err(error) => tracing::warn!(
                        api = "openai.decisions",
                        elapsed_ms,
                        attempt,
                        category = error_code(*error).as_str(),
                        "document classification attempt failed"
                    ),
                }
                response
            };
            match response {
                Ok(response) => {
                    return policy::normalize(
                        &response.answers,
                        response.model.as_deref(),
                        provenance,
                    )
                    .ok()
                    .filter(ClassificationResult::is_consistent)
                    .ok_or(ErrorCode::InvalidResponse);
                }
                Err(error) if error.is_retryable() && attempt < MAX_ATTEMPTS => {
                    tokio::time::sleep(Duration::from_secs(u64::from(retry_delay(error, attempt))))
                        .await;
                }
                Err(error) => return Err(error_code(error)),
            }
        }
        Err(ErrorCode::Unavailable)
    }

    /// Explicitly reclassify the current committed version, synchronously. Reuses the stored
    /// sample (or resamples stored chunk text); never parses originals or touches vectors.
    /// A provider failure leaves the stored classification unchanged.
    pub async fn request(
        &self,
        deal: &str,
        file_id: &str,
        actor: i64,
        expected_generation: i64,
    ) -> ServiceResult<DocumentClassification> {
        if !self.is_enabled() {
            return Err(ServiceError::unavailable(
                "Document classification is not enabled",
            ));
        }
        let target = self
            .repository
            .reclassify_target(deal, file_id, actor)
            .await
            .map_err(map_repository)?;
        let current = target.stored.as_ref().map_or(0, |stored| stored.generation);
        if current != expected_generation {
            return Err(stale());
        }
        let prepared = match &target.stored {
            Some(stored) if stored.sample_version == SAMPLE_VERSION => PreparedEvidence {
                fingerprint: policy::fingerprint(
                    &target.version.content_sha256,
                    &stored.sample.hash,
                ),
                empty: stored.sample.text.is_empty(),
                sample: stored.sample.clone(),
            },
            _ => self.sample_committed(&target.version).await?,
        };
        let provenance = Provenance {
            fingerprint: prepared.fingerprint.clone(),
            sampled: prepared.sample.sampled,
        };
        let result = if prepared.empty {
            policy::insufficient(&provenance)
        } else {
            self.decide(&prepared.sample.text, &provenance)
                .await
                .map_err(|_| {
                    ServiceError::unavailable("Document classification failed; retry later")
                })?
        };
        let (generation, completed_at) = self
            .repository
            .apply_reclassification(
                &target,
                actor,
                expected_generation,
                completed(prepared, &result),
            )
            .await
            .map_err(map_repository)?;
        let mut result = result;
        result.classified_at = completed_at
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|at| at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        Ok(DocumentClassification {
            version_id: target.version.version_id,
            run_status: RunStatus::Completed.as_str().into(),
            generation: Some(generation),
            last_error_code: None,
            result: Some(result),
        })
    }

    /// Rebuild the same `sample-v1` evidence from stored chunks: metadata first, then only
    /// the selected texts.
    async fn sample_committed(
        &self,
        version: &CommittedVersion,
    ) -> ServiceResult<PreparedEvidence> {
        let chunks = self
            .repository
            .chunk_metadata(&version.version_id)
            .await
            .map_err(map_repository)?;
        let source_type = Path::new(&version.original_filename)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let context = context(&version.original_filename, &source_type);
        let meta = stored_meta(&chunks);
        let needed = sample::initial_plan(context, &meta).needed_chunks(&meta);
        let loaded = self
            .repository
            .chunk_texts(&version.version_id, &needed)
            .await
            .map_err(map_repository)?;
        let texts: HashMap<i64, &str> = loaded
            .iter()
            .map(|(index, text)| (*index, text.as_str()))
            .collect();
        Ok(evidence(context, &meta, &texts, &version.content_sha256))
    }
}

fn completed(prepared: PreparedEvidence, result: &ClassificationResult) -> ClassificationRecord {
    match serde_json::to_string(result)
        .ok()
        .filter(|json| json.len() <= MAX_RESULT_BYTES)
    {
        Some(json) => record(prepared, RunStatus::Completed, Some(json), None),
        None => record(
            prepared,
            RunStatus::Failed,
            None,
            Some(ErrorCode::InvalidResponse),
        ),
    }
}

fn record(
    prepared: PreparedEvidence,
    status: RunStatus,
    result_json: Option<String>,
    error: Option<ErrorCode>,
) -> ClassificationRecord {
    ClassificationRecord {
        sample: prepared.sample,
        fingerprint: prepared.fingerprint,
        status,
        result_json,
        error,
    }
}

fn stored_meta(chunks: &[StoredChunk]) -> Vec<ChunkMeta<'_>> {
    chunks
        .iter()
        .map(|chunk| ChunkMeta {
            index: chunk.index,
            token_count: chunk.token_count,
            sha256: &chunk.sha256,
            section: chunk.section.as_deref(),
            pages: chunk.pages,
        })
        .collect()
}

fn context<'a>(filename: &'a str, source_type: &'a str) -> SampleContext<'a> {
    SampleContext {
        filename,
        source_type,
        evidence: if infer_supported_image_mime_type(Path::new(filename)).is_some() {
            EvidenceKind::ImageDescription
        } else {
            EvidenceKind::ExtractedText
        },
    }
}

fn evidence(
    context: SampleContext<'_>,
    meta: &[ChunkMeta<'_>],
    texts: &HashMap<i64, &str>,
    content_sha256: &str,
) -> PreparedEvidence {
    let (sample, empty) = match sample::build(context, meta, texts) {
        SampleOutcome::Evidence(sample) => (sample, false),
        SampleOutcome::Empty(sample) => (sample, true),
    };
    PreparedEvidence {
        fingerprint: policy::fingerprint(content_sha256, &sample.hash),
        sample,
        empty,
    }
}

fn page_range(chunk: &DocumentChunk) -> Option<(i64, i64)> {
    let pages = chunk
        .page_numbers
        .as_ref()
        .filter(|pages| !pages.is_empty())?;
    Some((
        i64::from(*pages.iter().min()?),
        i64::from(*pages.iter().max()?),
    ))
}

fn error_code(error: DecisionsError) -> ErrorCode {
    match error {
        DecisionsError::Timeout => ErrorCode::Timeout,
        DecisionsError::Transport => ErrorCode::Transport,
        DecisionsError::RateLimited { .. } => ErrorCode::RateLimited,
        DecisionsError::Unavailable { .. } => ErrorCode::UpstreamUnavailable,
        DecisionsError::Unauthorized => ErrorCode::Unauthorized,
        DecisionsError::Rejected { .. } | DecisionsError::RequestTooLarge => {
            ErrorCode::RequestRejected
        }
        DecisionsError::ResponseTooLarge | DecisionsError::InvalidResponse => {
            ErrorCode::InvalidResponse
        }
    }
}

/// Honor a bounded `Retry-After`; otherwise 2s then 4s plus up to 25% jitter, ≤60s.
pub(crate) fn retry_delay(error: DecisionsError, attempt: i64) -> u32 {
    let hinted = match error {
        DecisionsError::RateLimited { retry_after }
        | DecisionsError::Unavailable { retry_after } => retry_after,
        _ => None,
    };
    let seconds = hinted
        .map(|delay| delay.as_secs().max(1))
        .unwrap_or_else(|| {
            let exponent = u32::try_from(attempt.clamp(1, 6) - 1).unwrap_or(0);
            let base = BASE_RETRY_SECONDS.saturating_mul(1 << exponent);
            let jitter = u64::from(uuid::Uuid::new_v4().as_bytes()[0]) * base / (4 * 255);
            base + jitter
        });
    u32::try_from(seconds.min(MAX_RETRY_SECONDS)).unwrap_or(60)
}

fn stale() -> ServiceError {
    ServiceError::Conflict("Classification changed; refresh documents and retry".into())
}

fn map_repository(error: RepositoryError) -> ServiceError {
    match error {
        RepositoryError::Conflict(_) => stale(),
        other => other.into(),
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/classification/service_tests.rs"]
mod tests;
