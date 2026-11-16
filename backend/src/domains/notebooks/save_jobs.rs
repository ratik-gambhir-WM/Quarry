//! Bounded durable workers: preparation is private; native content and search publish together.
use super::{
    markdown,
    model::ClaimedJob,
    search_repository::PreparedNote,
    service::{failure, NotebookService},
};
use std::{sync::Arc, time::Duration};

pub(super) fn start(service: &Arc<NotebookService>) {
    // Independent notes can prepare concurrently. Durable slots bound admission separately.
    for _ in 0..4 {
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
                match service.repository.claim().await {
                    Ok(Some(claim)) => {
                        let result = tokio::time::timeout(
                            Duration::from_secs(120),
                            attempt(&service, &claim),
                        )
                        .await
                        .unwrap_or(Err("worker_interrupted"));
                        if let Err(code) = result {
                            if service.repository.fail(&claim, code).await.is_err() {
                                tracing::warn!(
                                    category = "storage",
                                    "notebook failure checkpoint unavailable"
                                );
                            }
                        }
                        continue;
                    }
                    Ok(None) => {}
                    Err(_) => tracing::warn!(category = "storage", "notebook job claim failed"),
                }
                let wake = service.wake.notified();
                tokio::select! { _=wake=>{}, _=tokio::time::sleep(Duration::from_secs(1))=>{} }
                drop(service);
            }
        });
    }
}
async fn attempt(service: &NotebookService, claim: &ClaimedJob) -> Result<(), &'static str> {
    if claim.input.pipeline_version != service.pipeline.fingerprint() {
        return Err("pipeline_changed");
    }
    let reuse = service
        .repository
        .reusable(&claim.input.note, &claim.input.pipeline_version)
        .await
        .map_err(failure)?;
    let prepared = if reuse.is_some() {
        None
    } else {
        // spawn_blocking needs an owned native snapshot; no SQL transaction spans export/provider work.
        let note = claim.input.note.clone();
        let (markdown, chunks) = tokio::task::spawn_blocking(move || {
            let markdown = markdown::export(&note)?;
            let chunks = markdown::chunks(&markdown)?;
            Ok::<_, &'static str>((markdown, chunks))
        })
        .await
        .map_err(|_| "worker_interrupted")??;
        service
            .repository
            .checkpoint(claim, "chunking")
            .await
            .map_err(failure)?;
        service
            .repository
            .checkpoint(claim, "embedding")
            .await
            .map_err(failure)?;
        let vectors = if chunks.is_empty() {
            Vec::new()
        } else {
            service.pipeline.embed(&chunks).await?
        };
        Some(
            PreparedNote::new(
                &claim.input.note,
                claim.input.pipeline_version.clone(),
                service.pipeline.dimensions(),
                markdown,
                chunks,
                vectors,
            )
            .map_err(|_| "embedding_failed")?,
        )
    };
    service
        .repository
        .checkpoint(claim, "persisting")
        .await
        .map_err(failure)?;
    service
        .repository
        .persist(claim, prepared, reuse)
        .await
        .map_err(failure)
}
#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/save_jobs_tests.rs"]
mod tests;
