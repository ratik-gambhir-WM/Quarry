use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::sync::{watch, RwLock};
use uuid::Uuid;

use crate::{
    document_jobs::DocumentJobEvent,
    services::{
        document_ingestion_service::{DocumentIngestionService, UploadedDocument},
        error::{ServiceError, ServiceResult},
    },
};

#[derive(Debug)]
pub struct StartedDocumentJob {
    pub job_id: String,
    pub filename: String,
}

#[derive(Clone)]
pub struct DocumentJobService {
    ingestion: Arc<DocumentIngestionService>,
    jobs: Arc<RwLock<HashMap<String, watch::Sender<DocumentJobEvent>>>>,
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
        user_id: String,
        file: UploadedDocument,
    ) -> ServiceResult<StartedDocumentJob> {
        let filename = file.filename.clone();
        let job_id = Uuid::new_v4().to_string();
        let (sender, _) = watch::channel(DocumentJobEvent::processing(
            job_id.clone(),
            filename.clone(),
        ));
        self.jobs.write().await.insert(job_id.clone(), sender);

        let service = self.clone();
        let worker_job_id = job_id.clone();
        let worker_filename = filename.clone();
        tokio::spawn(async move {
            let event = match service
                .ingestion
                .process(&deal_id, &user_id, vec![file])
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
                        document
                            .error
                            .unwrap_or_else(|| "document processing failed".to_string()),
                    ),
                    None => DocumentJobEvent::failed(
                        worker_job_id.clone(),
                        worker_filename.clone(),
                        "document processing returned no result".to_string(),
                    ),
                },
                Err(error) => DocumentJobEvent::failed(
                    worker_job_id.clone(),
                    worker_filename.clone(),
                    error.to_string(),
                ),
            };
            if let Some(sender) = service.jobs.read().await.get(&worker_job_id) {
                sender.send_replace(event);
            }
            tokio::time::sleep(service.completed_retention).await;
            service.jobs.write().await.remove(&worker_job_id);
        });

        Ok(StartedDocumentJob { job_id, filename })
    }

    pub async fn subscribe(
        &self,
        job_id: &str,
    ) -> ServiceResult<watch::Receiver<DocumentJobEvent>> {
        self.jobs
            .read()
            .await
            .get(job_id)
            .map(watch::Sender::subscribe)
            .ok_or_else(|| {
                ServiceError::not_found(format!("document job `{job_id}` was not found"))
            })
    }
}
