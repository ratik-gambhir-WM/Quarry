//! Durable, bounded classification and embedding worker for saved slide imports.

use super::super::{
    retrieval::{SlideClassificationJob, SlideEmbeddingJob},
    TemplateRepository,
};
use super::{SlideClassificationService, SlideProviderError};
use crate::shared::on_demand_task::OnDemandTask;
use chrono::{DateTime, Utc};
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
    pub now: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    pub random: Arc<dyn Fn() -> f64 + Send + Sync>,
    pub repository: Arc<dyn TemplateRepository>,
    pub service: Arc<SlideClassificationService>,
    pub import: Arc<super::super::ImportService>,
    pub projection: Option<Arc<super::super::graph::TemplateGraph>>,
    pub mutations: Arc<tokio::sync::Mutex<()>>,
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

#[derive(Clone, Copy)]
enum Stage {
    Classification,
    Embedding,
}

struct StagePolicy {
    name: &'static str,
    maximum_attempts: usize,
    timeout: Duration,
}

impl Stage {
    fn policy(self, options: &SlideClassificationWorkerOptions) -> StagePolicy {
        match self {
            Self::Classification => StagePolicy {
                name: "classification",
                maximum_attempts: options.classification_max_attempts,
                timeout: options.classification_timeout,
            },
            Self::Embedding => StagePolicy {
                name: "embedding",
                maximum_attempts: options.embedding_max_attempts,
                timeout: options.embedding_timeout,
            },
        }
    }

    fn record_failure(
        self,
        repository: &dyn TemplateRepository,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> Result<(), crate::shared::error::RepositoryError> {
        match self {
            Self::Classification => repository.record_slide_classification_failure(id, code, retry),
            Self::Embedding => repository.record_slide_embedding_failure(id, code, retry),
        }
    }
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
                let now = (self.options.now)();
                let worker = Arc::clone(&self);
                let prefer = self.prefer_classification.load(Ordering::Acquire);
                let work = tokio::task::spawn_blocking(move || worker.claim(now, prefer))
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
                match tokio::task::spawn_blocking(move || repository.has_pending_slide_processing())
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
    fn claim(
        &self,
        now: DateTime<Utc>,
        prefer_classification: bool,
    ) -> anyhow::Result<Option<Work>> {
        let classification = || {
            self.options
                .repository
                .claim_next_slide_classification(
                    &iso(now),
                    &iso(now
                        + chrono_duration(
                            self.options.classification_timeout + self.options.lease_padding,
                        )),
                )
                .map(|job| job.map(Work::Classification))
        };
        let embedding = || {
            self.options
                .repository
                .claim_next_slide_embedding(
                    &iso(now),
                    &iso(now
                        + chrono_duration(
                            self.options.embedding_timeout + self.options.lease_padding,
                        )),
                )
                .map(|job| job.map(Work::Embedding))
        };
        if prefer_classification {
            match classification()? {
                Some(work) => Ok(Some(work)),
                None => Ok(embedding()?),
            }
        } else {
            match embedding()? {
                Some(work) => Ok(Some(work)),
                None => Ok(classification()?),
            }
        }
    }
    async fn execute(&self, work: Work) {
        let mut cancelled = self.cancel.subscribe();
        let (stage, id, attempt) = match &work {
            Work::Classification(job) => (
                Stage::Classification,
                job.template.template_id.clone(),
                job.attempt_count,
            ),
            Work::Embedding(job) => (Stage::Embedding, job.template_id.clone(), job.attempt_count),
        };
        let StagePolicy {
            name,
            maximum_attempts: maximum,
            timeout,
        } = stage.policy(&self.options);
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
        if let Err(error) = result {
            let provider = error.downcast_ref::<SlideProviderError>();
            let retry = if provider.is_none_or(|error| error.retryable) && attempt < maximum {
                Some(self.retry_at(attempt))
            } else {
                None
            };
            let fallback = format!("{name}_internal");
            let code = provider.map_or(fallback.as_str(), |error| error.code.as_str());
            let repository = self.options.repository.clone();
            let code = code.to_owned();
            let id = id.clone();
            let recorded = tokio::task::spawn_blocking(move || {
                stage.record_failure(repository.as_ref(), &id, &code, retry.as_deref())
            })
            .await
            .map_err(anyhow::Error::from)
            .and_then(|result| result.map_err(Into::into));
            if let Err(error) = recorded {
                let _ = error;
                tracing::warn!("classification worker could not record failure");
            }
        }
        if let Some(projection) = &self.options.projection {
            // Serialize the snapshot + graph write with HTTP deletion/publication.
            let _mutation = self.options.mutations.lock().await;
            let import = self.options.import.clone();
            let record = tokio::task::spawn_blocking(move || import.projection_record(&id)).await;
            match record {
                Ok(Ok(Some(record))) => {
                    if !matches!(
                        tokio::time::timeout(Duration::from_secs(10), projection.upsert(record))
                            .await,
                        Ok(Ok(()))
                    ) {
                        tracing::warn!("classification saved; template projection failed and will reconcile on restart");
                    }
                }
                Ok(Ok(None)) => {} // Deleted while the provider was running.
                _ => tracing::warn!("classification worker could not read projection record"),
            }
        }
    }
    pub fn retry_at(&self, attempt: usize) -> String {
        let exponential = self.options.retry_base.as_millis() as f64
            * 2.0_f64.powf(attempt.saturating_sub(1) as f64);
        let jitter = (exponential * 0.25 * (self.options.random)()).floor();
        iso((self.options.now)() + chrono::Duration::milliseconds((exponential + jitter) as i64))
    }
}

fn iso(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn chrono_duration(value: Duration) -> chrono::Duration {
    chrono::Duration::milliseconds(value.as_millis().min(i64::MAX as u128) as i64)
}

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/providers/worker_tests.rs"]
mod tests;
