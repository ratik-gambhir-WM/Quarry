use super::*;
#[test]
fn normalized_and_compact_documents_match_executed_typescript_oracle() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/slide_templates/normalization-reference.json"
    ))
    .unwrap();
    for case in cases {
        let (document, warnings) = super::super::normalize::normalize(&case["input"]).unwrap();
        assert_eq!(json!(warnings), case["normalized"]["warnings"]);
        assert_eq!(
            super::super::retrieval::stable_stringify(&normalized_document(&document)),
            super::super::retrieval::stable_stringify(&case["normalized"]["templateJson"])
        );
        assert_eq!(
            super::super::retrieval::stable_stringify(&compact_document(&document)),
            super::super::retrieval::stable_stringify(&case["compact"])
        );
    }
}
