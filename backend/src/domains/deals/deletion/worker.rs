//! Request-triggered, bounded cleanup tasks over the durable deletion ledger.
use super::service::DealDeletionService;
use crate::shared::error::RepositoryError;
use std::{sync::Arc, time::Duration};
use tokio::task::JoinSet;

const MAX_CONCURRENT_DELETIONS: usize = 4;

pub(super) fn notify(service: &Arc<DealDeletionService>) {
    let worker = service.clone();
    service.task.notify(async move {
        loop {
            worker.task.begin_scan();
            if let Err(_error) = worker.tick().await {
                tracing::error!("deletion ledger scan failed");
            }
            match worker.repository.has_pending().await {
                Ok(false) => {
                    if worker.task.retire_if_idle() {
                        return;
                    }
                    continue;
                }
                Ok(true) => {} // Busy deals and delayed retries still need another scan.
                Err(_error) => tracing::error!("deletion ledger pending check failed"),
            }
            worker.task.wait(Duration::from_secs(1)).await;
        }
    });
}

impl DealDeletionService {
    pub(super) async fn tick(self: &Arc<Self>) -> Result<(), RepositoryError> {
        let mut after = String::new();
        loop {
            let operations = self.repository.pending(&after).await?;
            let Some(last) = operations.last() else {
                return Ok(());
            };
            after.clone_from(&last.deletion_id);
            let mut operations = operations.into_iter();
            let mut active = JoinSet::new();
            let mut failure = None;
            loop {
                while active.len() < MAX_CONCURRENT_DELETIONS {
                    let Some(op) = operations.next() else { break };
                    let service = self.clone();
                    active.spawn(async move {
                        if let Err(error) = service.advance(&op).await {
                            let blocked = super::repository::integrity_failure(&error);
                            tracing::error!(deletion_id=%op.deletion_id, blocked, "deal cleanup phase failed");
                            service.repository.failure(op.deletion_id, blocked).await?;
                        }
                        Ok(())
                    });
                }
                let Some(result) = active.join_next().await else {
                    break;
                };
                // Finish admitted siblings even when one task or ledger write fails.
                let result = result.unwrap_or_else(|error| {
                    Err(RepositoryError::BlockingWorker(error.to_string()))
                });
                if let Err(error) = result {
                    failure.get_or_insert(error);
                }
            }
            if let Some(error) = failure {
                return Err(error);
            }
        }
    }
}
