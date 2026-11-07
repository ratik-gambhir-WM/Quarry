use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
struct Provider {
    calls: AtomicUsize,
    gate: tokio::sync::Notify,
    blocked: bool,
}
impl EmbeddingProvider for Provider {
    fn embed<'a>(&'a self, _query: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>> {
        async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.blocked {
                self.gate.notified().await;
            }
            Ok(vec![1.0, 0.0])
        }
        .boxed()
    }
}
#[test]
fn embedding_rejects_wrong_shape_and_nonfinite_narrowing() {
    for input in [
        vec![],
        vec![vec![]],
        vec![vec![1.0], vec![2.0]],
        vec![vec![f64::NAN]],
        vec![vec![f64::INFINITY]],
        vec![vec![f64::MAX]],
    ] {
        assert!(validate_vector(input, 1).is_err());
    }
    assert!(validate_vector(vec![vec![1.0]], 2).is_err());
    assert_eq!(validate_vector(vec![vec![0.5]], 1).unwrap(), vec![0.5]);
}
#[tokio::test]
async fn embedding_coalesces_misses_and_keeps_exact_query_identity() {
    let provider = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        gate: Default::default(),
        blocked: true,
    });
    let cache = QueryEmbeddingCache::new(provider.clone(), 1, Duration::from_secs(1)).unwrap();
    let a = cache.embed("C++ query");
    let b = cache.embed("C++ query");
    let release = async {
        while provider.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        provider.gate.notify_one();
    };
    let (a, b, _) = tokio::join!(a, b, release);
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    cache.embed("C++ query").await.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let c = cache.embed("c++ query");
    let release = async {
        while provider.calls.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
        provider.gate.notify_one();
    };
    let (c, _) = tokio::join!(c, release);
    c.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn embedding_last_waiter_cancellation_drops_flight_and_provider_permit() {
    let provider = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        gate: Default::default(),
        blocked: true,
    });
    let cache = QueryEmbeddingCache::new(provider.clone(), 1, Duration::from_secs(1)).unwrap();
    let mut work = Box::pin(cache.embed("private query"));
    assert!(futures_util::poll!(&mut work).is_pending());
    assert_eq!(cache.providers.available_permits(), 0);
    drop(work);
    assert_eq!(cache.providers.available_permits(), 1);
    assert!(cache
        .state
        .lock()
        .unwrap()
        .flights
        .values()
        .all(|f| f.upgrade().is_none()));
}
#[tokio::test]
async fn embedding_ttl_and_entry_budget_do_not_extend_on_hits() {
    let provider = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        gate: Default::default(),
        blocked: false,
    });
    let cache = QueryEmbeddingCache::new(provider.clone(), 1, Duration::from_secs(1)).unwrap();
    cache.embed("a").await.unwrap();
    let expiry = cache
        .state
        .lock()
        .unwrap()
        .entries
        .values()
        .next()
        .unwrap()
        .expires;
    cache.embed("a").await.unwrap();
    assert_eq!(
        cache
            .state
            .lock()
            .unwrap()
            .entries
            .values()
            .next()
            .unwrap()
            .expires,
        expiry
    );
    // Inject controlled time at the pruning boundary, without sleeps.
    prune(&mut cache.state.lock().unwrap(), expiry);
    cache.embed("a").await.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    for i in 0..MAX_ENTRIES + 10 {
        cache.embed(&format!("q{i}")).await.unwrap();
    }
    let state = cache.state.lock().unwrap();
    assert_eq!(state.entries.len(), MAX_ENTRIES);
    assert!(state.bytes <= MAX_BYTES);
}

#[tokio::test]
async fn embedding_flight_waiter_and_provider_bounds_and_partial_cancellation() {
    let provider = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        gate: Default::default(),
        blocked: true,
    });
    let cache = QueryEmbeddingCache::new(provider.clone(), 1, Duration::from_secs(1)).unwrap();
    let mut waiters = Vec::new();
    for _ in 0..MAX_WAITERS {
        let mut waiter = Box::pin(cache.embed("same"));
        assert!(futures_util::poll!(&mut waiter).is_pending());
        waiters.push(waiter);
    }
    assert!(cache.embed("same").await.is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    // Dropping one waiter preserves shared work for the others.
    waiters.pop();
    assert_eq!(cache.providers.available_permits(), 0);
    provider.gate.notify_one();
    waiters.pop().unwrap().await.unwrap();
    drop(waiters);
    assert_eq!(cache.providers.available_permits(), 1);
    let mut flights = Vec::new();
    for i in 0..MAX_FLIGHTS {
        let query = format!("q{i}");
        let cache = &cache;
        let mut work = Box::pin(async move { cache.embed(&query).await });
        assert!(futures_util::poll!(&mut work).is_pending());
        flights.push(work);
    }
    assert!(cache.embed("overflow").await.is_err());
    // Only one active provider, with the remaining unique misses waiting inside the fixed bound.
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    drop(flights);
    assert_eq!(cache.providers.available_permits(), 1);
}
struct FailingProvider(AtomicUsize);
impl EmbeddingProvider for FailingProvider {
    fn embed<'a>(&'a self, _: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>> {
        async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(ServiceError::unavailable("sanitized failure"))
        }
        .boxed()
    }
}
#[tokio::test]
async fn embedding_failure_is_not_cached_and_large_entries_respect_byte_bound() {
    let provider = Arc::new(FailingProvider(AtomicUsize::new(0)));
    let cache = QueryEmbeddingCache::new(provider.clone(), 1, Duration::from_secs(1)).unwrap();
    assert!(cache.embed("query").await.is_err());
    assert!(cache.embed("query").await.is_err());
    assert_eq!(provider.0.load(Ordering::SeqCst), 2);
    assert!(cache.state.lock().unwrap().entries.is_empty());
    struct Large;
    impl EmbeddingProvider for Large {
        fn embed<'a>(&'a self, _: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>> {
            async { Ok(vec![0.0; 1024 * 1024]) }.boxed()
        }
    }
    let cache = QueryEmbeddingCache::new(Arc::new(Large), 1, Duration::from_secs(1)).unwrap();
    for i in 0..6 {
        cache.embed(&i.to_string()).await.unwrap();
    }
    let state = cache.state.lock().unwrap();
    assert_eq!(state.entries.len(), 3);
    assert!(state.bytes <= MAX_BYTES);
}
