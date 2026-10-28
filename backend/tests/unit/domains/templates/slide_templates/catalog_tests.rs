use super::*;
#[test]
fn built_in_checksums_match_reference_catalog_bytes_and_manifest_projection() {
    let repository =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap();
    seed(&repository).unwrap();
    let checksums = [
        (
            "layered-architecture",
            "23511e822fb88cefda26f5aa1ef0b64685eef7e5b0a7360dba60f5b866c9d2a4",
        ),
        (
            "layered-platform",
            "060b482521637491bd54e72aac2f98d8f7b4ba837c1426282f61082876d3955b",
        ),
        (
            "microservice-architecture",
            "4800396661170b3bb05989025db3c13097014038075b11bf8ce11d831eabe3e8",
        ),
        (
            "multi-tenant-system",
            "453109ea895e024a61e6e191c5c633d9ef4afe1e6a75580d754cc6ac06704f24",
        ),
        (
            "multi-application-ecosystem",
            "1791801bce5a0969378ef477d0598509099cf05e41341d0cc3f1ef4394467cf1",
        ),
        (
            "product-architecture",
            "0ed0a4c52e6fff3100091a318e22ddd63585ef8cdcb023c1f141edae27048d0f",
        ),
        (
            "architecture-ssa",
            "919e6259b01d1ea8194d1e4ce8e3a97ee756412b6a3e6cb6b58a9a0e2da59612",
        ),
        (
            "security-ssa",
            "24743d996235f09d1a2ad11c656b47414d15391b235f6bbf2e48deda7b5a22f1",
        ),
        (
            "sdlc-ssa",
            "5d91bbabab85c6168fda1d21c8a2372effce299deaa452cf204b4ed0d668d8bd",
        ),
    ];
    for (id, expected) in checksums {
        assert_eq!(
            repository
                .find_metadata(id)
                .unwrap()
                .unwrap()
                .catalog_checksum
                .as_deref(),
            Some(expected)
        );
    }
}
#[test]
fn commentary_frame_removal_applies_reserved_ids_to_every_element_type() {
    let input = serde_json::json!({"slides":[{"elements":[
        {"type":"text","id":"west-monroe-dot-1","text":"remove"},
        {"type":"shape","id":"element-903000"},
        {"type":"line","id":"west-monroe-logo"},
        {"type":"image","id":"west-monroe-footer","src":"example.png"},
        {"type":"image","id":"legacy","src":"./element-5.png"},
        {"type":"text","id":"keep","text":"Content"}
    ]}]});
    let (mut document, _) = normalize::normalize(&input).unwrap();
    normalize_commentary(&mut document);
    assert_eq!(document.presentation.slides[0].elements.len(), 1);
    assert!(
        matches!(&document.presentation.slides[0].elements[0],Element::Text(text) if text.id=="keep")
    );
}
