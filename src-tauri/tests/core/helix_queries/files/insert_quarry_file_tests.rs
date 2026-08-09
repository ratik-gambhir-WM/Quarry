use std::collections::BTreeMap;

use super::*;

fn document() -> DocumentNode {
    DocumentNode {
        document_id: "doc_7f9c2d41-6e83-4a15-b287-934b62e01c55".to_string(),
        user_id: "user_24a871b3-905c-4c21-a0d6-81df54ae7729".to_string(),
        file_name: "Project_Aurora_Stock_Purchase_Agreement.pdf".to_string(),
        source_type: "pdf".to_string(),
        local_path: Some(
            "/Users/demo/Quarry/data-room/Project Aurora/Legal/Project_Aurora_Stock_Purchase_Agreement.pdf"
                .to_string(),
        ),
        file_size_bytes: 2_847_391,
        token_count: 18_642,
        content_hash:
            "d4f7b13a85790b36f06b9d50f4c1d02abff8aa30201ca03e87350f86d72a50c4"
                .to_string(),
        rendered_pdf_path: Some(
            "/Users/demo/Quarry/previews/project-aurora-stock-purchase-agreement.pdf"
                .to_string(),
        ),
    }
}

fn chunk() -> ChunkNode {
    ChunkNode {
        chunk_id: "chunk_7f9c2d41_0042".to_string(),
        document_id: "doc_7f9c2d41-6e83-4a15-b287-934b62e01c55".to_string(),
        user_id: "user_24a871b3-905c-4c21-a0d6-81df54ae7729".to_string(),
        text: "The aggregate purchase price payable at closing is $185,000,000, subject to customary adjustments for cash, indebtedness, and net working capital."
            .to_string(),
        embedding: Some(vec![0.018, -0.227, 0.441, 0.093, -0.315, 0.172, 0.508, -0.064]),
        sequence_number: 42,
        page_numbers: Some(vec![47, 48]),
        start_offset: 18_420,
        end_offset: 18_566,
        token_count: 31,
        content_hash:
            "8f1d5c23a7b04e619aec27f903db8246c1a55e19fc35ba027e34f8bb70d9124a"
                .to_string(),
        section_title: Some("Section 2.4 — Purchase Price Adjustments".to_string()),
    }
}

#[test]
fn file_query_accepts_and_decomposes_document_node() {
    let request = insert_quarry_file(document()).unwrap();
    let parameters = request.parameters.unwrap();

    assert_eq!(
        parameters.get("document_id"),
        Some(&DynamicQueryValue::String(
            "doc_7f9c2d41-6e83-4a15-b287-934b62e01c55".to_string()
        ))
    );
    assert_eq!(
        parameters.get("file_name"),
        Some(&DynamicQueryValue::String(
            "Project_Aurora_Stock_Purchase_Agreement.pdf".to_string()
        ))
    );
    assert_eq!(
        parameters.get("local_path"),
        Some(&DynamicQueryValue::String(
            "/Users/demo/Quarry/data-room/Project Aurora/Legal/Project_Aurora_Stock_Purchase_Agreement.pdf"
                .to_string()
        ))
    );
    assert_eq!(
        parameters.get("file_size_bytes"),
        Some(&DynamicQueryValue::I64(2_847_391))
    );
    assert_eq!(
        parameters.get("token_count"),
        Some(&DynamicQueryValue::I64(18_642))
    );
    assert_eq!(
        parameters.get("content_hash"),
        Some(&DynamicQueryValue::String(
            "d4f7b13a85790b36f06b9d50f4c1d02abff8aa30201ca03e87350f86d72a50c4".to_string()
        ))
    );
}

#[test]
fn file_query_starts_incomplete_and_completion_is_explicit() {
    let insert_json = serde_json::to_string(&insert_quarry_file(document()).unwrap()).unwrap();
    assert!(insert_json.contains("ingestion_complete"));
    assert!(insert_json.contains("false"));

    let completion =
        mark_quarry_file_ingestion_complete("doc-1".to_string(), "user-1".to_string()).unwrap();
    let completion_json = serde_json::to_string(&completion).unwrap();
    assert!(completion_json.contains("ingestion_complete"));
    assert!(completion_json.contains("true"));
}

#[test]
fn chunk_query_looks_up_existing_file_and_conditionally_creates_graph() {
    let request = insert_chunk_for_document(chunk()).unwrap();
    let parameters = request.parameters.as_ref().unwrap();

    assert_eq!(
        parameters,
        &BTreeMap::from([
            (
                "chunk_id".to_string(),
                DynamicQueryValue::String("chunk_7f9c2d41_0042".to_string()),
            ),
            (
                "content_hash".to_string(),
                DynamicQueryValue::String(
                    "8f1d5c23a7b04e619aec27f903db8246c1a55e19fc35ba027e34f8bb70d9124a"
                        .to_string(),
                ),
            ),
            (
                "document_id".to_string(),
                DynamicQueryValue::String(
                    "doc_7f9c2d41-6e83-4a15-b287-934b62e01c55".to_string(),
                ),
            ),
            (
                "embedding".to_string(),
                DynamicQueryValue::Array(vec![
                    DynamicQueryValue::F32(0.018),
                    DynamicQueryValue::F32(-0.227),
                    DynamicQueryValue::F32(0.441),
                    DynamicQueryValue::F32(0.093),
                    DynamicQueryValue::F32(-0.315),
                    DynamicQueryValue::F32(0.172),
                    DynamicQueryValue::F32(0.508),
                    DynamicQueryValue::F32(-0.064),
                ]),
            ),
            ("end_offset".to_string(), DynamicQueryValue::I64(18_566)),
            (
                "page_numbers".to_string(),
                DynamicQueryValue::Array(vec![
                    DynamicQueryValue::I64(47),
                    DynamicQueryValue::I64(48),
                ]),
            ),
            (
                "section_title".to_string(),
                DynamicQueryValue::String(
                    "Section 2.4 — Purchase Price Adjustments".to_string(),
                ),
            ),
            ("sequence_number".to_string(), DynamicQueryValue::I64(42)),
            (
                "start_offset".to_string(),
                DynamicQueryValue::I64(18_420),
            ),
            (
                "text".to_string(),
                DynamicQueryValue::String(
                    "The aggregate purchase price payable at closing is $185,000,000, subject to customary adjustments for cash, indebtedness, and net working capital."
                        .to_string(),
                ),
            ),
            ("token_count".to_string(), DynamicQueryValue::I64(31)),
            (
                "user_id".to_string(),
                DynamicQueryValue::String(
                    "user_24a871b3-905c-4c21-a0d6-81df54ae7729".to_string(),
                ),
            ),
        ])
    );

    let json = serde_json::to_value(&request).unwrap();
    let queries = json["query"]["queries"].as_array().unwrap();

    assert_eq!(queries.len(), 3);
    assert_eq!(
        json["query"]["returns"],
        serde_json::json!(["quarry_file", "chunk", "quarry_file_has_chunk"])
    );
    assert_eq!(
        queries[0]["Query"],
        serde_json::json!({
            "name": "quarry_file",
            "steps": [
                {
                    "NWhere": {
                        "Eq": ["$label", {"String": QUARRY_FILE_LABEL}]
                    }
                },
                {
                    "Where": {
                        "EqExpr": [
                            "document_id",
                            {"Param": "document_id"}
                        ]
                    }
                },
                {
                    "Where": {
                        "EqExpr": ["user_id", {"Param": "user_id"}]
                    }
                }
            ],
            "condition": null
        })
    );
    assert_eq!(
        queries[1]["Query"],
        serde_json::json!({
            "name": "chunk",
            "steps": [{
                "AddN": {
                    "label": CHUNK_LABEL,
                    "properties": [
                        ["chunk_id", {"Expr": {"Param": "chunk_id"}}],
                        ["document_id", {"Expr": {"Param": "document_id"}}],
                        ["user_id", {"Expr": {"Param": "user_id"}}],
                        ["text", {"Expr": {"Param": "text"}}],
                        ["embedding", {"Expr": {"Param": "embedding"}}],
                        ["sequence_number", {"Expr": {"Param": "sequence_number"}}],
                        ["page_numbers", {"Expr": {"Param": "page_numbers"}}],
                        ["start_offset", {"Expr": {"Param": "start_offset"}}],
                        ["end_offset", {"Expr": {"Param": "end_offset"}}],
                        ["token_count", {"Expr": {"Param": "token_count"}}],
                        ["content_hash", {"Expr": {"Param": "content_hash"}}],
                        ["section_title", {"Expr": {"Param": "section_title"}}]
                    ]
                }
            }],
            "condition": {"VarNotEmpty": "quarry_file"}
        })
    );
    assert_eq!(
        queries[2]["Query"],
        serde_json::json!({
            "name": "quarry_file_has_chunk",
            "steps": [
                {"N": {"Var": "quarry_file"}},
                {
                    "AddE": {
                        "label": QUARRY_FILE_HAS_CHUNK_LABEL,
                        "to": {"Var": "chunk"},
                        "properties": [
                            ["user_id", {"Expr": {"Param": "user_id"}}]
                        ]
                    }
                }
            ],
            "condition": {"VarNotEmpty": "chunk"}
        })
    );
}

#[test]
fn index_query_contains_file_and_chunk_indexes() {
    let json = serde_json::to_value(create_document_indexes()).unwrap();
    let queries = json["query"]["queries"].as_array().unwrap();
    let expected_indexes = [
        (
            "document_id_unique",
            serde_json::json!({
                "NodeEquality": {
                    "label": QUARRY_FILE_LABEL,
                    "property": "document_id",
                    "unique": true
                }
            }),
        ),
        (
            "document_user_id",
            serde_json::json!({
                "NodeEquality": {
                    "label": QUARRY_FILE_LABEL,
                    "property": "user_id",
                    "unique": false
                }
            }),
        ),
        (
            "document_source_type",
            serde_json::json!({
                "NodeEquality": {
                    "label": QUARRY_FILE_LABEL,
                    "property": "source_type",
                    "unique": false
                }
            }),
        ),
        (
            "document_local_path",
            serde_json::json!({
                "NodeEquality": {
                    "label": QUARRY_FILE_LABEL,
                    "property": "local_path",
                    "unique": false
                }
            }),
        ),
        (
            "document_content_hash",
            serde_json::json!({
                "NodeEquality": {
                    "label": QUARRY_FILE_LABEL,
                    "property": "content_hash",
                    "unique": false
                }
            }),
        ),
        (
            "document_file_name",
            serde_json::json!({
                "NodeText": {
                    "label": QUARRY_FILE_LABEL,
                    "property": "file_name"
                }
            }),
        ),
        (
            "chunk_id_unique",
            serde_json::json!({
                "NodeEquality": {
                    "label": CHUNK_LABEL,
                    "property": "chunk_id",
                    "unique": true
                }
            }),
        ),
        (
            "chunk_document_id",
            serde_json::json!({
                "NodeEquality": {
                    "label": CHUNK_LABEL,
                    "property": "document_id",
                    "unique": false
                }
            }),
        ),
        (
            "chunk_user_id",
            serde_json::json!({
                "NodeEquality": {
                    "label": CHUNK_LABEL,
                    "property": "user_id",
                    "unique": false
                }
            }),
        ),
        (
            "chunk_embedding",
            serde_json::json!({
                "NodeVector": {
                    "label": CHUNK_LABEL,
                    "property": "embedding",
                    "tenant_property": "user_id"
                }
            }),
        ),
        (
            "chunk_text",
            serde_json::json!({
                "NodeText": {
                    "label": CHUNK_LABEL,
                    "property": "text",
                    "tenant_property": "user_id"
                }
            }),
        ),
    ];

    assert_eq!(queries.len(), expected_indexes.len());
    assert_eq!(json["query"]["returns"], serde_json::json!([]));

    for (query, (expected_name, expected_spec)) in queries.iter().zip(expected_indexes) {
        assert_eq!(query["Query"]["name"], expected_name);
        assert_eq!(query["Query"]["condition"], serde_json::Value::Null);
        assert_eq!(
            query["Query"]["steps"],
            serde_json::json!([{
                "CreateIndex": {
                    "spec": expected_spec,
                    "if_not_exists": true
                }
            }])
        );
    }
}

#[test]
fn registered_bundle_contains_document_query_routes() {
    let bundle = helix_db::query_generator::build_query_bundle().unwrap();

    for route in [
        "insert_quarry_file_route",
        "insert_chunk_for_document_route",
        "create_document_indexes",
    ] {
        assert!(bundle.write_routes.contains_key(route));
    }
}
