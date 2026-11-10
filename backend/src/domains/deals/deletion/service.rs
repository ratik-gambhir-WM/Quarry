use super::{
    lifecycle::DealLifecycle,
    model::{DealDeletion, DeletionPhase},
    repository::DeletionRepository,
};
use crate::{
    domains::documents::{
        graph::repository::DocumentGraphWriter, ingestion::job_service::DocumentJobService,
        viewing::service::StoredDocumentService,
    },
    shared::error::{RepositoryError, ServiceResult},
};
use std::{
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::sync::Notify;

pub(crate) struct DealDeletionService {
    repository: DeletionRepository,
    graph: DocumentGraphWriter,
    lifecycle: DealLifecycle,
    jobs: Arc<DocumentJobService>,
    previews: Arc<StoredDocumentService>,
    wake: Notify,
}
impl DealDeletionService {
    pub(crate) fn new(
        repository: DeletionRepository,
        graph: DocumentGraphWriter,
        lifecycle: DealLifecycle,
        jobs: Arc<DocumentJobService>,
        previews: Arc<StoredDocumentService>,
    ) -> Arc<Self> {
        Arc::new(Self {
            repository,
            graph,
            lifecycle,
            jobs,
            previews,
            wake: Notify::new(),
        })
    }
    pub(crate) async fn authorize(&self, deal: String, actor: i64) -> ServiceResult<()> {
        Ok(self.repository.preflight(deal, actor).await?)
    }
    pub(crate) async fn accept(&self, deal: String, actor: i64) -> ServiceResult<DealDeletion> {
        let receipt = self
            .repository
            .accept(deal, actor)
            .await
            .map_err(|error| match error {
                RepositoryError::Storage(_)
                | RepositoryError::StorageCause(_)
                | RepositoryError::BlockingWorker(_)
                | RepositoryError::Unavailable(_) => {
                    crate::shared::error::ServiceError::unavailable(
                        "deletion could not be durably registered",
                    )
                }
                other => other.into(),
            })?;
        self.wake.notify_one();
        Ok(receipt)
    }
    pub(crate) async fn status(&self, id: String, actor: i64) -> ServiceResult<DealDeletion> {
        Ok(self.repository.status(id, actor).await?)
    }
    pub(crate) fn start(self: &Arc<Self>) {
        let service = Arc::downgrade(self);
        tokio::spawn(async move {
            worker(service).await;
        });
    }
    async fn tick(&self) -> Result<(), RepositoryError> {
        for op in self.repository.pending().await? {
            if let Err(error) = self.advance(&op).await {
                let blocked = super::repository::integrity_failure(&error);
                tracing::error!(deletion_id=%op.deletion_id, blocked, "deal cleanup phase failed");
                self.repository.failure(op.deletion_id, blocked).await?;
            }
        }
        Ok(())
    }
    async fn advance(&self, op: &DealDeletion) -> Result<(), RepositoryError> {
        // Workers and request bodies hold admission for their actual task lifetime. This guard
        // is never held with a SQLite transaction while waiting for other tasks to finish.
        let _drained = self.lifecycle.drain(&op.deal_id).await;
        if matches!(
            op.phase,
            DeletionPhase::Draining | DeletionPhase::Fencing | DeletionPhase::Sqlite
        ) {
            self.repository
                .checkpoint(op.deletion_id.clone(), DeletionPhase::Fencing)
                .await?;
            self.graph.fence_deal(&op.deal_id).await?;
            self.repository.snapshot_and_purge(op.clone()).await?;
        } else {
            // Reconcile the fence after restart/ambiguous acknowledgement before any purge.
            self.graph.fence_deal(&op.deal_id).await?;
        }
        self.jobs.evict_deal(&op.deal_id).await;
        self.previews.evict_previews().await;
        let files = self.repository.files(op.deletion_id.clone()).await?;
        if !files.is_empty() {
            for file in files {
                if self.graph.purge_deal_file(&op.deal_id, &file).await? {
                    self.repository
                        .purged_file(op.deletion_id.clone(), file)
                        .await?;
                }
            }
            return Ok(());
        }
        self.repository
            .checkpoint(op.deletion_id.clone(), DeletionPhase::Verifying)
            .await?;
        self.graph.verify_deal_absent(&op.deal_id).await?;
        self.repository.complete(op.deletion_id.clone()).await
    }
}
async fn worker(service: Weak<DealDeletionService>) {
    loop {
        let Some(service) = service.upgrade() else {
            break;
        };
        if let Err(_error) = service.tick().await {
            tracing::error!("deletion ledger scan failed");
        }
        tokio::select! { _=service.wake.notified()=>{}, _=tokio::time::sleep(Duration::from_secs(1))=>{} }
        drop(service);
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/deals/deletion/service_tests.rs"]
mod tests;
