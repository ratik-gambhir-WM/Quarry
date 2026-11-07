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
