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
