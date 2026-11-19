//! SQL document retrieval is PostgreSQL-owned, including authorization and deletion.
use crate::domains::documents::test_support::*;
#[tokio::test]
async fn sql_document_search_filters_membership_before_limits_and_keeps_wire_metadata() {
    let db = database().await;
    save(&db, "own", "DEAL-SQL", "shared", 3).await;
    save(&db, "foreign", "DEAL-FOREIGN", "secret", 12).await;
    let repository = search(db.clone());
    let (keyword, vector) = repository
        .branches(
            1,
            "DEAL-SQL",
            Some("Architecture shared"),
            Some(vec![0.5; 1536]),
            2,
        )
        .await
        .unwrap();
    assert_eq!(keyword.len(), 2);
    assert_eq!(vector.len(), 2);
    assert!(keyword.iter().all(|h| h.chunk.deal_id == "DEAL-SQL"
        && h.chunk.page_start.is_some()
        && h.chunk.char_end >= h.chunk.char_start));
    assert!(vector.iter().all(|h| h.distance.abs() < 1e-6));
    assert!(repository
        .branches(1, "DEAL-FOREIGN", Some("secret"), None, 2)
        .await
        .is_err());
    for query in ["unrelated", "!!!"] {
        assert!(repository
            .branches(1, "DEAL-SQL", Some(query), None, 50)
            .await
            .unwrap()
            .0
            .is_empty());
    }
    let profile = crate::domains::documents::store::search_repository::EmbeddingProfile::new(
        "another-model".into(),
        1536,
    );
    let mismatched =
        crate::domains::documents::store::search_repository::DocumentSearchRepository::new(
            db, profile,
        );
    assert!(mismatched
        .branches(1, "DEAL-SQL", None, Some(vec![0.5; 1536]), 50)
        .await
        .unwrap()
        .1
        .is_empty());
}

#[tokio::test]
async fn lexical_channels_keep_technical_tokens_english_stems_and_safe_and_semantics() {
    use crate::{
        domains::documents::{
            ingestion::persistence::persist_document_and_chunks, store::repository::DocumentStore,
        },
        shared::ids::sha256_hex,
    };
    let db = database().await;
    for (file, text) in [
        ("cpp", "C++ memory allocator"),
        ("csharp", "C# managed runtime"),
        ("dotnet", ".NET managed runtime"),
        ("identifier", "customer_id database identifier"),
        ("phrase", "alpha beta architecture José"),
        ("separated", "alpha noisy beta"),
    ] {
        let (doc, mut chunks, bytes) = sample(file, "DEAL-SQL", text, 1);
        chunks[0].text = text.into();
        chunks[0].content_hash = sha256_hex(text.as_bytes());
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
    }
    let repo = search(db);
    for (query, expected) in [
        ("C++", "cpp"),
        ("C#", "csharp"),
        (".NET", "dotnet"),
        ("customer_id", "identifier"),
        ("architectures", "phrase"),
        ("arch*", "phrase"),
        ("\"alpha beta\"", "phrase"),
        ("José", "phrase"),
    ] {
        let (hits, _) = repo
            .branches(1, "DEAL-SQL", Some(query), None, 50)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "query {query}");
        assert_eq!(hits[0].chunk.file_id, expected);
    }
    for query in [
        "the and",
        "!!!",
        "C++ managed",
        "alpha missing",
        "customer_id unknown",
    ] {
        assert!(
            repo.branches(1, "DEAL-SQL", Some(query), None, 50)
                .await
                .unwrap()
                .0
                .is_empty(),
            "query {query}"
        );
    }
}
