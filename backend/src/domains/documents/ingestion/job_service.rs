use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::sync::{watch, RwLock};
use uuid::Uuid;

use crate::{
    domains::documents::ingestion::{
        jobs::DocumentJobEvent,
        service::{DocumentIngestionService, UploadedDocument},
    },
    shared::error::{ServiceError, ServiceResult},
};

#[derive(Debug)]
pub struct StartedDocumentJob {
    pub job_id: String,
    pub filename: String,
}

type OwnedJobs = HashMap<String, (String, i64, watch::Sender<DocumentJobEvent>)>;

#[derive(Clone)]
pub struct DocumentJobService {
    ingestion: Arc<DocumentIngestionService>,
    jobs: Arc<RwLock<OwnedJobs>>,
    completed_retention: Duration,
}

impl DocumentJobService {
    pub fn new(ingestion: Arc<DocumentIngestionService>, completed_retention: Duration) -> Self {
        Self {
            ingestion,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            completed_retention,
        }
    }

    pub async fn start(
        &self,
        deal_id: String,
        user_id: i64,
        file: UploadedDocument,
    ) -> ServiceResult<StartedDocumentJob> {
        self.ingestion.authorize_upload(&deal_id, user_id).await?;
        let filename = file.filename.clone();
        let job_id = Uuid::new_v4().to_string();
        let (sender, _) = watch::channel(DocumentJobEvent::processing(
            job_id.clone(),
            filename.clone(),
        ));
        self.jobs
            .write()
            .await
            .insert(job_id.clone(), (deal_id.clone(), user_id, sender));

        let service = self.clone();
        let worker_job_id = job_id.clone();
        let worker_filename = filename.clone();
        tokio::spawn(async move {
            let event = match service
                .ingestion
                .process(&deal_id, user_id, vec![file])
                .await
            {
                Ok(response) => match response.documents.into_iter().next() {
                    Some(document) if document.skipped => DocumentJobEvent::skipped(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        document.document_id,
                    ),
                    Some(document) if document.success => DocumentJobEvent::completed(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        document.document_id,
                        document.chunk_count,
                    ),
                    Some(document) => DocumentJobEvent::failed(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        document.error.unwrap_or_else(|| {
                            "Document processing failed. Refresh documents and retry.".into()
                        }),
                    ),
                    None => DocumentJobEvent::failed(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        "document processing returned no result".to_string(),
                    ),
                },
                Err(_error) => DocumentJobEvent::failed(
                    worker_job_id.clone(),
                    worker_filename.clone(),
                    "Document processing failed. Refresh documents and retry.".into(),
                ),
            };
            if let Some((_, _, sender)) = service.jobs.read().await.get(&worker_job_id) {
                sender.send_replace(event);
            }
            tokio::time::sleep(service.completed_retention).await;
            service.jobs.write().await.remove(&worker_job_id);
        });

        Ok(StartedDocumentJob { job_id, filename })
    }

    pub async fn subscribe_owned(
        &self,
        job_id: &str,
        actor: i64,
    ) -> ServiceResult<watch::Receiver<DocumentJobEvent>> {
        let (deal, _, sender) = self
            .jobs
            .read()
            .await
            .get(job_id)
            .cloned()
            .ok_or_else(|| ServiceError::not_found("document job not found"))?;
        self.ingestion.authorize_read(&deal, actor).await?;
        Ok(sender.subscribe())
    }
    pub async fn authorize_job(&self, job_id: &str, actor: i64) -> ServiceResult<()> {
        let deal = self
            .jobs
            .read()
            .await
            .get(job_id)
            .map(|(d, _, _)| d.clone())
            .ok_or_else(|| ServiceError::not_found("document job not found"))?;
        self.ingestion.authorize_read(&deal, actor).await
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/ingestion/job_service_tests.rs"]
mod tests;
