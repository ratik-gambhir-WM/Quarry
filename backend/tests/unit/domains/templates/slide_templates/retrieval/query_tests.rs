use super::*;
#[test]
fn query_validation_requires_current_explicit_contract() {
    let mut input = json!({"mode":"filter","query":null,"semantic_target":"both","template_ids":[],"similar_to_template_id":null,"facet_name":null,"filters":{"kinds":[],"domains":[],"topics":[],"intents":[],"archetypes":[],"slot_roles":[],"content_density":[]},"select":["all"],"limit":20});
    assert!(parse_slide_query_input(&input).is_ok());
    input["semantic_target"] = json!("unknown");
    assert!(parse_slide_query_input(&input).is_err());
    assert!(build_safe_fts_query("café cloud", "AND").is_ok());
    assert!(build_safe_fts_query("security*", "AND").is_err());
}

fn record(id: &str, created_at: &str) -> RetrievalClassificationRecord {
    serde_json::from_value(json!({
        "templateId":id,"createdAt":created_at,"kind":"diagram",
        "previewAvailable":false,"title":id,
        "classification":{
            "templateId":id,"attemptCount":1,"embeddingAttemptCount":1,
            "embeddingStatus":"ready","inputFingerprint":"input",
            "promptVersion":"test","schemaVersion":1,"status":"ready",
            "subjectVector":[1.0,0.0],"capabilityVector":[0.0,1.0]
        }
    }))
    .unwrap()
}

#[test]
fn fusion_accumulates_ranks_and_borrows_the_first_record_for_duplicate_ids() {
    let a = record("a", "2026-01-01");
    let b = record("b", "2026-01-01");
    let duplicate = record("b", "2026-02-01");
    let c = record("c", "2026-01-01");
    let first = [&a, &b];
    let second = [&duplicate, &c];
    let scores = reciprocal_rank_scores(&[&first, &second], 60.0);
    assert_eq!(scores.len(), 3);
    assert!(std::ptr::eq(scores[0].0, &b));
    assert!((scores[0].1 - (1.0 / 62.0 + 1.0 / 61.0)).abs() < 1e-12);
    let fused = reciprocal_rank_fusion(&first, &second, 60.0);
    assert_eq!(
        fused
            .iter()
            .map(|record| record.template_id.as_str())
            .collect::<Vec<_>>(),
        ["b", "a", "c"]
    );
    for (actual, expected) in fused.iter().zip([&b, &a, &c]) {
        assert!(std::ptr::eq(*actual, expected));
    }
    assert!(reciprocal_rank_fusion(&[], &[], 60.0).is_empty());
}

#[test]
fn vector_ranking_and_fusion_preserve_recency_and_id_ties_without_copying_records() {
    let older = record("older", "2026-01-01");
    let b = record("b", "2026-02-01");
    let a = record("a", "2026-02-01");
    let records = [&older, &b, &a];
    let ranked = rank_by_vector(&[1.0, 0.0], &records, VectorRole::Subject).unwrap();
    for (actual, expected) in ranked.iter().zip([&a, &b, &older]) {
        assert!(std::ptr::eq(*actual, expected));
    }
    let fused = reciprocal_rank_fusion(&[&older, &b, &a], &[&a, &b, &older], 60.0);
    for (actual, expected) in fused.iter().zip([&a, &older, &b]) {
        assert!(std::ptr::eq(*actual, expected));
    }
    assert!(rank_by_vector(&[1.0], &records, VectorRole::Subject).is_err());
}
