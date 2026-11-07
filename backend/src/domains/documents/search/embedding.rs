//! Request-polled embedding coalescing. The registry holds weak references only: dropping the
//! last waiter drops provider work, including semaphore waits. No background tasks are spawned.
use crate::{
    adapters::openai::client::OpenAiClient,
    shared::error::{ServiceError, ServiceResult},
};
use futures_util::{
    future::{BoxFuture, Shared},
    FutureExt,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::{
    sync::Semaphore,
    time::{timeout, Instant},
};

type Vector = Arc<Vec<f32>>;
type Work = Shared<BoxFuture<'static, Result<Vector, Arc<ServiceError>>>>;
type Key = [u8; 32];
const TTL: Duration = Duration::from_secs(300);
const MAX_ENTRIES: usize = 512;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_FLIGHTS: usize = 32;
const MAX_WAITERS: usize = 64;
const ENTRY_OVERHEAD: usize = 256;
// Reserve allocation slack for retained hash-table capacity and the bounded flight registry,
// including after eviction leaves only a few large vectors in a previously full table.
const TABLE_OVERHEAD: usize = MAX_ENTRIES * ENTRY_OVERHEAD;

pub(crate) trait EmbeddingProvider: Send + Sync {
    fn embed<'a>(&'a self, query: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>>;
}

pub(super) struct OpenAiQueryEmbedding {
    pub client: Arc<OpenAiClient>,
    pub model: String,
    pub dimensions: usize,
}
impl EmbeddingProvider for OpenAiQueryEmbedding {
    fn embed<'a>(&'a self, query: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>> {
        Box::pin(async move {
            let vectors = self
                .client
                .gen_private_embeddings(&[query], Some(&self.model))
                .await
                .map_err(|_| unavailable())?;
            validate_vector(vectors, self.dimensions)
        })
    }
}
fn validate_vector(mut vectors: Vec<Vec<f64>>, dimensions: usize) -> ServiceResult<Vec<f32>> {
    if vectors.len() != 1 || dimensions == 0 || vectors[0].len() != dimensions {
        return Err(unavailable());
    }
    vectors
        .pop()
        .expect("one vector checked")
        .into_iter()
        .map(|v| {
            let narrowed = v as f32;
            if v.is_finite() && narrowed.is_finite() {
                Ok(narrowed)
            } else {
                Err(unavailable())
            }
        })
        .collect()
}
fn unavailable() -> ServiceError {
    ServiceError::unavailable("semantic search unavailable; retry or use keyword search")
}

struct Entry {
    vector: Vector,
    expires: Instant,
    bytes: usize,
}
struct Flight {
    work: Work,
}
#[derive(Default)]
struct State {
    entries: HashMap<Key, Entry>,
    flights: HashMap<Key, Weak<Flight>>,
    bytes: usize,
}

pub struct QueryEmbeddingCache {
    salt: Key,
    namespace: String,
    provider: Arc<dyn EmbeddingProvider>,
    state: Arc<Mutex<State>>,
    providers: Arc<Semaphore>,
    deadline: Duration,
}
impl QueryEmbeddingCache {
    pub(crate) fn openai(
        client: Arc<OpenAiClient>,
        model: String,
        dimensions: usize,
        concurrency: usize,
        deadline: Duration,
    ) -> Result<Self, String> {
        let namespace = format!("openai/query-embedding/v1/{dimensions}/{model}");
        let mut cache = Self::new(
            Arc::new(OpenAiQueryEmbedding {
                client,
                model,
                dimensions,
            }),
            concurrency,
            deadline,
        )?;
        cache.namespace = namespace;
        Ok(cache)
    }
    pub(crate) fn new(
        provider: Arc<dyn EmbeddingProvider>,
        concurrency: usize,
        deadline: Duration,
    ) -> Result<Self, String> {
        let mut salt = [0; 32];
        getrandom::fill(&mut salt)
            .map_err(|_| "could not initialize private search cache".to_owned())?;
        Ok(Self {
            salt,
            namespace: "test/query-embedding/v1".into(),
            provider,
            state: Arc::new(Mutex::new(State::default())),
            providers: Arc::new(Semaphore::new(concurrency)),
            deadline,
        })
    }
    pub(super) async fn embed(&self, query: &str) -> ServiceResult<Vector> {
        let started = Instant::now();
        let mut digest = Sha256::new();
        digest.update(self.salt);
        digest.update(self.namespace.as_bytes());
        digest.update([0]);
        digest.update(query.as_bytes());
        let key: Key = digest.finalize().into();
        let flight = {
            let mut state = self.state.lock().map_err(|_| unavailable())?;
            prune(&mut state, Instant::now());
            if let Some(entry) = state.entries.get(&key) {
                tracing::info!(
                    phase = "embedding",
                    cache_hit = true,
                    elapsed_seconds = started.elapsed().as_secs_f64()
                );
                return Ok(entry.vector.clone());
            }
            state.flights.retain(|_, f| f.strong_count() > 0);
            if let Some(flight) = state.flights.get(&key).and_then(Weak::upgrade) {
                // This temporary reference is included in the count.
                if Arc::strong_count(&flight) > MAX_WAITERS {
                    return Err(unavailable());
                }
                flight
            } else {
                if state.flights.len() >= MAX_FLIGHTS {
                    return Err(unavailable());
                }
                let provider = self.provider.clone();
                let providers = self.providers.clone();
                let cache = Arc::downgrade(&self.state);
                let query = query.to_owned();
                let deadline = Instant::now() + self.deadline;
                let work = async move {
                    let vector = tokio::time::timeout_at(deadline, async {
                        let _permit = providers.acquire().await.map_err(|_| unavailable())?;
                        provider.embed(&query).await.map(Arc::new)
                    })
                    .await
                    .map_err(|_| unavailable())??;
                    if let Some(cache) = cache.upgrade() {
                        let mut state = cache.lock().map_err(|_| unavailable())?;
                        prune(&mut state, Instant::now());
                        let bytes = vector.capacity() * std::mem::size_of::<f32>() + ENTRY_OVERHEAD;
                        if bytes <= MAX_BYTES - TABLE_OVERHEAD {
                            while state.entries.len() >= MAX_ENTRIES
                                || state.bytes + bytes > MAX_BYTES - TABLE_OVERHEAD
                            {
                                let oldest = state
                                    .entries
                                    .iter()
                                    .min_by_key(|(_, e)| e.expires)
                                    .map(|(k, _)| *k);
                                if let Some(oldest) = oldest {
                                    remove(&mut state, &oldest);
                                } else {
                                    break;
                                }
                            }
                            state.bytes += bytes;
                            state.entries.insert(
                                key,
                                Entry {
                                    vector: vector.clone(),
                                    expires: Instant::now() + TTL,
                                    bytes,
                                },
                            );
                        }
                    }
                    Ok(vector)
                }
                .map(|result: ServiceResult<Vector>| result.map_err(Arc::new))
                .boxed()
                .shared();
                let flight = Arc::new(Flight { work });
                state.flights.insert(key, Arc::downgrade(&flight));
                flight
            }
        };
        let result = timeout(self.deadline, flight.work.clone())
            .await
            .map_err(|_| unavailable())?;
        // Retain the flight across await so cancellation releases the last weak registration.
        drop(flight);
        tracing::info!(
            phase = "embedding",
            cache_hit = false,
            success = result.is_ok(),
            elapsed_seconds = started.elapsed().as_secs_f64()
        );
        result.map_err(|_| unavailable())
    }
}
fn remove(state: &mut State, key: &Key) {
    if let Some(e) = state.entries.remove(key) {
        state.bytes -= e.bytes;
    }
}
fn prune(state: &mut State, now: Instant) {
    state.entries.retain(|_, e| e.expires > now);
    state.bytes = state.entries.values().map(|e| e.bytes).sum();
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/search/embedding_tests.rs"]
mod tests;
