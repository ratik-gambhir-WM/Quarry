use super::{model::*, pipeline::SavePipeline, repository::NotebookRepository, save_jobs};
use crate::shared::error::{ServiceError, ServiceResult};
use std::sync::Arc;

pub(crate) struct NotebookService {
    pub(super) repository: NotebookRepository,
    pub(super) pipeline: Arc<dyn SavePipeline>,
    pub(super) wake: tokio::sync::Notify,
    query_permits: tokio::sync::Semaphore,
}
impl NotebookService {
    pub(crate) fn new(
        repository: NotebookRepository,
        pipeline: Arc<super::NotebookPipeline>,
    ) -> Arc<Self> {
        Self::with_pipeline(repository, pipeline)
    }
    pub(super) fn with_pipeline(
        repository: NotebookRepository,
        pipeline: Arc<dyn SavePipeline>,
    ) -> Arc<Self> {
        let service = Arc::new(Self {
            repository,
            pipeline,
            wake: tokio::sync::Notify::new(),
            query_permits: tokio::sync::Semaphore::new(4),
        });
        save_jobs::start(&service);
        service
    }
    pub(super) async fn search(
        &self,
        owner: i64,
        notebook: String,
        input: SearchInput,
    ) -> ServiceResult<SearchResults> {
        let _permit = if input.mode != SearchMode::Keyword {
            Some(self.query_permits.try_acquire().map_err(|_| {
                ServiceError::unavailable("notebook query embedding capacity exhausted")
            })?)
        } else {
            None
        };
        let vector = if input.mode != SearchMode::Keyword {
            let inputs = [input.query_text.clone()];
            let mut vectors = tokio::time::timeout(
                std::time::Duration::from_secs(45),
                self.pipeline.embed(&inputs),
            )
            .await
            .map_err(|_| ServiceError::internal("notebook query embedding timed out"))?
            .map_err(|_| ServiceError::unavailable("notebook query embedding unavailable"))?;
            if vectors.len() != 1 {
                return Err(ServiceError::internal("invalid notebook query embedding"));
            }
            let vector = vectors.remove(0);
            if vector.as_slice().len() != self.pipeline.dimensions() {
                return Err(ServiceError::internal("invalid notebook query embedding"));
            }
            Some(vector)
        } else {
            None
        };
        Ok(self
            .repository
            .search(owner, notebook, self.pipeline.fingerprint(), input, vector)
            .await?)
    }
    pub(super) async fn create(
        &self,
        owner: i64,
        title: String,
        deal: Option<String>,
    ) -> ServiceResult<Notebook> {
        Ok(self.repository.create(owner, title, deal).await?)
    }
    pub(super) async fn get_notebook(&self, owner: i64, id: String) -> ServiceResult<Notebook> {
        Ok(self.repository.get(owner, id).await?)
    }
    pub(super) async fn get_notebooks(
        &self,
        owner: i64,
        options: ListOptions,
    ) -> ServiceResult<Page<Notebooks>> {
        Ok(self.repository.list(owner, options).await?)
    }
    pub(super) async fn update_notebook(
        &self,
        owner: i64,
        id: String,
        title: Option<String>,
        deal: PatchField<String>,
    ) -> ServiceResult<Notebook> {
        Ok(self.repository.update(owner, id, title, deal).await?)
    }
    pub(super) async fn delete_notebook(&self, owner: i64, id: String) -> ServiceResult<()> {
        self.repository.delete(owner, id).await?;
        self.wake.notify_one();
        Ok(())
    }
    pub(super) async fn create_folder(
        &self,
        owner: i64,
        notebook: String,
        name: String,
    ) -> ServiceResult<Folder> {
        Ok(self.repository.create_folder(owner, notebook, name).await?)
    }
    pub(super) async fn get_folder(
        &self,
        owner: i64,
        notebook: String,
        id: String,
    ) -> ServiceResult<Folder> {
        Ok(self.repository.get_folder(owner, notebook, id).await?)
    }
    pub(super) async fn list_folders(
        &self,
        owner: i64,
        notebook: String,
        options: ListOptions,
    ) -> ServiceResult<Page<Folders>> {
        Ok(self
            .repository
            .list_folders(owner, notebook, options)
            .await?)
    }
    pub(super) async fn update_folder(
        &self,
        owner: i64,
        notebook: String,
        id: String,
        name: String,
    ) -> ServiceResult<Folder> {
        Ok(self
            .repository
            .update_folder(owner, notebook, id, name)
            .await?)
    }
    pub(super) async fn delete_folder(
        &self,
        owner: i64,
        notebook: String,
        id: String,
    ) -> ServiceResult<()> {
        Ok(self.repository.delete_folder(owner, notebook, id).await?)
    }
    pub(super) async fn get_note(
        &self,
        owner: i64,
        notebook: String,
        id: String,
    ) -> ServiceResult<Note> {
        Ok(self.repository.get_note(owner, notebook, id).await?)
    }
    pub(super) async fn list_notes(
        &self,
        owner: i64,
        notebook: String,
        options: ListOptions,
    ) -> ServiceResult<Page<Notes>> {
        Ok(self.repository.list_notes(owner, notebook, options).await?)
    }
    pub(super) async fn delete_note(
        &self,
        owner: i64,
        notebook: String,
        id: String,
        expected_revision: i64,
    ) -> ServiceResult<()> {
        self.repository
            .delete_note(owner, notebook, id, expected_revision)
            .await?;
        self.wake.notify_one();
        Ok(())
    }
    pub(super) async fn save(
        &self,
        owner: i64,
        notebook: String,
        id: Option<String>,
        input: SaveInput,
    ) -> ServiceResult<SaveJob> {
        let job = self
            .repository
            .admit_with_pipeline(owner, notebook, id, input, self.pipeline.fingerprint())
            .await?;
        self.wake.notify_one();
        Ok(job)
    }
    pub(super) async fn job(
        &self,
        owner: i64,
        notebook: String,
        id: String,
    ) -> ServiceResult<SaveJob> {
        Ok(self.repository.job(owner, notebook, id).await?)
    }
    pub(super) async fn retry(
        &self,
        owner: i64,
        notebook: String,
        id: String,
    ) -> ServiceResult<SaveJob> {
        let job = self.repository.retry(owner, notebook, id).await?;
        self.wake.notify_one();
        Ok(job)
    }
}
pub(super) fn failure(error: crate::shared::error::RepositoryError) -> &'static str {
    match ServiceError::from(error) {
        ServiceError::NotFound(_) => "resource_not_found",
        ServiceError::Conflict(_) => "revision_conflict",
        _ => "storage_failed",
    }
}
