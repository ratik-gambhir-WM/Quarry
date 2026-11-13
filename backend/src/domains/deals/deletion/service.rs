use super::{
    lifecycle::DealLifecycle,
    model::{DealDeletion, DeletionPhase},
    repository::DeletionRepository,
};
use crate::shared::on_demand_task::OnDemandTask;
use crate::{
    domains::documents::{
        graph::repository::DocumentGraphWriter, ingestion::job_service::DocumentJobService,
        viewing::service::StoredDocumentService,
    },
    shared::error::{RepositoryError, ServiceResult},
};
use std::sync::Arc;

pub(crate) struct DealDeletionService {
    pub(super) repository: DeletionRepository,
    graph: DocumentGraphWriter,
    lifecycle: DealLifecycle,
    jobs: Arc<DocumentJobService>,
    previews: Arc<StoredDocumentService>,
    pub(super) task: OnDemandTask,
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
            task: OnDemandTask::default(),
        })
    }
    pub(crate) async fn authorize(&self, deal: String, actor: i64) -> ServiceResult<()> {
        Ok(self.repository.preflight(deal, actor).await?)
    }
    pub(crate) async fn accept(
        self: &Arc<Self>,
        deal: String,
        actor: i64,
        expected_revision: i64,
    ) -> ServiceResult<DealDeletion> {
        // Once admission starts, request cancellation must not separate a committed
        // receipt from its notification. The task ends after acceptance, not cleanup.
        let service = self.clone();
        tokio::spawn(async move {
            service
                .accept_and_notify(deal, actor, expected_revision)
                .await
        })
        .await
        .map_err(|_| {
            crate::shared::error::ServiceError::unavailable("deletion acceptance interrupted")
        })?
    }
    async fn accept_and_notify(
        self: &Arc<Self>,
        deal: String,
        actor: i64,
        expected_revision: i64,
    ) -> ServiceResult<DealDeletion> {
        let receipt = self
            .repository
            .accept(deal, actor, expected_revision)
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
        if receipt.status != super::model::DeletionStatus::Completed {
            super::worker::notify(self);
        }
        Ok(receipt)
    }
    pub(crate) async fn status(&self, id: String, actor: i64) -> ServiceResult<DealDeletion> {
        Ok(self.repository.status(id, actor).await?)
    }
    /// Discover interrupted receipts once; the coordinator retires when work is exhausted.
    pub(crate) fn resume_pending(self: &Arc<Self>) {
        super::worker::notify(self);
    }
    pub(super) async fn advance(&self, op: &DealDeletion) -> Result<(), RepositoryError> {
        // Workers and request bodies hold admission for their actual task lifetime. This guard
        // is acquired only after those tasks finish; busy deals wait for the next scan.
        let Some(_drained) = self.lifecycle.try_drain(&op.deal_id).await else {
            return Ok(());
        };
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
#[cfg(test)]
#[path = "../../../../tests/unit/domains/deals/deletion/service_tests.rs"]
mod tests;
