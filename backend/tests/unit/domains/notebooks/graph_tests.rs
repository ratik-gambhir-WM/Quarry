use super::*;
use crate::domains::notebooks::{model::*, test_support};
fn note() -> Note {
    Note {
        summary: NoteSummary {
            note_id: "note".into(),
            notebook_id: "book".into(),
            folder_id: None,
            title: "Title".into(),
            content_schema_version: 1,
            revision: 2,
            created_at: now(),
            updated_at: now(),
        },
        blocks: test_support::blocks(),
    }
}
#[test]
fn notebook_projection_preflights_every_chunk_and_publishes_root_last() {
    let note = note();
    let chunks = vec!["first".into(), "second".into()];
    let prepare = || {
        prepare_projection(
            7,
            &note,
            "hash",
            &chunks,
            vec![vec![0.2, 0.8]; 2],
            NonZeroUsize::new(2).unwrap(),
            "test-model",
        )
        .unwrap()
    };
    let queries = prepare();
    assert_eq!(queries.len(), 3);
    let bytes = queries
        .iter()
        .map(|q| q.to_json_bytes().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        bytes,
        prepare()
            .iter()
            .map(|q| q.to_json_bytes().unwrap())
            .collect::<Vec<_>>()
    );
    for chunk in &bytes[..2] {
        let text = String::from_utf8(chunk.clone()).unwrap();
        assert!(text.contains(CHUNK));
        assert!(!text.contains(ROOT));
        assert!(text.contains("7:book:note:2:hash:"));
        assert!(text.contains("embedding_dimensions"));
    }
    let root = String::from_utf8(bytes[2].clone()).unwrap();
    assert!(root.contains(ROOT));
    assert!(root.contains("old_chunks"));
    assert!(root.contains("chunk_count"));
    assert!(prepare_projection(
        7,
        &note,
        "hash",
        &chunks,
        vec![vec![f32::NAN, 0.2]; 2],
        NonZeroUsize::new(2).unwrap(),
        "test-model"
    )
    .is_err());
    assert!(prepare_projection(
        7,
        &note,
        "hash",
        &["x".repeat(2 * 1024 * 1024)],
        vec![vec![0.2, 0.8]],
        NonZeroUsize::new(2).unwrap(),
        "test-model"
    )
    .is_err());
}
#[test]
fn notebook_empty_projection_publishes_no_vectors_and_removes_previous_revision() {
    let queries = prepare_projection(
        1,
        &note(),
        "empty",
        &[],
        vec![],
        NonZeroUsize::new(2).unwrap(),
        "test-model",
    )
    .unwrap();
    assert_eq!(queries.len(), 1);
    let text = String::from_utf8(queries[0].to_json_bytes().unwrap()).unwrap();
    assert!(text.contains("old_chunks"));
    assert!(!text.contains("F32Array"));
}
