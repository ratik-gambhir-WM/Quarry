//! Process-local task lifetime tracking; SQLite barriers and remote fences remain authoritative.
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};
use tokio::sync::{Mutex, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};
/// Clones retain the same admission, including across an uncancellable blocking task.
pub(crate) type DealAdmission = Arc<OwnedRwLockReadGuard<()>>;

#[derive(Clone, Default)]
pub(crate) struct DealLifecycle(Arc<Mutex<HashMap<String, Weak<RwLock<()>>>>>);
impl DealLifecycle {
    async fn lock(&self, deal: &str) -> Arc<RwLock<()>> {
        let mut locks = self.0.lock().await;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(deal).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(RwLock::new(()));
        locks.insert(deal.into(), Arc::downgrade(&lock));
        lock
    }
    pub(crate) async fn admit(&self, deal: &str) -> DealAdmission {
        Arc::new(self.lock(deal).await.read_owned().await)
    }
    pub(super) async fn drain(&self, deal: &str) -> OwnedRwLockWriteGuard<()> {
        self.lock(deal).await.write_owned().await
    }
}
