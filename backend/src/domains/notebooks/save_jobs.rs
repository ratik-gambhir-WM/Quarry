//! One bounded worker per application; durable leases allow restart recovery without client replay.
use super::{
    markdown,
    model::ClaimedJob,
    service::{failure, NotebookService},
};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};

pub(super) fn start(service: &Arc<NotebookService>) {
    let weak = Arc::downgrade(service);
    tokio::spawn(async move {
        let mut maintenance = tokio::time::Instant::now();
        loop {
            let Some(service) = weak.upgrade() else {
                break;
            };
            if maintenance <= tokio::time::Instant::now() {
                if service.repository.prune().await.is_err() {
                    tracing::warn!(category = "storage", "notebook job retention failed");
                }
                maintenance = tokio::time::Instant::now() + Duration::from_secs(3600);
            }
            let mut worked = false;
            match service.repository.claim().await {
                Ok(Some(claim)) => {
                    worked = true;
                    let result =
                        tokio::time::timeout(Duration::from_secs(120), attempt(&service, &claim))
                            .await
                            .unwrap_or(Err("worker_interrupted"));
                    if let Err(code) = result {
                        if service.repository.fail(&claim, code).await.is_err() {
                            tracing::warn!(job_id=%claim.job.job_id,category="storage", "notebook failure checkpoint unavailable");
                        }
                    }
                }
                Ok(None) => {}
                Err(_) => tracing::warn!(category = "storage", "notebook job claim failed"),
            }
            // Cleanup is independent of parent/job cascades and retries without deleting tombstones.
            if let Ok(batch) = service.repository.cleanup_batch().await {
                for (note, owner) in batch {
                    if matches!(
                        tokio::time::timeout(
                            Duration::from_secs(15),
                            service.pipeline.delete(owner, note.clone())
                        )
                        .await,
                        Ok(Ok(()))
                    ) {
                        let _ = service.repository.cleaned(note).await;
                    } else {
                        let _ = service.repository.cleanup_failed(note).await;
                    }
                }
            }
            if worked {
                continue;
            }
            // Do not retain the service across idle waits: dropping the application stops its worker.
            let notified = service.wake.notified();
            tokio::select! { _ = notified => {}, _ = tokio::time::sleep(Duration::from_secs(1)) => {} }
            drop(service);
        }
    });
}
async fn attempt(service: &NotebookService, claim: &ClaimedJob) -> Result<(), &'static str> {
    if claim.input.pipeline_version != service.pipeline.fingerprint() {
        return Err("pipeline_changed");
    }
    service.repository.persist(claim).await.map_err(failure)?;
    service
        .repository
        .checkpoint(claim, "exporting", None, false)
        .await
        .map_err(failure)?;
    let note = claim.input.note.clone();
    let (markdown, chunks) = tokio::task::spawn_blocking(move || {
        let markdown = markdown::export(&note)?;
        let chunks = markdown::chunks(&markdown)?;
        Ok::<_, &'static str>((markdown, chunks))
    })
    .await
    .map_err(|_| "worker_interrupted")??;
    let fingerprint = service.pipeline.fingerprint();
    let hash = format!("{:x}", Sha256::digest(format!("{fingerprint}\n{markdown}")));
    service
        .repository
        .checkpoint(claim, "chunking", Some(hash.clone()), false)
        .await
        .map_err(failure)?;
    if !service
        .pipeline
        .published(claim.owner, &claim.input.note, &hash)
        .await?
    {
        service
            .repository
            .checkpoint(claim, "embedding", Some(hash.clone()), false)
            .await
            .map_err(failure)?;
        let vectors = service.pipeline.embed(&chunks).await?;
        service
            .repository
            .checkpoint(claim, "indexing", Some(hash.clone()), false)
            .await
            .map_err(failure)?;
        service
            .pipeline
            .publish(claim.owner, &claim.input.note, &hash, &chunks, vectors)
            .await?;
    }
    service
        .repository
        .checkpoint(claim, "indexing", Some(hash), true)
        .await
        .map_err(failure)
}
#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/save_jobs_tests.rs"]
mod tests;
