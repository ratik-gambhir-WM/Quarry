use std::{env, num::NonZeroUsize, sync::Arc};

use crate::{
    adapters::helix::client::HelixClient,
    domains::documents::index::{
        model::{FileChunkNode, FileNode, FileVersionNode},
        query::{FileChunkKeywordSearch, FileChunkVectorSearch},
        repository::{DocumentIndexReader, DocumentIndexWriter, DocumentSearchIndex},
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
    let writer = DocumentIndexWriter::new(helix.clone());
    let reader = DocumentIndexReader::new(helix.clone());
    let search = DocumentSearchIndex::new(helix.clone());
    writer
        .initialize(vector_dimension)
        .await
        .expect("the disposable runtime must initialize every document index");
    let file = FileNode {
        workspace_id: "compat-workspace".to_string(),
        file_id: "compat-file".to_string(),
        display_name: "synthetic.txt".to_string(),
    };
    let version = FileVersionNode {
        workspace_id: "compat-workspace".to_string(),
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
        workspace_id: "compat-other-workspace".to_string(),
        file_id: "compat-other-file".to_string(),
        display_name: "synthetic-other.txt".to_string(),
    };
    let other_version = FileVersionNode {
        workspace_id: "compat-other-workspace".to_string(),
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
            workspace_id: "compat-workspace".to_string(),
            query_embedding: vec![0.25, 0.75],
            limit: 10,
        })
        .await
        .expect("the vector response must map");
    assert!(!vector_hits.is_empty());
    assert!(vector_hits
        .iter()
        .all(|hit| { hit.chunk.workspace_id == "compat-workspace" && hit.distance.is_finite() }));

    let keyword_hits = search
        .search_keyword(FileChunkKeywordSearch {
            workspace_id: "compat-workspace".to_string(),
            query_text: "synthetic compatibility".to_string(),
            limit: 10,
        })
        .await
        .expect("the keyword response must map");
    assert!(!keyword_hits.is_empty());
    assert!(keyword_hits
        .iter()
        .all(|hit| { hit.chunk.workspace_id == "compat-workspace" && hit.score.is_finite() }));

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
            workspace_id: "compat-workspace".to_string(),
            query_embedding: vec![f32::NAN],
            limit: 1,
        })
        .await
        .is_err());
}

fn file_chunk(
    workspace_id: &str,
    file_id: &str,
    version_id: &str,
    chunk_id: &str,
    chunk_index: i64,
    text: &str,
) -> FileChunkNode {
    FileChunkNode {
        chunk_id: chunk_id.to_string(),
        workspace_id: workspace_id.to_string(),
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
