use super::super::embedding::EmbeddingProvider;
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
struct Fake {
    deals: usize,
    prepared: AtomicUsize,
    keywords: AtomicUsize,
    embeddings: AtomicUsize,
    revoke: bool,
    stale: bool,
    fail: bool,
    keyword_empty: bool,
    semantic_distance: f64,
    prepare_gate: Option<Arc<Semaphore>>,
}
impl Fake {
    fn new(deals: usize) -> Self {
        Self {
            deals,
            prepared: AtomicUsize::new(0),
            keywords: AtomicUsize::new(0),
            embeddings: AtomicUsize::new(0),
            revoke: false,
            stale: false,
            fail: false,
            keyword_empty: false,
            semantic_distance: 0.1,
            prepare_gate: None,
        }
    }
    fn chunk(deal: &str) -> FileChunkResult {
        serde_json::from_value(serde_json::json!({"deal_id":deal,"file_id":"f","version_id":"v","chunk_id":deal,"index_generation":"g","chunk_sha256":"hash","chunk_index":1,"text":"Synthetic passage","token_count":2,"page_start":1,"page_end":1,"char_start":0,"char_end":17,"section_path":"","created_at":""})).unwrap()
    }
}
impl EmbeddingProvider for Fake {
    fn embed<'a>(&'a self, _: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>> {
        async move {
            self.embeddings.fetch_add(1, Ordering::SeqCst);
            Ok(vec![1.0, 0.0])
        }
        .boxed()
    }
}
impl HybridBackend for Fake {
    fn deals<'a>(&'a self, _: i64, _: &'a str) -> BoxFuture<'a, ServiceResult<Vec<String>>> {
        async move { Ok((0..self.deals).map(|i| format!("DEAL-{i}")).collect()) }.boxed()
    }
    fn branches<'a>(
        &'a self,
        _actor: i64,
        deal: &'a str,
        _query: &'a str,
        _vector: Arc<Vec<f32>>,
        limit: usize,
    ) -> BoxFuture<'a, ServiceResult<(Vec<FileChunkResult>, Vec<VectorFileChunkHit>)>> {
        async move {
            assert_eq!(limit, 50);
            self.prepared.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = &self.prepare_gate {
                gate.acquire().await.unwrap().forget();
            }
            self.keywords.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(ServiceError::internal("private upstream diagnostic"));
            }
            if self.stale {
                return Ok((vec![], vec![]));
            }
            let keyword = if self.keyword_empty {
                vec![]
            } else {
                vec![Self::chunk(deal)]
            };
            Ok((
                keyword,
                vec![VectorFileChunkHit {
                    chunk: Self::chunk(deal),
                    distance: self.semantic_distance,
                }],
            ))
        }
        .boxed()
    }
    fn final_ids(
        &self,
        actor: i64,
        ids: Vec<String>,
    ) -> BoxFuture<'_, ServiceResult<HashSet<String>>> {
        async move {
            Ok(if self.revoke || actor == 2 {
                HashSet::new()
            } else {
                ids.into_iter().collect()
            })
        }
        .boxed()
    }
}
fn request() -> HybridSearch {
    HybridSearch {
        query_text: "Question".into(),
        limit: 30,
    }
}
fn config() -> SearchConfig {
    SearchConfig {
        enabled: true,
        ..Default::default()
    }
}
#[tokio::test]
async fn hybrid_embeds_once_then_uses_one_snapshot_per_deal() {
    let fake = Arc::new(Fake::new(10));
    let embeddings = Arc::new(QueryEmbeddings::new(
        fake.clone(),
        4,
        std::time::Duration::from_secs(1),
    ));
    let runtime = HybridRuntime::new(config(), Some(embeddings));
    let hits = runtime.run(fake.as_ref(), 1, "", request()).await.unwrap();
    assert_eq!(hits.results.len(), 10);
    assert_eq!(fake.prepared.load(Ordering::SeqCst), 10);
    assert_eq!(fake.embeddings.load(Ordering::SeqCst), 1);
    assert!(hits.results.iter().all(|h| h.match_sources.len() == 2));
    // Each request embeds independently and repeats final authorization.
    assert!(runtime
        .run(fake.as_ref(), 2, "", request())
        .await
        .unwrap()
        .results
        .is_empty());
    assert_eq!(fake.embeddings.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn hybrid_empty_scope_needs_no_provider_and_stale_or_revoked_hits_are_removed() {
    let runtime = HybridRuntime::new(config(), None);
    assert!(runtime
        .run(&Fake::new(0), 1, "", request())
        .await
        .unwrap()
        .results
        .is_empty());
    assert!(runtime.run(&Fake::new(1), 1, "", request()).await.is_err());
    for (stale, revoke) in [(true, false), (false, true)] {
        let fake = Arc::new(Fake {
            stale,
            revoke,
            ..Fake::new(1)
        });
        let embeddings = Arc::new(QueryEmbeddings::new(
            fake.clone(),
            4,
            std::time::Duration::from_secs(1),
        ));
        assert!(HybridRuntime::new(config(), Some(embeddings))
            .run(fake.as_ref(), 1, "", request())
            .await
            .unwrap()
            .results
            .is_empty());
    }
}
#[tokio::test]
async fn hybrid_branch_failure_is_not_partial_success_and_releases_admission() {
    let fake = Arc::new(Fake {
        fail: true,
        ..Fake::new(1)
    });
    let embeddings = Arc::new(QueryEmbeddings::new(
        fake.clone(),
        4,
        std::time::Duration::from_secs(1),
    ));
    let runtime = HybridRuntime::new(config(), Some(embeddings));
    let error = runtime
        .run(fake.as_ref(), 1, "", request())
        .await
        .err()
        .unwrap();
    assert!(matches!(error, ServiceError::Unavailable(_)));
    assert!(!error.to_string().contains("private upstream"));
    assert_eq!(
        runtime.active.available_permits(),
        runtime.config.active_requests
    );
    assert_eq!(
        runtime.admitted.available_permits(),
        runtime.config.active_requests + runtime.config.queued_requests
    );
}

#[tokio::test]
async fn hybrid_bounds_deal_snapshots_after_embedding_and_drops_all_request_work() {
    let gate = Arc::new(Semaphore::new(0));
    let fake = Arc::new(Fake {
        prepare_gate: Some(gate.clone()),
        ..Fake::new(10)
    });
    let embeddings = Arc::new(QueryEmbeddings::new(
        fake.clone(),
        4,
        std::time::Duration::from_secs(1),
    ));
    let runtime = HybridRuntime::new(config(), Some(embeddings));
    let mut request = Box::pin(runtime.run(fake.as_ref(), 1, "", request()));
    assert!(futures_util::poll!(&mut request).is_pending());
    assert_eq!(fake.prepared.load(Ordering::SeqCst), 4);
    assert_eq!(fake.embeddings.load(Ordering::SeqCst), 1);
    assert_eq!(fake.keywords.load(Ordering::SeqCst), 0);
    drop(request);
    // Pending preparation/embedding futures have been dropped, not spawned independently.
    gate.add_permits(10);
    tokio::task::yield_now().await;
    assert_eq!(fake.prepared.load(Ordering::SeqCst), 4);
    assert_eq!(fake.keywords.load(Ordering::SeqCst), 0);
    assert_eq!(
        runtime.active.available_permits(),
        runtime.config.active_requests
    );
}
#[tokio::test]
async fn hybrid_admission_is_bounded_and_deadlines_return_explicit_errors() {
    let fake = Arc::new(Fake {
        prepare_gate: Some(Arc::new(Semaphore::new(0))),
        ..Fake::new(1)
    });
    let embeddings = Arc::new(QueryEmbeddings::new(
        fake.clone(),
        4,
        std::time::Duration::from_secs(1),
    ));
    let runtime = HybridRuntime::new(
        SearchConfig {
            active_requests: 1,
            queued_requests: 1,
            queue_timeout: std::time::Duration::ZERO,
            request_timeout: std::time::Duration::from_millis(10),
            ..config()
        },
        Some(embeddings),
    );
    let mut active = Box::pin(runtime.run(fake.as_ref(), 1, "", request()));
    assert!(futures_util::poll!(&mut active).is_pending());
    let mut queued = Box::pin(runtime.run(fake.as_ref(), 1, "", request()));
    assert!(futures_util::poll!(&mut queued).is_pending());
    assert!(runtime
        .run(fake.as_ref(), 1, "", request())
        .await
        .err()
        .unwrap()
        .to_string()
        .contains("busy"));
    assert!(queued.await.err().unwrap().to_string().contains("queue"));
    assert!(active
        .await
        .err()
        .unwrap()
        .to_string()
        .contains("timed out"));
    assert_eq!(runtime.admitted.available_permits(), 2);
}

#[tokio::test]
async fn hybrid_search_read_limit_is_shared_across_service_clones_and_released_on_drop() {
    let repository = crate::domains::documents::test_support::search(
        crate::domains::documents::test_support::database().await,
    );
    let service = DocumentSearchService::new(repository).with_hybrid(
        SearchConfig {
            sql_reads: 2,
            ..Default::default()
        },
        None,
    );
    let clone = service.clone();
    let started = AtomicUsize::new(0);
    let work = || async {
        started.fetch_add(1, Ordering::SeqCst);
        futures_util::future::pending::<Result<(), crate::shared::error::RepositoryError>>().await
    };
    let mut a = Box::pin(service.read(work()));
    let mut b = Box::pin(clone.read(work()));
    let mut c = Box::pin(clone.read(work()));
    assert!(futures_util::poll!(&mut a).is_pending());
    assert!(futures_util::poll!(&mut b).is_pending());
    assert!(futures_util::poll!(&mut c).is_pending());
    assert_eq!(started.load(Ordering::SeqCst), 2);
    drop(a);
    assert!(futures_util::poll!(&mut c).is_pending());
    assert_eq!(started.load(Ordering::SeqCst), 3);
    drop(b);
    drop(c);
    assert!(service.read(async { Ok(()) }).await.is_ok());
}

#[tokio::test]
async fn hybrid_does_not_fill_unrelated_queries_with_weak_nearest_neighbors() {
    for distance in [0.9, 1.0, 1.5, f64::NAN, f64::INFINITY, -0.1] {
        let fake = Arc::new(Fake {
            keyword_empty: true,
            semantic_distance: distance,
            ..Fake::new(1)
        });
        let embeddings = Arc::new(QueryEmbeddings::new(
            fake.clone(),
            4,
            std::time::Duration::from_secs(1),
        ));
        let runtime = HybridRuntime::new(config(), Some(embeddings));
        let response = runtime
            .run(
                fake.as_ref(),
                1,
                "",
                HybridSearch {
                    query_text: "sports".into(),
                    limit: 30,
                },
            )
            .await
            .unwrap();
        assert!(
            response.results.is_empty(),
            "distance {distance} must not create a result"
        );
    }
}
#[tokio::test]
async fn hybrid_preserves_exact_keyword_hits_and_relevant_semantic_paraphrases() {
    for (keyword_empty, distance, sources) in [
        (false, 0.9, vec![super::super::model::MatchSource::Keyword]),
        (true, 0.3, vec![super::super::model::MatchSource::Semantic]),
    ] {
        let fake = Arc::new(Fake {
            keyword_empty,
            semantic_distance: distance,
            ..Fake::new(1)
        });
        let embeddings = Arc::new(QueryEmbeddings::new(
            fake.clone(),
            4,
            std::time::Duration::from_secs(1),
        ));
        let response = HybridRuntime::new(config(), Some(embeddings))
            .run(fake.as_ref(), 1, "", request())
            .await
            .unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].match_sources, sources);
    }
    assert!(relevant_semantic_distance(0.3, 0.4));
    assert!(!relevant_semantic_distance(0.30001, 0.4));
    assert!(!relevant_semantic_distance(0.3, 0.8));
}

#[tokio::test]
async fn hybrid_rejects_helix_orthogonal_and_weak_neighbors_instead_of_filling_thirty_results() {
    // The old 1-distance conversion falsely interpreted an orthogonal 0.5 as 50% similarity.
    // Use the actual normalized scale from the pinned Helix compatibility test.
    for distance in [0.5, 0.45, 0.4, 0.35] {
        let fake = Arc::new(Fake {
            keyword_empty: true,
            semantic_distance: distance,
            ..Fake::new(30)
        });
        let embeddings = Arc::new(QueryEmbeddings::new(
            fake.clone(),
            4,
            std::time::Duration::from_secs(1),
        ));
        let results = HybridRuntime::new(config(), Some(embeddings))
            .run(
                fake.as_ref(),
                1,
                "",
                HybridSearch {
                    query_text: "Bread".into(),
                    limit: 30,
                },
            )
            .await
            .unwrap();
        assert!(
            results.results.is_empty(),
            "normalized Helix distance {distance} must be rejected"
        );
    }
}

#[tokio::test]
async fn hybrid_recovers_measured_concept_matches_without_admitting_judged_controls() {
    // Recorded with text-embedding-3-large/3072 and the actual current Helix passages.
    // This guards the filtering policy, not future model quality or unjudged queries.
    for (case, similarity, expected) in [
        ("royal_dynasties_rome", 0.3694882392883301, 1),
        ("legacy_technology_question_cobol", 0.3925054669380188, 1),
        ("programming_language_cobol", 0.2832067012786865, 1),
        ("bread", 0.3170681595802307, 1),
        ("sports", 0.12832111120224, 0),
        ("football_scoring", 0.17572879791259766, 0),
        ("diesel_starter", 0.14035749435424805, 0),
        ("database_restore", 0.22503066062927246, 0),
        ("mobile_app_navigation", 0.25388896465301514, 0),
    ] {
        let fake = Arc::new(Fake {
            keyword_empty: true,
            semantic_distance: (1.0 - similarity) / 2.0,
            ..Fake::new(1)
        });
        let embeddings = Arc::new(QueryEmbeddings::new(
            fake.clone(),
            4,
            std::time::Duration::from_secs(1),
        ));
        let measured_config = SearchConfig {
            min_semantic_similarity: 0.26,
            ..config()
        };
        let response = HybridRuntime::new(measured_config, Some(embeddings))
            .run(fake.as_ref(), 1, "", request())
            .await
            .unwrap();
        assert_eq!(response.results.len(), expected, "case: {case}");
    }
}
