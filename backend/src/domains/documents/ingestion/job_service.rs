//! Durable document workers and database-polled SSE snapshots.
use super::{
    jobs::{AdmissionStatus, DocumentJobEvent},
    repository::{Claimed, DocumentJobRepository, Job},
    service::{DocumentIngestionService, UploadedDocument},
};
use crate::shared::{
    error::ServiceResult,
    worker::{drain_owned, StopSignal, WorkerHandle, DRAIN_GRACE},
};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinSet};

/// Hard ceiling for concurrently processed background documents.
pub const MAX_BACKGROUND_DOCUMENTS: usize = 8;
const IDLE_POLL: Duration = Duration::from_millis(200);
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct StartedDocumentJob {
    pub job_id: String,
    pub filename: String,
}
#[derive(Clone)]
pub struct DocumentJobService {
    ingestion: Arc<DocumentIngestionService>,
    queue: DocumentJobRepository,
}
impl DocumentJobService {
    pub fn new(ingestion: Arc<DocumentIngestionService>) -> Self {
        let queue = ingestion.queue();
        Self { ingestion, queue }
    }

    /// Start the single background supervisor. Bootstrap calls this once and owns the
    /// handle; service clones never start or stop another pool.
    pub fn start_worker(&self, concurrency: usize, retention: Duration) -> WorkerHandle {
        // The processing callback owns background stages; admission is request-owned.
        let ingestion = Arc::clone(&self.ingestion);
        let process: ProcessDocument = Arc::new(move |claim| {
            let ingestion = Arc::clone(&ingestion);
            Box::pin(async move { ingestion.process_claim(claim).await })
        });
        start_supervisor(self.queue.clone(), concurrency, retention, process)
    }
    pub async fn start(
        &self,
        deal_id: String,
        user_id: i64,
        file: UploadedDocument,
    ) -> ServiceResult<StartedDocumentJob> {
        let (job_id, filename) = self.accept_upload(deal_id, user_id, file).await?;
        self.ingestion.prepare_admission(&job_id, true).await?;
        Ok(StartedDocumentJob { job_id, filename })
    }
    pub(crate) async fn accept_upload(
        &self,
        deal: String,
        actor: i64,
        file: UploadedDocument,
    ) -> ServiceResult<(String, String)> {
        self.queue
            .admit(deal, actor, file, false)
            .await
            .map_err(Into::into)
    }
    pub(crate) async fn prepare_admission(&self, id: &str) -> ServiceResult<()> {
        self.ingestion.prepare_admission(id, true).await
    }
    pub(crate) async fn admission_event(
        &self,
        id: &str,
        actor: i64,
    ) -> ServiceResult<super::jobs::AdmissionEvent> {
        self.authorize_job(id, actor).await?;
        let job = self.queue.job(id).await?;
        Ok(super::jobs::AdmissionEvent {
            job_id: id.into(),
            filename: job.filename,
            status: if job.prepared {
                AdmissionStatus::Accepted
            } else if job.status == "failed" {
                AdmissionStatus::Failed
            } else if job.stage == "classifying" {
                AdmissionStatus::Classifying
            } else {
                AdmissionStatus::Parsing
            },
        })
    }
    pub async fn authorize_job(&self, id: &str, actor: i64) -> ServiceResult<()> {
        let job = self.queue.job(id).await?;
        self.ingestion.authorize_read(&job.deal, actor).await
    }
    pub async fn subscribe_owned(
        &self,
        id: &str,
        actor: i64,
    ) -> ServiceResult<watch::Receiver<DocumentJobEvent>> {
        self.authorize_job(id, actor).await?;
        let job = self.queue.job(id).await?;
        let initial = event(id, &job);
        let (sender, receiver) = watch::channel(initial);
        let service = self.clone();
        let id = id.to_owned();
        tokio::spawn(async move {
            let mut sequence = job.sequence;
            while !sender.is_closed() {
                tokio::select! {_=sender.closed()=>break,_=tokio::time::sleep(Duration::from_millis(200))=>{}}
                if service.authorize_job(&id, actor).await.is_err() {
                    break;
                }
                let Ok(job) = service.queue.job(&id).await else {
                    break;
                };
                if job.sequence != sequence {
                    sequence = job.sequence;
                    let snapshot = event(&id, &job);
                    let terminal = snapshot.is_terminal();
                    sender.send_replace(snapshot);
                    if terminal {
                        break;
                    }
                }
            }
        });
        Ok(receiver)
    }
}
/// Runs background stages and commits exactly one prepared document.
pub(crate) type ProcessDocument =
    Arc<dyn Fn(Claimed) -> Pin<Box<dyn Future<Output = ServiceResult<()>> + Send>> + Send + Sync>;

pub(crate) fn start_supervisor(
    queue: DocumentJobRepository,
    concurrency: usize,
    retention: Duration,
    process: ProcessDocument,
) -> WorkerHandle {
    let supervisor = DocumentSupervisor {
        queue,
        limit: concurrency.clamp(1, MAX_BACKGROUND_DOCUMENTS),
        retention_seconds: u32::try_from(retention.as_secs()).unwrap_or(u32::MAX),
        process,
    };
    WorkerHandle::spawn(move |stop| supervisor.run(stop))
}

/// Claims background jobs only while a task slot is free; each owned task embeds and
/// commits exactly one document independently of its siblings.
struct DocumentSupervisor {
    queue: DocumentJobRepository,
    limit: usize,
    retention_seconds: u32,
    process: ProcessDocument,
}

impl DocumentSupervisor {
    async fn run(self, mut stop: StopSignal) {
        let mut active = JoinSet::new();
        let mut maintenance_due = tokio::time::Instant::now();
        loop {
            while let Some(outcome) = active.try_join_next() {
                observe_document_task(outcome);
            }
            if stop.is_requested() {
                break;
            }
            // Maintenance runs on its own cadence even while every slot is busy.
            if tokio::time::Instant::now() >= maintenance_due {
                if self.queue.prune(self.retention_seconds).await.is_err() {
                    tracing::warn!(category = "storage", "document retention unavailable");
                }
                maintenance_due = tokio::time::Instant::now() + MAINTENANCE_INTERVAL;
            }
            self.fill(&mut active, &stop).await;
            tokio::select! {
                outcome = active.join_next(), if !active.is_empty() => {
                    if let Some(outcome) = outcome {
                        observe_document_task(outcome);
                    }
                }
                _ = tokio::time::sleep(IDLE_POLL) => {}
                _ = stop.requested() => break,
            }
        }
        drain_owned(&mut active, DRAIN_GRACE).await;
    }

    /// Capacity is checked before every claim, so no backlog is preclaimed and no lease
    /// ages while waiting for a slot.
    async fn fill(&self, active: &mut JoinSet<ServiceResult<()>>, stop: &StopSignal) {
        while !stop.is_requested() && active.len() < self.limit {
            match self.queue.claim_ready().await {
                Ok(Some(claim)) => {
                    active.spawn((self.process)(claim));
                }
                Ok(None) => break,
                Err(_) => {
                    tracing::warn!(category = "storage", "document claim unavailable");
                    break;
                }
            }
        }
    }
}

/// One task's failure or panic never ends the supervisor or cancels siblings. A panicked
/// task cannot prove ownership, so its attempt is left for lease-expiry recovery.
fn observe_document_task(outcome: Result<ServiceResult<()>, tokio::task::JoinError>) {
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(_)) => tracing::debug!(category = "attempt_failed", "document task failed"),
        Err(error) if error.is_panic() => {
            tracing::error!(category = "worker", "document task panicked")
        }
        Err(_) => tracing::debug!(category = "worker", "document task aborted"),
    }
}

fn event(id: &str, job: &Job) -> DocumentJobEvent {
    if let Some(result) = &job.result {
        if result.skipped {
            return DocumentJobEvent::skipped(
                id.into(),
                job.filename.clone(),
                result.document_id.clone(),
            );
        }
        if result.success {
            return DocumentJobEvent::completed(
                id.into(),
                job.filename.clone(),
                result.document_id.clone(),
                result.chunk_count,
            );
        }
        return DocumentJobEvent::failed(
            id.into(),
            job.filename.clone(),
            result
                .error
                .clone()
                .unwrap_or_else(|| "Document processing failed. Retry the same file.".into()),
        );
    }
    if job.status == "failed" {
        return DocumentJobEvent::failed(
            id.into(),
            job.filename.clone(),
            "Document processing failed. Retry the same file.".into(),
        );
    }
    DocumentJobEvent::processing(id.into(), job.filename.clone())
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/ingestion/job_service_tests.rs"]
mod tests;
