//! Dormant compatibility worker. The server's import v2 path processes slides synchronously.

use super::super::{
    retrieval::{SlideClassificationJob, SlideEmbeddingJob},
    TemplateRepository,
};
use super::{SlideClassificationService, SlideProviderError};
use chrono::{DateTime, Utc};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    sync::{watch, Notify},
    task::{JoinHandle, JoinSet},
};

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
}

pub struct SlideClassificationWorker {
    options: SlideClassificationWorkerOptions,
    stopping: AtomicBool,
    polling: AtomicBool,
    prefer_classification: AtomicBool,
    wake: Notify,
    cancel: watch::Sender<bool>,
    task: Mutex<Option<JoinHandle<()>>>,
}

enum Work {
    Classification(SlideClassificationJob),
    Embedding(SlideEmbeddingJob),
}

impl SlideClassificationWorker {
    pub fn new(options: SlideClassificationWorkerOptions) -> Self {
        let (cancel, _) = watch::channel(false);
        Self {
            options,
            stopping: AtomicBool::new(false),
            polling: AtomicBool::new(false),
            prefer_classification: AtomicBool::new(true),
            wake: Notify::new(),
            cancel,
            task: Mutex::new(None),
        }
    }
    /// Start polling and immediately try to lease durable work.
    pub fn start(self: &Arc<Self>) {
        self.polling.store(true, Ordering::Release);
        self.notify();
    }
    /// Drain immediately even when periodic polling has not been started.
    pub fn notify(self: &Arc<Self>) {
        if self.stopping.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut task) = self.task.lock() else {
            return;
        };
        if task.as_ref().is_some_and(|task| !task.is_finished()) {
            self.wake.notify_one();
            return;
        }
        let worker = Arc::clone(self);
        *task = Some(tokio::spawn(async move {
            worker.run().await;
        }));
    }
    /// Stop claiming jobs, then cancel provider futures after the bounded drain.
    pub async fn stop(&self, drain_timeout: Duration) {
        self.stopping.store(true, Ordering::Release);
        self.polling.store(false, Ordering::Release);
        self.wake.notify_one();
        let task = self.task.lock().ok().and_then(|mut task| task.take());
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
        let mut poll = tokio::time::interval(Duration::from_secs(1));
        loop {
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
            if (self.stopping.load(Ordering::Acquire) || !self.polling.load(Ordering::Acquire))
                && active.is_empty()
            {
                return;
            }
            tokio::select! {
                _=poll.tick(),if self.polling.load(Ordering::Acquire)=>{},
                _=self.wake.notified()=>{},
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
        let (classification, id, attempt) = match &work {
            Work::Classification(job) => {
                (true, job.template.template_id.clone(), job.attempt_count)
            }
            Work::Embedding(job) => (false, job.template_id.clone(), job.attempt_count),
        };
        let stage = if classification {
            "classification"
        } else {
            "embedding"
        };
        let operation = async {
            match &work {
                Work::Classification(job) => self.options.service.classify(job).await,
                Work::Embedding(job) => self.options.service.embed(job).await,
            }
        };
        let result = if *cancelled.borrow() {
            Err(SlideProviderError::new(format!("{stage}_timeout"), true).into())
        } else {
            tokio::select! {
                result=operation=>result,
                _=cancelled.changed()=>Err(SlideProviderError::new(format!("{stage}_timeout"),true).into()),
            }
        };
        if let Err(error) = result {
            let provider = error.downcast_ref::<SlideProviderError>();
            let maximum = if classification {
                self.options.classification_max_attempts
            } else {
                self.options.embedding_max_attempts
            };
            let retry = if provider.is_some_and(|error| error.retryable) && attempt < maximum {
                Some(self.retry_at(attempt))
            } else {
                None
            };
            let fallback = format!("{stage}_internal");
            let code = provider.map_or(fallback.as_str(), |error| error.code.as_str());
            let repository = self.options.repository.clone();
            let code = code.to_owned();
            let recorded = tokio::task::spawn_blocking(move || {
                if classification {
                    repository.record_slide_classification_failure(&id, &code, retry.as_deref())
                } else {
                    repository.record_slide_embedding_failure(&id, &code, retry.as_deref())
                }
            })
            .await
            .map_err(anyhow::Error::from)
            .and_then(|result| result.map_err(Into::into));
            if let Err(error) = recorded {
                let _ = error;
                tracing::warn!("classification worker could not record failure");
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
