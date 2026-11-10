use helix_db::dsl::prelude::*;
use std::{env, num::NonZeroUsize, sync::Arc};

use crate::{
    adapters::helix::client::HelixClient,
    domains::documents::graph::{
        model::{FileChunkNode, FileNode, FileVersionNode},
        query::{FileChunkKeywordSearch, FileChunkVectorSearch},
        repository::{DocumentGraphReader, DocumentGraphSearch, DocumentGraphWriter},
    },
};

const HELIX_COMPAT_URL_ENV: &str = "QUARRY_HELIX_COMPAT_URL";

#[tokio::test]
#[ignore = "requires an explicitly supplied disposable QUARRY_HELIX_COMPAT_URL"]
async fn v3_runtime_accepts_document_index_initialization_and_graph_writes() {
    let url = env::var(HELIX_COMPAT_URL_ENV)
        .expect("the ignored compatibility test requires an explicit Helix URL");
    let helix = Arc::new(HelixClient::with_config(&url, None).expect("valid Helix URL"));
    let vector_dimension = NonZeroUsize::new(2).expect("test vector dimension is non-zero");
    let writer = DocumentGraphWriter::new(helix.clone(), vector_dimension);
    let reader = DocumentGraphReader::new(helix.clone());
    let search = DocumentGraphSearch::new(helix.clone());
    writer
        .initialize()
        .await
        .expect("the disposable runtime must initialize every document index");
    let file = FileNode {
        deal_id: "compat-workspace".to_string(),
        file_id: "compat-file".to_string(),
        display_name: "synthetic.txt".to_string(),
    };
    let version = FileVersionNode {
        deal_id: "compat-workspace".to_string(),
        file_id: "compat-file".to_string(),
        version_id: "compat-version".to_string(),
        mime_type: "text/plain".to_string(),
        content_sha256: "compat-content".to_string(),
        byte_size: 2,
        index_generation: "compat-version".to_string(),
        indexed_at: "2026-09-28T00:00:00.000Z".to_string(),
    };
    let chunks = vec![
        file_chunk(
            "compat-workspace",
            "compat-file",
            "compat-version",
            "compat-chunk-1",
            1,
            "synthetic compatibility beta",
        ),
        file_chunk(
            "compat-workspace",
            "compat-file",
            "compat-version",
            "compat-chunk-0",
            0,
            "synthetic compatibility alpha",
        ),
    ];
    writer
        .insert_graph(file.clone(), version.clone(), chunks.clone())
        .await
        .expect("the disposable runtime must accept one synthetic graph write");

    assert_eq!(
        reader
            .current_document("compat-workspace", "compat-file")
            .await
            .expect("the current document response must map")
            .expect("the synthetic current document must be present")
            .version,
        version
    );
    assert!(reader
        .current_document_by_hash("compat-workspace", "compat-content")
        .await
        .expect("the content-hash response must map")
        .is_some());
    assert!(reader
        .document_version("compat-workspace", "compat-file", "compat-version")
        .await
        .expect("the historical version response must map")
        .is_some());
    assert_eq!(
        reader
            .document_version_chunks("compat-workspace", "compat-file", "compat-version")
            .await
            .expect("the chunks response must map")
            .iter()
            .map(|chunk| chunk.chunk_index)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );

    writer
        .insert_graph(file.clone(), version.clone(), chunks)
        .await
        .expect("replaying the same graph write must converge without duplicates");
    assert_eq!(
        reader
            .document_version_chunks("compat-workspace", "compat-file", "compat-version")
            .await
            .expect("the replayed graph must still be readable")
            .len(),
        2
    );

    let other_file = FileNode {
        deal_id: "compat-other-workspace".to_string(),
        file_id: "compat-other-file".to_string(),
        display_name: "synthetic-other.txt".to_string(),
    };
    let other_version = FileVersionNode {
        deal_id: "compat-other-workspace".to_string(),
        file_id: "compat-other-file".to_string(),
        version_id: "compat-other-version".to_string(),
        mime_type: "text/plain".to_string(),
        content_sha256: "compat-other-content".to_string(),
        byte_size: 2,
        index_generation: "compat-other-version".to_string(),
        indexed_at: "2026-09-28T00:00:00.000Z".to_string(),
    };
    writer
        .insert_graph(
            other_file,
            other_version,
            vec![file_chunk(
                "compat-other-workspace",
                "compat-other-file",
                "compat-other-version",
                "compat-other-chunk",
                0,
                "synthetic compatibility other workspace",
            )],
        )
        .await
        .expect("the second synthetic workspace must be writable");

    let vector_hits = search
        .search_vector(FileChunkVectorSearch {
            deal_id: "compat-workspace".to_string(),
            query_embedding: vec![0.25, 0.75],
            limit: 10,
        })
        .await
        .expect("the vector response must map");
    assert!(!vector_hits.is_empty());
    assert!(vector_hits
        .iter()
        .all(|hit| { hit.chunk.deal_id == "compat-workspace" && hit.distance.is_finite() }));

    let keyword_hits = search
        .search_keyword(FileChunkKeywordSearch {
            deal_id: "compat-workspace".to_string(),
            query_text: "synthetic compatibility".to_string(),
            limit: 10,
        })
        .await
        .expect("the keyword response must map");
    assert!(!keyword_hits.is_empty());
    assert!(keyword_hits
        .iter()
        .all(|hit| { hit.chunk.deal_id == "compat-workspace" && hit.score.is_finite() }));

    writer
        .insert_graph(
            file,
            version,
            vec![file_chunk(
                "compat-workspace",
                "compat-file",
                "compat-version",
                "compat-replacement-chunk",
                0,
                "synthetic compatibility replacement",
            )],
        )
        .await
        .expect("the version-scoped chunk replacement must be writable");
    assert_eq!(
        reader
            .document_version_chunks("compat-workspace", "compat-file", "compat-version")
            .await
            .expect("the replacement chunks response must map")
            .len(),
        1
    );

    assert!(search
        .search_vector(FileChunkVectorSearch {
            deal_id: "compat-workspace".to_string(),
            query_embedding: vec![f32::NAN],
            limit: 1,
        })
        .await
        .is_err());
}

fn file_chunk(
    deal_id: &str,
    file_id: &str,
    version_id: &str,
    chunk_id: &str,
    chunk_index: i64,
    text: &str,
) -> FileChunkNode {
    FileChunkNode {
        chunk_id: chunk_id.to_string(),
        deal_id: deal_id.to_string(),
        file_id: file_id.to_string(),
        version_id: version_id.to_string(),
        index_generation: version_id.to_string(),
        chunk_index,
        text: text.to_string(),
        embedding: vec![0.25, 0.75],
        chunk_sha256: format!("{chunk_id}-hash"),
        token_count: 3,
        page_start: None,
        page_end: None,
        char_start: 0,
        char_end: i64::try_from(text.len()).expect("synthetic text length fits i64"),
        section_path: "Synthetic".to_string(),
        created_at: "2026-09-28T00:00:00.000Z".to_string(),
    }
}

#[tokio::test]
#[ignore = "requires explicitly selected disposable QUARRY_HELIX_COMPAT_URL"]
async fn v3_current_vector_query_preserves_cosine_distances_and_candidate_filter() {
    let url = env::var(HELIX_COMPAT_URL_ENV).expect("disposable Helix URL");
    let helix = Arc::new(HelixClient::with_config(&url, None).unwrap());
    let writer = DocumentGraphWriter::new(helix.clone(), std::num::NonZeroUsize::new(2).unwrap());
    writer.initialize().await.unwrap();
    let search = DocumentGraphSearch::new(helix.clone());
    let deal = "DEAL-VECTOR-METRIC-CHECK";
    let mut chunks = Vec::new();
    for (index, (id, vector)) in [
        ("aligned", vec![1.0, 0.0]),
        ("scaled", vec![5.0, 0.0]),
        ("orthogonal", vec![0.0, 1.0]),
        ("opposite", vec![-1.0, 0.0]),
        ("oblique", vec![3.0, 4.0]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut chunk = file_chunk(
            deal,
            "metric-file",
            "metric-version",
            id,
            i64::try_from(index).unwrap(),
            "Synthetic metric evidence",
        );
        chunk.embedding = vector;
        chunks.push(chunk);
    }
    let expected_vectors = chunks
        .iter()
        .map(|c| (c.chunk_id.clone(), c.embedding.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    writer
        .insert_graph(
            FileNode {
                deal_id: deal.into(),
                file_id: "metric-file".into(),
                display_name: "synthetic-metric.txt".into(),
            },
            FileVersionNode {
                deal_id: deal.into(),
                file_id: "metric-file".into(),
                version_id: "metric-version".into(),
                mime_type: "text/plain".into(),
                content_sha256: "synthetic-metric-content".into(),
                byte_size: 25,
                index_generation: "metric-version".into(),
                indexed_at: "2026-11-06T00:00:00.000Z".into(),
            },
            chunks,
        )
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct StoredVector {
        chunk_id: String,
        embedding: Vec<f32>,
    }
    #[derive(serde::Deserialize)]
    struct StoredVectors {
        chunks: Vec<StoredVector>,
    }
    let stored: StoredVectors = helix
        .execute_read_query(
            "test.synthetic_vector_dimensions",
            metric_stored_vectors(deal.into()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stored.chunks.len(), expected_vectors.len());
    for stored in stored.chunks {
        assert_eq!(stored.embedding.len(), 2);
        assert_eq!(stored.embedding, expected_vectors[&stored.chunk_id]);
    }
    let query = |limit| FileChunkVectorSearch {
        deal_id: deal.into(),
        query_embedding: vec![1.0, 0.0],
        limit,
    };
    let ids = vec![
        "aligned".into(),
        "scaled".into(),
        "orthogonal".into(),
        "opposite".into(),
        "oblique".into(),
    ];
    let hits = search.vector_current(query(5), ids).await.unwrap();
    assert_eq!(hits.len(), 5);
    for (id, expected) in [
        ("aligned", 0.0),
        ("scaled", 0.0),
        ("orthogonal", 0.5),
        ("opposite", 1.0),
        ("oblique", 0.2),
    ] {
        let hit = hits.iter().find(|h| h.chunk.chunk_id == id).unwrap();
        assert!(
            (hit.distance - expected).abs() < 1e-5,
            "{id}: expected {expected}, got {}",
            hit.distance
        );
    }
    let scoped = search
        .vector_current(query(1), vec!["orthogonal".into(), "opposite".into()])
        .await
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].chunk.chunk_id, "orthogonal");
    assert!((scoped[0].distance - 0.5).abs() < 1e-5);
}

#[query]
fn metric_stored_vectors(deal: String) -> ReadBatch {
    let _ = &deal;
    read_batch()
        .var_as(
            "chunks",
            g().n_with_label("FileChunk")
                .where_(Predicate::eq_param("deal_id", "deal"))
                .project(vec![
                    PropertyProjection::new("chunk_id"),
                    PropertyProjection::new("embedding"),
                ]),
        )
        .returning(["chunks"])
}

#[tokio::test]
#[ignore = "requires an explicitly supplied disposable QUARRY_HELIX_COMPAT_URL"]
async fn deletion_fence_rejects_prepared_setup_chunks_and_publication() {
    use crate::domains::documents::graph::{deletion, writer::insert_file_version_graph};
    let url = env::var(HELIX_COMPAT_URL_ENV).expect("explicit disposable Helix URL");
    let helix = Arc::new(HelixClient::with_config(&url, None).unwrap());
    let writer = DocumentGraphWriter::new(helix.clone(), NonZeroUsize::new(2).unwrap());
    writer.initialize().await.unwrap();
    let deal = format!("delete-{}", uuid::Uuid::new_v4());
    let file = FileNode {
        deal_id: deal.clone(),
        file_id: format!("{deal}-file"),
        display_name: "synthetic".into(),
    };
    let version = FileVersionNode {
        deal_id: deal.clone(),
        file_id: file.file_id.clone(),
        version_id: format!("{deal}-version"),
        mime_type: "text/plain".into(),
        content_sha256: "synthetic".into(),
        byte_size: 1,
        index_generation: "generation".into(),
        indexed_at: "2026-11-10T00:00:00Z".into(),
    };
    let mut chunk = file_chunk(
        &deal,
        &file.file_id,
        &version.version_id,
        &format!("{deal}-chunk"),
        0,
        "synthetic fence proof",
    );
    chunk.index_generation = "generation".into();
    let prepared =
        insert_file_version_graph(file.clone(), version.clone(), vec![chunk.clone()]).unwrap();
    for _ in 0..2 {
        let result: serde_json::Value = helix
            .execute_write_query("test.fence", deletion::fence(deal.clone()).unwrap())
            .await
            .unwrap();
        assert_eq!(result["fence"].as_array().unwrap().len(), 1);
    }
    let mut requests = vec![prepared.setup_query];
    requests.extend(prepared.chunk_queries);
    requests.push(prepared.publish_query);
    for request in requests {
        let result: serde_json::Value = helix
            .execute_write_query("test.late_write", request)
            .await
            .unwrap();
        assert_eq!(result["deletion_fence"].as_array().unwrap().len(), 1);
    }
    assert!(writer
        .insert_graph(file.clone(), version, vec![chunk])
        .await
        .is_err());
    let result: serde_json::Value = helix
        .execute_read_query(
            "test.absent",
            QueryRequest::read(
                read_batch()
                    .var_as(
                        "files",
                        g().n_with_label("QuarryFile")
                            .where_(Predicate::eq("deal_id", deal.clone())),
                    )
                    .var_as(
                        "versions",
                        g().n_with_label("FileVersion")
                            .where_(Predicate::eq("deal_id", deal.clone())),
                    )
                    .var_as(
                        "chunks",
                        g().n_with_label("FileChunk")
                            .where_(Predicate::eq("deal_id", deal)),
                    )
                    .returning(["files", "versions", "chunks"]),
            ),
        )
        .await
        .unwrap();
    for key in ["files", "versions", "chunks"] {
        assert!(result[key].as_array().unwrap().is_empty());
    }
}

#[tokio::test]
#[ignore = "requires an explicitly supplied disposable QUARRY_HELIX_COMPAT_URL"]
async fn deletion_purges_scoped_nodes_edges_and_search_indexes_without_touching_other_deals() {
    let url = env::var(HELIX_COMPAT_URL_ENV).expect("explicit disposable Helix URL");
    let helix = Arc::new(HelixClient::with_config(&url, None).unwrap());
    let writer = DocumentGraphWriter::new(helix.clone(), NonZeroUsize::new(2).unwrap());
    writer.initialize().await.unwrap();
    let deal = format!("purge-{}", uuid::Uuid::new_v4());
    let other = format!("other-{}", uuid::Uuid::new_v4());
    for scope in [&deal, &other] {
        let file = FileNode {
            deal_id: scope.clone(),
            file_id: format!("{scope}-file"),
            display_name: "synthetic".into(),
        };
        for generation in ["old", "current", "partial"] {
            let id = format!("{scope}-{generation}");
            let version = FileVersionNode {
                deal_id: scope.clone(),
                file_id: file.file_id.clone(),
                version_id: id.clone(),
                mime_type: "text/plain".into(),
                content_sha256: id.clone(),
                byte_size: 1,
                index_generation: id.clone(),
                indexed_at: "2026-11-10T00:00:00Z".into(),
            };
            let mut chunk = file_chunk(
                scope,
                &file.file_id,
                &id,
                &format!("{id}-chunk"),
                0,
                "synthetic cleanup searchable",
            );
            chunk.index_generation = id;
            if generation == "partial" {
                let prepared = crate::domains::documents::graph::writer::insert_file_version_graph(
                    file.clone(),
                    version,
                    vec![chunk],
                )
                .unwrap();
                let _: serde_json::Value = helix
                    .execute_write_query("test.partial_setup", prepared.setup_query)
                    .await
                    .unwrap();
                for query in prepared.chunk_queries {
                    let _: serde_json::Value = helix
                        .execute_write_query("test.partial_chunks", query)
                        .await
                        .unwrap();
                }
            } else {
                writer
                    .insert_graph(file.clone(), version, vec![chunk])
                    .await
                    .unwrap();
            }
        }
    }
    assert!(
        writer
            .purge_deal_file(&deal, &format!("{other}-file"))
            .await
            .is_err(),
        "a mismatched file owner must block cleanup"
    );
    writer.fence_deal(&deal).await.unwrap();
    // Bounded, repeatable sweeps remove current and historical versions and all edges.
    while !writer
        .purge_deal_file(&deal, &format!("{deal}-file"))
        .await
        .unwrap()
    {}
    assert!(writer
        .purge_deal_file(&deal, &format!("{deal}-file"))
        .await
        .unwrap());
    writer.verify_deal_absent(&deal).await.unwrap();
    let reader = DocumentGraphReader::new(helix.clone());
    assert!(reader
        .current_document(&deal, &format!("{deal}-file"))
        .await
        .unwrap()
        .is_none());
    assert!(reader
        .current_document(&other, &format!("{other}-file"))
        .await
        .unwrap()
        .is_some());
    let search = DocumentGraphSearch::new(helix.clone());
    assert!(search
        .keyword_current(
            FileChunkKeywordSearch {
                deal_id: deal.clone(),
                query_text: "synthetic".into(),
                limit: 10
            },
            vec![format!("{deal}-current")]
        )
        .await
        .unwrap()
        .is_empty());
    assert!(search
        .vector_current(
            FileChunkVectorSearch {
                deal_id: deal.clone(),
                query_embedding: vec![1.0, 0.0],
                limit: 10
            },
            vec![format!("{deal}-current")]
        )
        .await
        .unwrap()
        .is_empty());
}
