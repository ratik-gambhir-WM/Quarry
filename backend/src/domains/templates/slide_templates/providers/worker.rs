//! Durable, bounded classification and embedding worker for saved slide imports.

use super::super::{
    retrieval::{SlideClassificationJob, SlideEmbeddingJob, SlideProcessingStage, StageClaim},
    TemplateProcessingRepository,
};
use super::{SlideClassificationService, SlideProviderError};
use crate::shared::on_demand_task::OnDemandTask;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::watch, task::JoinSet};

pub struct SlideClassificationWorkerOptions {
    pub classification_max_attempts: usize,
    pub classification_timeout: Duration,
    pub concurrency: usize,
    pub embedding_max_attempts: usize,
    pub embedding_timeout: Duration,
    pub lease_padding: Duration,
    pub retry_base: Duration,
    pub random: Arc<dyn Fn() -> f64 + Send + Sync>,
    pub repository: Arc<dyn TemplateProcessingRepository>,
    pub service: Arc<SlideClassificationService>,
}

pub struct SlideClassificationWorker {
    options: SlideClassificationWorkerOptions,
    stopping: AtomicBool,
    prefer_classification: AtomicBool,
    cancel: watch::Sender<bool>,
    task: OnDemandTask,
}

enum Work {
    Classification(SlideClassificationJob),
    Embedding(SlideEmbeddingJob),
}

struct StagePolicy {
    name: &'static str,
    maximum_attempts: usize,
    timeout: Duration,
}

fn policy(stage: SlideProcessingStage, options: &SlideClassificationWorkerOptions) -> StagePolicy {
    match stage {
        SlideProcessingStage::Classification => StagePolicy {
            name: "classification",
            maximum_attempts: options.classification_max_attempts,
            timeout: options.classification_timeout,
        },
        SlideProcessingStage::Embedding => StagePolicy {
            name: "embedding",
            maximum_attempts: options.embedding_max_attempts,
            timeout: options.embedding_timeout,
        },
    }
}

/// Database-clock lease length: provider timeout plus padding, rounded up to whole seconds.
/// One more second covers the floored epoch-second start, so a lease is never shorter
/// than the provider timeout it protects.
fn lease_seconds(timeout: Duration, padding: Duration) -> u32 {
    let total = timeout.saturating_add(padding);
    let seconds = total.as_secs() + u64::from(total.subsec_nanos() > 0) + 1;
    u32::try_from(seconds).unwrap_or(u32::MAX)
}

impl SlideClassificationWorker {
    pub fn new(options: SlideClassificationWorkerOptions) -> Self {
        let (cancel, _) = watch::channel(false);
        Self {
            options,
            stopping: AtomicBool::new(false),
            prefer_classification: AtomicBool::new(true),
            cancel,
            task: OnDemandTask::default(),
        }
    }
    /// Spawn a bounded drain, or wake the current one. Retire when durable work is exhausted.
    pub fn notify(self: &Arc<Self>) {
        if self.stopping.load(Ordering::Acquire) {
            return;
        }
        let worker = Arc::clone(self);
        self.task.notify(async move {
            worker.run().await;
        });
    }
    /// Stop new claims immediately; in-flight jobs can still drain.
    pub fn request_stop(&self) {
        self.stopping.store(true, Ordering::Release);
        self.task.request_stop();
    }
    /// Stop claiming jobs, then cancel provider futures after the bounded drain.
    pub async fn stop(&self, drain_timeout: Duration) {
        self.request_stop();
        let task = self.task.take_task();
        if let Some(mut task) = task {
            if tokio::time::timeout(drain_timeout, &mut task)
                .await
                .is_err()
            {
                self.cancel.send_replace(true);
                // Cancellation is cooperative at every in-flight provider future and records its failure.
                let _ = task.await;
            }
        }
    }
    async fn run(self: Arc<Self>) {
        let mut active = JoinSet::new();
        loop {
            self.task.begin_scan();
            while !self.stopping.load(Ordering::Acquire) && active.len() < self.options.concurrency
            {
                let worker = Arc::clone(&self);
                let prefer = self.prefer_classification.load(Ordering::Acquire);
                let work = tokio::spawn(async move { worker.claim(prefer).await })
                    .await
                    .map_err(anyhow::Error::from)
                    .and_then(|result| result);
                let work = match work {
                    Ok(Some(work)) => work,
                    Ok(None) => break,
                    Err(error) => {
                        let _ = error;
                        tracing::warn!("classification worker could not claim work");
                        break;
                    }
                };
                self.prefer_classification
                    .store(!matches!(work, Work::Classification(_)), Ordering::Release);
                let worker = Arc::clone(&self);
                active.spawn(async move {
                    worker.execute(work).await;
                });
            }
            if active.is_empty() {
                if self.stopping.load(Ordering::Acquire) {
                    return;
                }
                let repository = self.options.repository.clone();
                match tokio::spawn(async move { repository.has_pending_slide_processing().await })
                    .await
                {
                    Ok(Ok(false)) => {
                        if self.task.retire_if_idle() {
                            return;
                        }
                        continue;
                    }
                    Ok(Ok(true)) => {} // Includes future retries and unexpired leases.
                    _ => tracing::warn!("classification worker could not read pending work"),
                }
            }
            tokio::select! {
                _=self.task.wait(Duration::from_secs(1))=>{},
                result=active.join_next(),if !active.is_empty()=>{if let Some(Err(error))=result{tracing::warn!(error=%error,"classification worker task failed");}},
            }
        }
    }
    async fn claim(&self, prefer_classification: bool) -> anyhow::Result<Option<Work>> {
        let options = &self.options;
        let classification = || async {
            options
                .repository
                .claim_next_slide_classification(lease_seconds(
                    options.classification_timeout,
                    options.lease_padding,
                ))
                .await
                .map(|job| job.map(Work::Classification))
        };
        let embedding = || async {
            options
                .repository
                .claim_next_slide_embedding(lease_seconds(
                    options.embedding_timeout,
                    options.lease_padding,
                ))
                .await
                .map(|job| job.map(Work::Embedding))
        };
        if prefer_classification {
            match classification().await? {
                Some(work) => Ok(Some(work)),
                None => Ok(embedding().await?),
            }
        } else {
            match embedding().await? {
                Some(work) => Ok(Some(work)),
                None => Ok(classification().await?),
            }
        }
    }
    async fn execute(&self, work: Work) {
        let mut cancelled = self.cancel.subscribe();
        let (stage, claim, attempt): (_, StageClaim, _) = match &work {
            Work::Classification(job) => (
                SlideProcessingStage::Classification,
                job.claim.clone(),
                job.attempt_count,
            ),
            Work::Embedding(job) => (
                SlideProcessingStage::Embedding,
                job.claim.clone(),
                job.attempt_count,
            ),
        };
        let StagePolicy {
            name,
            maximum_attempts: maximum,
            timeout,
        } = policy(stage, &self.options);
        let operation = async {
            // Expired leases also consume the bounded attempt budget.
            if attempt > maximum {
                return Err(
                    SlideProviderError::new(format!("{name}_attempts_exhausted"), false).into(),
                );
            }
            match work {
                Work::Classification(job) => {
                    if job.preview.is_none() {
                        Err(
                            SlideProviderError::new("classification_preview_unavailable", false)
                                .into(),
                        )
                    } else {
                        self.options.service.classify(job).await
                    }
                }
                Work::Embedding(job) => self.options.service.embed(&job).await,
            }
        };
        let result = if *cancelled.borrow() {
            Err(SlideProviderError::new(format!("{name}_timeout"), true).into())
        } else {
            tokio::select! {
                result=tokio::time::timeout(timeout, operation)=>result.unwrap_or_else(|_| Err(SlideProviderError::new(format!("{name}_timeout"),true).into())),
                _=cancelled.changed()=>Err(SlideProviderError::new(format!("{name}_timeout"),true).into()),
            }
        };
        let Err(error) = result else { return };
        let provider = error.downcast_ref::<SlideProviderError>();
        let retry = if provider.is_none_or(|error| error.retryable) && attempt < maximum {
            Some(self.retry_after_seconds(attempt))
        } else {
            None
        };
        let fallback = format!("{name}_internal");
        let code = provider
            .map_or(fallback.as_str(), |error| error.code.as_str())
            .to_owned();
        let repository = self.options.repository.clone();
        // The repository matches this exact claim, so a stale attempt cannot fail a successor.
        let recorded = tokio::spawn(async move {
            repository
                .record_slide_stage_failure(stage, &claim, &code, retry)
                .await
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|result| result.map_err(Into::into));
        if recorded.is_err() {
            tracing::warn!(
                stage = name,
                "classification worker could not record failure"
            );
        }
    }
    /// Exponential backoff with bounded jitter, rounded up to database-clock seconds.
    pub fn retry_after_seconds(&self, attempt: usize) -> u32 {
        let exponential = self.options.retry_base.as_millis() as f64
            * 2.0_f64.powf(attempt.saturating_sub(1) as f64);
        let jitter = (exponential * 0.25 * (self.options.random)()).floor();
        let seconds = ((exponential + jitter) / 1000.0).ceil();
        if seconds.is_finite() {
            seconds.clamp(0.0, f64::from(u32::MAX)) as u32
        } else {
            u32::MAX
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/providers/worker_tests.rs"]
mod tests;
