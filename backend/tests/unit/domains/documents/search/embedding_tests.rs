use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[derive(Default)]
struct Provider {
    queries: Mutex<Vec<String>>,
    gate: Option<Semaphore>,
}
impl EmbeddingProvider for Provider {
    fn embed<'a>(&'a self, query: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>> {
        Box::pin(async move {
            self.queries.lock().unwrap().push(query.into());
            if let Some(gate) = &self.gate {
                gate.acquire().await.unwrap().forget();
            }
            Ok(vec![1.0, 0.0])
        })
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
async fn every_embedding_request_calls_the_provider_with_the_exact_query() {
    let provider = Arc::new(Provider::default());
    let embeddings = QueryEmbeddings::new(provider.clone(), 1, Duration::from_secs(1));
    for query in ["C++ query", "C++ query", "c++ query"] {
        assert_eq!(*embeddings.embed(query).await.unwrap(), vec![1.0, 0.0]);
    }
    assert_eq!(
        *provider.queries.lock().unwrap(),
        ["C++ query", "C++ query", "c++ query"]
    );
}

#[tokio::test]
async fn concurrent_identical_requests_are_independent_and_cancellation_releases_capacity() {
    let provider = Arc::new(Provider {
        gate: Some(Semaphore::new(0)),
        ..Default::default()
    });
    let embeddings = QueryEmbeddings::new(provider.clone(), 2, Duration::from_secs(1));
    let mut first = Box::pin(embeddings.embed("same query"));
    let mut second = Box::pin(embeddings.embed("same query"));
    let mut third = Box::pin(embeddings.embed("another query"));
    assert!(futures_util::poll!(&mut first).is_pending());
    assert!(futures_util::poll!(&mut second).is_pending());
    assert!(futures_util::poll!(&mut third).is_pending());
    assert_eq!(
        *provider.queries.lock().unwrap(),
        ["same query", "same query"]
    );
    drop(first);
    assert!(futures_util::poll!(&mut third).is_pending());
    assert_eq!(provider.queries.lock().unwrap().len(), 3);
    // Cancelling one request neither completes nor cancels another request.
    provider.gate.as_ref().unwrap().add_permits(1);
    assert_eq!(*second.await.unwrap(), vec![1.0, 0.0]);
    assert!(futures_util::poll!(&mut third).is_pending());
    drop(third);
    assert_eq!(embeddings.providers.available_permits(), 2);
}

#[tokio::test]
async fn embedding_timeout_releases_provider_capacity() {
    let provider = Arc::new(Provider {
        gate: Some(Semaphore::new(0)),
        ..Default::default()
    });
    let embeddings = QueryEmbeddings::new(provider, 1, Duration::from_millis(10));
    assert!(embeddings.embed("query").await.is_err());
    assert_eq!(embeddings.providers.available_permits(), 1);
}

struct FailingProvider(AtomicUsize);
impl EmbeddingProvider for FailingProvider {
    fn embed<'a>(&'a self, _: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(ServiceError::unavailable("sanitized failure"))
        })
    }
}
#[tokio::test]
async fn embedding_failure_allows_an_independent_retry() {
    let provider = Arc::new(FailingProvider(AtomicUsize::new(0)));
    let embeddings = QueryEmbeddings::new(provider.clone(), 1, Duration::from_secs(1));
    assert!(embeddings.embed("query").await.is_err());
    assert!(embeddings.embed("query").await.is_err());
    assert_eq!(provider.0.load(Ordering::SeqCst), 2);
    assert_eq!(embeddings.providers.available_permits(), 1);
}
