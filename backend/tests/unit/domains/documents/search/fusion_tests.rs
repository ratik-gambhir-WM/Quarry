use super::*;
pub(super) fn chunk(id: &str) -> FileChunkResult {
    FileChunkResult {
        chunk_id: id.into(),
        deal_id: "DEAL-1".into(),
        file_id: "file".into(),
        version_id: "v".into(),
        index_generation: "g".into(),
        chunk_sha256: "hash".into(),
        chunk_index: 1,
        text: "Synthetic passage".into(),
        token_count: 2,
        page_start: Some(1),
        page_end: Some(1),
        char_start: 0,
        char_end: 17,
        section_path: "".into(),
        created_at: "".into(),
    }
}
#[test]
fn fusion_deduplicates_before_ranking_sums_overlap_and_has_stable_ties() {
    let results = top(
        fuse(
            vec![chunk("a"), chunk("a"), chunk("b")],
            vec![chunk("b"), chunk("c")],
        ),
        10,
    );
    assert_eq!(
        results
            .iter()
            .map(|h| h.chunk_id.as_str())
            .collect::<Vec<_>>(),
        vec!["b", "a", "c"]
    );
    assert_eq!(
        results[0].match_sources,
        vec![MatchSource::Keyword, MatchSource::Semantic]
    );
    assert_eq!(
        top(fuse(vec![], vec![chunk("c")]), 1)[0].match_sources,
        vec![MatchSource::Semantic]
    );
    assert!(top(fuse(vec![], vec![]), 10).is_empty());
    let mut a = fuse(vec![chunk("a")], vec![]);
    a.extend(fuse(vec![chunk("b")], vec![]));
    let mut b = fuse(vec![chunk("b")], vec![]);
    b.extend(fuse(vec![chunk("a")], vec![]));
    assert_eq!(top(a, 1)[0].chunk_id, top(b, 1)[0].chunk_id);
}
#[test]
fn public_dto_hides_generation_hash_and_scores() {
    let result = serde_json::to_value(top(fuse(vec![chunk("a")], vec![]), 1)).unwrap();
    assert_eq!(result[0]["chunkId"], "a");
    assert_eq!(result[0]["matchSources"], serde_json::json!(["keyword"]));
    assert!(result[0].get("score").is_none());
    assert!(result[0].get("indexGeneration").is_none());
}

#[test]
fn fusion_identity_cannot_reuse_a_different_file_version_generation_hash_or_index() {
    let original = chunk("a");
    for field in ["deal", "file", "version", "generation", "hash", "index"] {
        let mut replacement = original.clone();
        match field {
            "deal" => replacement.deal_id = "DEAL-OTHER".into(),
            "file" => replacement.file_id = "other-file".into(),
            "version" => replacement.version_id = "other-version".into(),
            "generation" => replacement.index_generation = "other-generation".into(),
            "hash" => replacement.chunk_sha256 = "other-hash".into(),
            "index" => replacement.chunk_index += 1,
            _ => unreachable!(),
        }
        assert_ne!(identity(&original), identity(&replacement));
    }
}
