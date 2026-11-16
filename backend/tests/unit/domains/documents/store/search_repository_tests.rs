use super::*;
#[tokio::test]
#[ignore = "explicit 10000-chunk disposable PostgreSQL measurement"]
async fn measure_native_document_search_at_ten_thousand_chunks() {
    use crate::domains::documents::{
        ingestion::persistence::persist_document_and_chunks, store::repository::DocumentStore,
        test_support::*,
    };
    let db = database().await;
    let (doc, mut chunks, bytes) = sample("benchmark", "DEAL-SQL", "baseline", 10000);
    chunks[0].text.push_str(" needleonly");
    chunks[0].content_hash = sha256_hex(chunks[0].text.as_bytes());
    let started = std::time::Instant::now();
    persist_document_and_chunks(
        &DocumentStore::new(db.clone()),
        "DEAL-SQL",
        1,
        doc,
        chunks,
        bytes,
        None,
    )
    .await
    .unwrap();
    let ingest_ms = started.elapsed().as_millis();
    // Measure the maintained-index state separately from bulk GIN pending-list cost.
    sqlx::query("VACUUM (ANALYZE) document_chunks")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::raw_sql("ANALYZE document_chunks; ANALYZE document_chunk_identities; ANALYZE quarry_file_versions; ANALYZE quarry_files; ANALYZE document_identity_manifests; ANALYZE deal_memberships; ANALYZE deal_deletions; ANALYZE document_embedding_profiles;").execute(db.pool()).await.unwrap();
    let repo = search(db.clone());
    for query in ["needleonly", "architecture", "absenttoken"] {
        let mut times = Vec::new();
        let mut count = 0;
        for _ in 0..10 {
            let start = std::time::Instant::now();
            count = repo
                .branches(1, "DEAL-SQL", Some(query), None, 20)
                .await
                .unwrap()
                .0
                .len();
            times.push(start.elapsed().as_micros());
        }
        times.sort_unstable();
        eprintln!(
            "native lexical count={count} p50_us={} p95_us={}",
            times[5], times[9]
        );
    }
    let mut times = Vec::new();
    for _ in 0..10 {
        let start = std::time::Instant::now();
        assert_eq!(
            repo.branches(1, "DEAL-SQL", None, Some(vec![0.5; 1536]), 20)
                .await
                .unwrap()
                .1
                .len(),
            20
        );
        times.push(start.elapsed().as_micros());
    }
    times.sort_unstable();
    let index_bytes: i64 =
        sqlx::query_scalar("SELECT pg_relation_size('idx_document_chunks_lexical')")
            .fetch_one(db.pool())
            .await
            .unwrap();
    eprintln!("native ingest_ms={ingest_ms} gin_bytes={index_bytes} exact_vector_p50_us={} exact_vector_p95_us={}",times[5],times[9]);
    let query = ranked(
        1,
        "DEAL-SQL",
        &repo.profile.id,
        Ranking::Keyword("needleonly"),
        20,
    )
    .unwrap();
    let explain = format!("EXPLAIN (ANALYZE,BUFFERS,FORMAT TEXT) {}", query.sql());
    let mut statement = sqlx::query_scalar::<_, String>(&explain);
    for parameter in query.parameters() {
        statement = statement.bind(parameter);
    }
    let plan = statement.fetch_all(db.pool()).await.unwrap().join("\n");
    eprintln!(
        "{}",
        plan.lines()
            .filter(|line| line.contains("Scan") || line.contains("actual time"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        plan.contains("idx_document_chunks_lexical"),
        "selective retrieval must use GIN"
    );
}
