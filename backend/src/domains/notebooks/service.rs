use super::{graph::SavePipeline, model::*, repository::NotebookRepository, save_jobs};
use crate::shared::error::{ServiceError, ServiceResult};
use std::sync::Arc;

pub(crate) struct NotebookService {
    pub(super) repository: NotebookRepository,
    pub(super) pipeline: Arc<dyn SavePipeline>,
    pub(super) wake: tokio::sync::Notify,
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
        });
        save_jobs::start(&service);
        service
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
    ) -> ServiceResult<()> {
        self.repository.delete_note(owner, notebook, id).await?;
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
