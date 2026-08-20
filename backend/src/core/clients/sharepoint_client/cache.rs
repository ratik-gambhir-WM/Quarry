use std::{collections::HashMap, sync::Arc, time::Duration};

use serde_json::Value;
use tokio::sync::RwLock;

use super::types::{CacheAdapter, CacheFuture};

const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);

struct CacheEntry {
    value: Value,
    expires_at: tokio::time::Instant,
}

/// Default in-memory cache with TTL-based expiration.
#[derive(Clone, Default)]
pub struct InMemoryCache {
    store: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl CacheAdapter for InMemoryCache {
    fn get<'a>(&'a self, key: &'a str) -> CacheFuture<'a, Option<Value>> {
        Box::pin(async move {
            let mut store = self.store.write().await;
            let entry = store.get(key)?;
            if tokio::time::Instant::now() >= entry.expires_at {
                store.remove(key);
                return None;
            }
            Some(entry.value.clone())
        })
    }

    fn set<'a>(&'a self, key: &'a str, value: Value, ttl: Option<Duration>) -> CacheFuture<'a, ()> {
        Box::pin(async move {
            let expires_at = tokio::time::Instant::now() + ttl.unwrap_or(DEFAULT_CACHE_TTL);
            self.store
                .write()
                .await
                .insert(key.to_owned(), CacheEntry { value, expires_at });
        })
    }

    fn delete<'a>(&'a self, key: &'a str) -> CacheFuture<'a, ()> {
        Box::pin(async move {
            self.store.write().await.remove(key);
        })
    }

    fn clear(&self) -> CacheFuture<'_, ()> {
        Box::pin(async move {
            self.store.write().await.clear();
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/core/clients/sharepoint_client/cache_tests.rs"]
mod tests;
