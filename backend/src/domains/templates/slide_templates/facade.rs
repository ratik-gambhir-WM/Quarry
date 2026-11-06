//! Bounded asynchronous entrypoint shared by the native routes and Quarry facade.
use super::{error::FailureKind, *};
use serde_json::Value;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex as StdMutex, Weak,
};
use tokio::sync::{Mutex, Semaphore};

struct CancelOnDrop(RequestCancellation);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[derive(Clone)]
pub struct SlideTemplates {
    pub(super) import: Arc<ImportService>,
    export: Arc<ExportService>,
    worker: Option<Arc<SlideClassificationWorker>>,
    capacity: Arc<Semaphore>,
    admission: Arc<Semaphore>,
    mutations: Arc<Mutex<()>>,
    projection: Option<Arc<super::graph::TemplateGraph>>,
    pub(super) allow_embeddings: bool,
    stopping: Arc<AtomicBool>,
    active: Arc<StdMutex<Vec<Weak<AtomicBool>>>>,
}
impl SlideTemplates {
    /// Scope request operations while retaining the global worker and concurrency budgets.
    pub(crate) fn for_user(&self, user_id: i64) -> SlideResult<Self> {
        Ok(Self {
            import: Arc::new(self.import.for_user(user_id)?),
            export: Arc::new(self.export.for_user(user_id)?),
            ..self.clone()
        })
    }

    pub fn new(
        repository: Arc<dyn TemplateRepository>,
        preview: Arc<dyn TemplatePreviewGenerator>,
        classification: Option<Arc<SlideClassificationService>>,
        projection: Option<Arc<super::graph::TemplateGraph>>,
        allow_embeddings: bool,
    ) -> Self {
        let import = Arc::new(ImportService::with_preview_generator(
            repository.clone(),
            preview,
        ));
        let mutations = Arc::new(Mutex::new(()));
        let worker = classification.map(|service| {
            Arc::new(SlideClassificationWorker::new(
                SlideClassificationWorkerOptions {
                    classification_max_attempts: 3,
                    classification_timeout: std::time::Duration::from_secs(150),
                    concurrency: super::parallel::SLIDE_CONCURRENCY,
                    embedding_max_attempts: 3,
                    embedding_timeout: std::time::Duration::from_secs(150),
                    lease_padding: std::time::Duration::from_secs(30),
                    retry_base: std::time::Duration::from_secs(5),
                    now: Arc::new(chrono::Utc::now),
                    random: Arc::new(|| f64::from(uuid::Uuid::new_v4().as_bytes()[0]) / 255.0),
                    repository: repository.clone(),
                    service,
                    import: import.clone(),
                    projection: projection.clone(),
                    mutations: mutations.clone(),
                },
            ))
        });
        Self {
            import,
            export: Arc::new(ExportService::new(repository)),
            worker,
            capacity: Arc::new(Semaphore::new(2)),
            admission: Arc::new(Semaphore::new(4)),
            mutations,
            projection,
            allow_embeddings,
            stopping: Arc::new(AtomicBool::new(false)),
            active: Arc::new(StdMutex::new(Vec::new())),
        }
    }
    /// Resume durable pending jobs, including expired leases from a prior process.
    pub fn start_worker(&self) {
        if let Some(worker) = &self.worker {
            worker.start();
        }
    }
    /// Drain classification work after stopping HTTP admission.
    pub async fn stop_worker(&self) {
        if let Some(worker) = &self.worker {
            worker.stop(std::time::Duration::from_secs(5)).await;
        }
    }

    pub fn try_admit(&self) -> SlideResult<tokio::sync::OwnedSemaphorePermit> {
        if self.stopping.load(Ordering::Acquire) {
            return Err(unavailable(
                "template_stopping",
                "Template processing is shutting down.",
            ));
        }
        self.admission.clone().try_acquire_owned().map_err(|_| {
            unavailable(
                "template_busy",
                "Template requests are busy. Try again shortly.",
            )
        })
    }
    pub async fn parse_json(&self, bytes: Vec<u8>) -> SlideResult<Value> {
        self.run(move |_, _, _| {
            serde_json::from_slice(&bytes).map_err(|_| {
                SlideTemplateError::new(
                    FailureKind::Invalid,
                    "invalid_json",
                    "The presentation JSON is invalid.",
                )
            })
        })
        .await
    }
    pub(super) async fn encode_json<T: serde::Serialize + Send + 'static>(
        &self,
        make: impl FnOnce() -> SlideResult<T> + Send + 'static,
    ) -> SlideResult<Vec<u8>> {
        self.run(move |_, _, _| {
            use std::io::Write;
            struct Bounded(Vec<u8>);
            impl Write for Bounded {
                fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                    if self.0.len().saturating_add(bytes.len())
                        > super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES
                    {
                        return Err(std::io::Error::other("template response limit"));
                    }
                    self.0.extend_from_slice(bytes);
                    Ok(bytes.len())
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }
            let value = make()?;
            let mut output = Bounded(Vec::new());
            serde_json::to_writer(&mut output, &value).map_err(|_| {
                SlideTemplateError::new(
                    FailureKind::TooLarge,
                    "response_too_large",
                    "The template response exceeds the processing limit.",
                )
            })?;
            Ok(output.0)
        })
        .await
    }
    /// Cancel owned blocking/browser work before the host drains HTTP requests.
    pub fn shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(worker) = &self.worker {
            worker.request_stop();
        }
        if let Ok(active) = self.active.lock() {
            for flag in active.iter().filter_map(Weak::upgrade) {
                flag.store(true, Ordering::Release);
            }
        }
    }
    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce(Arc<ImportService>, Arc<ExportService>, RequestCancellation) -> SlideResult<T>
            + Send
            + 'static,
    ) -> SlideResult<T> {
        let cancellation = RequestCancellation::default();
        let _guard = CancelOnDrop(cancellation.clone());
        self.run_cancellable(cancellation, work).await
    }
    async fn run_cancellable<T: Send + 'static>(
        &self,
        cancellation: RequestCancellation,
        work: impl FnOnce(Arc<ImportService>, Arc<ExportService>, RequestCancellation) -> SlideResult<T>
            + Send
            + 'static,
    ) -> SlideResult<T> {
        if self.stopping.load(Ordering::Acquire) {
            return Err(unavailable(
                "template_stopping",
                "Template processing is shutting down.",
            ));
        }
        {
            let mut active = self.active.lock().map_err(|_| {
                unavailable("template_stopping", "Template processing is unavailable.")
            })?;
            active.retain(|flag| flag.strong_count() > 0);
            active.push(Arc::downgrade(&cancellation.0));
            if self.stopping.load(Ordering::Acquire) {
                cancellation.cancel();
            }
        }
        let permit = self.capacity.clone().try_acquire_owned().map_err(|_| {
            unavailable(
                "template_busy",
                "Template processing is busy. Try again shortly.",
            )
        })?;
        let import = self.import.clone();
        let export = self.export.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if cancellation.is_cancelled() {
                return Err(unavailable(
                    "request_cancelled",
                    "Template processing was cancelled.",
                ));
            }
            work(import, export, cancellation)
        })
        .await
        .map_err(SlideTemplateError::internal)?
    }
    pub async fn get(&self, id: String) -> SlideResult<Option<Value>> {
        self.run(move |i, _, _| {
            let value = i.find_json(&id)?;
            if let Some(value) = &value {
                bound_json(value)?;
            }
            Ok(value)
        })
        .await
    }
    pub async fn previews(&self, page: usize) -> SlideResult<TemplatePreviewListResponse> {
        self.run(move |i, _, _| i.list_previews(page)).await
    }
    pub async fn catalog(&self, kind: Option<TemplateKind>) -> SlideResult<TemplateListResponse> {
        self.run(move |i, _, _| i.list(kind)).await
    }
    pub async fn preview(&self, id: String) -> SlideResult<Option<StoredTemplatePreview>> {
        self.run(move |i, _, _| i.find_preview(&id)).await
    }
    pub async fn asset(&self, id: String, asset: String) -> SlideResult<Option<TemplateAsset>> {
        self.run(move |i, _, _| i.find_asset(&id, &asset)).await
    }
    pub async fn inspect(
        &self,
        id: String,
        metadata: bool,
        embeddings: bool,
    ) -> SlideResult<Option<Value>> {
        if embeddings && !self.allow_embeddings {
            return Err(unavailable(
                "embedding_inspection_disabled",
                "Embedding inspection is disabled.",
            ));
        }
        self.run(move |i, _, _| i.find_v2(&id, metadata, embeddings))
            .await
    }
    pub async fn retry_processing(&self, id: String) -> SlideResult<RetrySlideProcessingResponse> {
        let worker = self.worker.as_ref().ok_or_else(|| {
            unavailable(
                "slide_classification_unavailable",
                "Slide classification is not configured.",
            )
        })?;
        let _mutation = self.mutations.clone().try_lock_owned().map_err(|_| {
            unavailable(
                "template_busy",
                "Template writes are busy. Try again shortly.",
            )
        })?;
        let result = self
            .run(move |import, _, _| import.retry_failed_processing(&id))
            .await?;
        // Durable pending state is committed before waking the existing bounded worker.
        worker.start();
        Ok(result)
    }
    pub async fn export(&self, input: Value) -> SlideResult<ExportedPowerPoint> {
        self.run(move |_, e, _| e.export(input)).await
    }
    pub async fn insert(&self, input: InsertPowerPointInput) -> SlideResult<ExportedPowerPoint> {
        self.run(move |_, e, _| e.insert(input)).await
    }
    pub async fn import(&self, bytes: Vec<u8>, kind: TemplateKind) -> SlideResult<ImportResponse> {
        let _mutation = self.mutations.clone().try_lock_owned().map_err(|_| {
            unavailable(
                "template_busy",
                "Template writes are busy. Try again shortly.",
            )
        })?;
        let result = self
            .run(move |i, _, c| i.import_cancellable(&bytes, kind, &c))
            .await?;
        self.project(&result.template_id).await?;
        Ok(result)
    }
    pub async fn batch(
        &self,
        bytes: Vec<u8>,
        kind: TemplateKind,
    ) -> SlideResult<BatchImportResponse> {
        let _mutation = self.mutations.clone().try_lock_owned().map_err(|_| {
            unavailable(
                "template_busy",
                "Template writes are busy. Try again shortly.",
            )
        })?;
        let result = self
            .run(move |i, _, c| i.batch_import_cancellable(&bytes, kind, &c))
            .await?;
        for template in &result.templates {
            self.project(&template.template_id).await?;
        }
        Ok(result)
    }
    pub async fn delete(&self, id: String) -> SlideResult<bool> {
        let _mutation = self.mutations.clone().try_lock_owned().map_err(|_| {
            unavailable(
                "template_busy",
                "Template writes are busy. Try again shortly.",
            )
        })?;
        let key = id.clone();
        let removed = self.run(move |i, _, _| i.delete(&key)).await?;
        // An inaccessible ID must never delete another owner's graph projection.
        if removed {
            if let Some(index) = &self.projection {
                index.delete(&id).await.map_err(|_| projection_failure())?;
            }
        }
        Ok(removed)
    }
    pub async fn classified(
        &self,
        bytes: Vec<u8>,
        kind: TemplateKind,
    ) -> SlideResult<ImportResponse> {
        let worker = self.worker.as_ref().ok_or_else(|| {
            unavailable(
                "slide_classification_unavailable",
                "Slide classification is not configured.",
            )
        })?;
        let _mutation = self.mutations.clone().try_lock_owned().map_err(|_| {
            unavailable(
                "template_busy",
                "Template writes are busy. Try again shortly.",
            )
        })?;
        let result = self
            .run(move |i, _, c| i.queue_classified_import(&bytes, kind, &c))
            .await?;
        // The transaction is committed. The worker reads the saved preview/JSON independently
        // of the HTTP request lifetime; no provider or graph call delays this acknowledgement.
        worker.start();
        Ok(result)
    }
    pub async fn batch_classified(
        &self,
        bytes: Vec<u8>,
        kind: TemplateKind,
    ) -> SlideResult<BatchImportResponse> {
        let worker = self.worker.as_ref().ok_or_else(|| {
            unavailable(
                "slide_classification_unavailable",
                "Slide classification is not configured.",
            )
        })?;
        let _mutation = self.mutations.clone().try_lock_owned().map_err(|_| {
            unavailable(
                "template_busy",
                "Template writes are busy. Try again shortly.",
            )
        })?;
        let result = self
            .run(move |import, _, cancellation| {
                import.queue_classified_batch_import(&bytes, kind, &cancellation)
            })
            .await?;
        // Durable jobs survive the request and are also resumed by the startup worker.
        worker.start();
        Ok(result)
    }
    pub async fn reconcile(&self) -> SlideResult<()> {
        if let Some(index) = &self.projection {
            // Reconcile only Quarry's template projection from canonical shared SQLite records.
            let catalog = self.catalog(None).await?;
            let ids = catalog
                .templates
                .iter()
                .map(|t| t.template_id.clone())
                .collect::<Vec<_>>();
            index
                .reconcile(&ids)
                .await
                .map_err(|_| projection_failure())?;
            for id in ids {
                self.project(&id).await?;
            }
        }
        Ok(())
    }
    async fn project(&self, id: &str) -> SlideResult<()> {
        if let Some(index) = &self.projection {
            let id = id.to_owned();
            let record = self.run(move |i, _, _| i.projection_record(&id)).await?;
            if let Some(record) = record {
                index
                    .upsert(record)
                    .await
                    .map_err(|_| projection_failure())?;
            }
        }
        Ok(())
    }
}
fn projection_failure() -> SlideTemplateError {
    unavailable("template_projection_failed","The template change was saved in SQLite, but Helix indexing could not be confirmed. Refresh before retrying.")
}
pub(super) fn unavailable(code: &'static str, message: &'static str) -> SlideTemplateError {
    SlideTemplateError::new(FailureKind::Unavailable, code, message)
}
pub(super) fn json_size(value: &Value) -> SlideResult<usize> {
    use std::io::Write;
    struct Counter(usize);
    impl Write for Counter {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(b.len());
            if self.0 > super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES {
                return Err(std::io::Error::other("template document limit"));
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value)
        .map(|_| counter.0)
        .map_err(|e| {
            SlideTemplateError::new(
                FailureKind::TooLarge,
                "template_too_large",
                "The template exceeds the document limit.",
            )
            .with_source(e)
        })
}

pub(super) fn bound_json(value: &Value) -> SlideResult<()> {
    json_size(value).map(|_| ())
}
