//! Owned lifecycle for long-running background supervisors.
//!
//! A [`WorkerHandle`] is created once by bootstrap. Clones share one stop signal and one
//! supervisor task. Dropping every handle detaches the supervisor rather than stopping it,
//! so tests and router-only assemblies keep the previous fire-and-forget behavior.
use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    sync::watch,
    task::{JoinHandle, JoinSet},
};

/// Bounded time owned tasks may take to finish after a stop request.
pub(crate) const DRAIN_GRACE: Duration = Duration::from_secs(5);

/// Receiver side of a supervisor stop request.
#[derive(Clone)]
pub(crate) struct StopSignal(watch::Receiver<bool>);

impl StopSignal {
    pub(crate) fn is_requested(&self) -> bool {
        *self.0.borrow()
    }

    /// Resolves once a stop is requested. A detached supervisor (all handles dropped)
    /// never observes a stop and keeps running until the runtime ends.
    pub(crate) async fn requested(&mut self) {
        if self.0.wait_for(|stop| *stop).await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

#[derive(Clone)]
pub struct WorkerHandle {
    stop: Arc<watch::Sender<bool>>,
    task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl WorkerHandle {
    pub(crate) fn spawn<F, Fut>(supervisor: F) -> Self
    where
        F: FnOnce(StopSignal) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (stop, receiver) = watch::channel(false);
        let task = tokio::spawn(supervisor(StopSignal(receiver)));
        Self {
            stop: Arc::new(stop),
            task: Arc::new(Mutex::new(Some(task))),
        }
    }

    /// Stop new claims. Already-owned tasks drain within [`DRAIN_GRACE`].
    pub fn request_stop(&self) {
        self.stop.send_replace(true);
    }

    /// Request a stop and wait for the supervisor and its owned tasks to finish.
    pub async fn stop(&self) {
        self.request_stop();
        let task = self
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(task) = task {
            if task.await.is_err() {
                tracing::warn!(
                    category = "worker",
                    "background supervisor ended abnormally"
                );
            }
        }
    }
}

/// Give owned tasks a bounded drain, then abort and join the rest. Aborted work is never
/// reported as successful; its durable lease makes it recoverable after expiry.
pub(crate) async fn drain_owned<T: 'static>(active: &mut JoinSet<T>, grace: Duration) {
    let drained =
        tokio::time::timeout(grace, async { while active.join_next().await.is_some() {} }).await;
    if drained.is_err() {
        active.abort_all();
        while active.join_next().await.is_some() {}
    }
}

#[cfg(test)]
#[path = "../../tests/unit/shared/worker_tests.rs"]
mod tests;
