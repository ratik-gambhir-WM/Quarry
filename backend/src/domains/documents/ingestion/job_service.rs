//! Durable document workers and database-polled SSE snapshots.
use super::{
    jobs::DocumentJobEvent,
    repository::{DocumentJobRepository, Job},
    service::{DocumentIngestionService, UploadedDocument},
};
use crate::shared::error::ServiceResult;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
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
    pub fn new(ingestion: Arc<DocumentIngestionService>, retention: Duration) -> Self {
        let queue = ingestion.queue();
        let weak = Arc::downgrade(&ingestion);
        let worker = queue.clone();
        tokio::spawn(async move {
            let mut maintenance = tokio::time::Instant::now();
            loop {
                let Some(ingestion) = weak.upgrade() else {
                    break;
                };
                if tokio::time::Instant::now() >= maintenance {
                    let seconds = u32::try_from(retention.as_secs()).unwrap_or(u32::MAX);
                    if worker.prune(seconds).await.is_err() {
                        tracing::warn!(category = "storage", "document retention unavailable");
                    }
                    maintenance = tokio::time::Instant::now() + Duration::from_secs(60);
                }
                match worker.claim(None).await {
                    Ok(Some(claim)) => {
                        let _ = ingestion.process_claim(claim).await;
                    }
                    Ok(None) => {}
                    Err(_) => tracing::warn!(category = "storage", "document claim unavailable"),
                }
                drop(ingestion);
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });
        Self { ingestion, queue }
    }
    pub async fn start(
        &self,
        deal_id: String,
        user_id: i64,
        file: UploadedDocument,
    ) -> ServiceResult<StartedDocumentJob> {
        let (job_id, filename) = self.queue.admit(deal_id, user_id, file, true).await?;
        Ok(StartedDocumentJob { job_id, filename })
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
