//! Race-free admission and retirement for an on-demand background coordinator.
use std::{future::Future, sync::Mutex, time::Duration};
use tokio::{sync::Notify, task::JoinHandle};

#[derive(Default)]
struct State {
    task: Option<JoinHandle<()>>,
    rescan: bool,
    stopping: bool,
}

#[derive(Default)]
pub(crate) struct OnDemandTask {
    state: Mutex<State>,
    wake: Notify,
}

impl OnDemandTask {
    pub(crate) fn notify(&self, work: impl Future<Output = ()> + Send + 'static) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.stopping {
            return;
        }
        state.rescan = true;
        if state.task.as_ref().is_some_and(|task| !task.is_finished()) {
            self.wake.notify_one();
        } else {
            state.task = Some(tokio::spawn(work));
        }
    }

    /// A notification after this point requires another scan before retirement.
    pub(crate) fn begin_scan(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.rescan = false;
    }

    /// Call only with no active jobs and after observing an empty durable queue.
    pub(crate) fn retire_if_idle(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.rescan && !state.stopping {
            return false;
        }
        // Clear under the notification lock, before returning. A concurrent enqueue
        // either requests another scan or starts a successor; no wake can be lost.
        state.task = None;
        true
    }

    pub(crate) fn request_stop(&self) {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .stopping = true;
        self.wake.notify_one();
    }

    pub(crate) fn take_task(&self) -> Option<JoinHandle<()>> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .task
            .take()
    }

    pub(crate) async fn wait(&self, retry: Duration) {
        tokio::select! {
            _ = self.wake.notified() => {},
            _ = tokio::time::sleep(retry) => {},
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/shared/on_demand_task_tests.rs"]
mod tests;
