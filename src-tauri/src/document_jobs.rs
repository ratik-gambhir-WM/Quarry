use std::{collections::HashMap, sync::Arc};

use serde::Serialize;
use tokio::sync::{Mutex, OwnedMutexGuard, OwnedSemaphorePermit, RwLock, Semaphore};

pub const DOCUMENT_JOB_EVENT: &str = "documents:job";
const DEFAULT_RETAINED_JOB_COUNT: usize = 200;
const MAX_CONCURRENT_DOCUMENT_JOBS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentJobStatus {
    Processing,
    Completed,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentJobEvent {
    pub job_id: String,
    pub filename: String,
    pub status: DocumentJobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DocumentJobEvent {
    pub fn processing(job_id: String, filename: String) -> Self {
        Self {
            job_id,
            filename,
            status: DocumentJobStatus::Processing,
            document_id: None,
            chunk_count: None,
            error: None,
        }
    }

    pub fn completed(
        job_id: String,
        filename: String,
        document_id: Option<String>,
        chunk_count: usize,
    ) -> Self {
        Self {
            job_id,
            filename,
            status: DocumentJobStatus::Completed,
            document_id,
            chunk_count: Some(chunk_count),
            error: None,
        }
    }

    pub fn skipped(job_id: String, filename: String, document_id: Option<String>) -> Self {
        Self {
            job_id,
            filename,
            status: DocumentJobStatus::Skipped,
            document_id,
            chunk_count: None,
            error: None,
        }
    }

    pub fn failed(job_id: String, filename: String, error: String) -> Self {
        Self {
            job_id,
            filename,
            status: DocumentJobStatus::Failed,
            document_id: None,
            chunk_count: None,
            error: Some(error),
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.status != DocumentJobStatus::Processing
    }
}

#[derive(Default)]
struct JobStore {
    jobs: HashMap<String, DocumentJobEvent>,
    insertion_order: Vec<String>,
}

pub struct DocumentJobManager {
    capacity: usize,
    jobs: RwLock<JobStore>,
    processing_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    processing_slots: Arc<Semaphore>,
}

impl Default for DocumentJobManager {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_RETAINED_JOB_COUNT)
    }
}

impl DocumentJobManager {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            jobs: RwLock::new(JobStore::default()),
            processing_locks: Mutex::new(HashMap::new()),
            processing_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_DOCUMENT_JOBS)),
        }
    }

    pub async fn insert(&self, event: DocumentJobEvent) {
        let mut store = self.jobs.write().await;
        if !store.jobs.contains_key(&event.job_id) {
            store.insertion_order.push(event.job_id.clone());
        }
        store.jobs.insert(event.job_id.clone(), event);
        while store.jobs.len() > self.capacity {
            let Some(index) = store.insertion_order.iter().position(|job_id| {
                store
                    .jobs
                    .get(job_id)
                    .is_some_and(DocumentJobEvent::is_terminal)
            }) else {
                break;
            };
            let job_id = store.insertion_order.remove(index);
            store.jobs.remove(&job_id);
        }
    }

    pub async fn update(&self, event: DocumentJobEvent) {
        self.insert(event).await;
    }

    pub async fn get(&self, job_id: &str) -> Option<DocumentJobEvent> {
        self.jobs.read().await.jobs.get(job_id).cloned()
    }

    pub async fn lock_document(&self, document_key: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.processing_locks.lock().await;
            if locks.len() >= 500 {
                locks.retain(|_, lock| Arc::strong_count(lock) > 1);
            }
            locks
                .entry(document_key.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        lock.lock_owned().await
    }

    pub async fn acquire_processing_slot(&self) -> OwnedSemaphorePermit {
        self.processing_slots
            .clone()
            .acquire_owned()
            .await
            .expect("document processing semaphore is never closed")
    }
}
