//! Differential fixtures generated with diligence-studio's unmodified TypeScript converter.
use crate::domains::templates::slide_templates::{
    compact_document, pptx, ArrowType, CanvasDocument, ElbowDirection, Element, LineType,
};
use serde_json::Value;

#[test]
fn imported_drawables_match_reference_typography_geometry_and_visibility() {
    let imported = pptx::read(include_bytes!(
        "../../fixtures/slide_templates/pptx/styled.pptx"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../fixtures/slide_templates/pptx/styled.reference.json"
    ))
    .unwrap();
    let actual = serde_json::json!({"templateJson": compact_document(&imported.template_json), "warnings": imported.warnings});
    assert_reference_fields(&actual, &expected, "$");
}

// JSON has one number type. Compare 1 and 1.0 numerically while enforcing exact
// object keys, array ordering, and every compact field value.
fn assert_reference_fields(actual: &Value, expected: &Value, path: &str) {
    match expected {
        Value::Object(values) => {
            assert_eq!(
                actual.as_object().unwrap().keys().collect::<Vec<_>>(),
                values.keys().collect::<Vec<_>>(),
                "{path}: object fields"
            );
            for (key, expected) in values {
                assert_reference_fields(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        Value::Array(values) => {
            let actual = actual
                .as_array()
                .unwrap_or_else(|| panic!("{path}: expected array"));
            assert_eq!(actual.len(), values.len(), "{path}: array length");
            for (index, expected) in values.iter().enumerate() {
                assert_reference_fields(&actual[index], expected, &format!("{path}[{index}]"));
            }
        }
        Value::Number(value) => {
            let expected = value.as_f64().unwrap();
            let actual = actual
                .as_f64()
                .unwrap_or_else(|| panic!("{path}: expected numeric {expected}, got {actual}"));
            assert!(
                (actual - expected).abs() < 0.0000001,
                "{path}: {actual} != {expected}"
            );
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn writer_round_trip_preserves_vertical_first_elbow_arrowheads() {
    let document: CanvasDocument = serde_json::from_value(serde_json::json!({
        "presentation": {"title":"Elbow", "preserveElementOrder":true,"showBranding":false,
            "slides":[{"id":"slide-1","name":"Elbow","width":1280,"height":720,"backgroundColor":"FFFFFF",
                "elements":[{"type":"line","id":"vertical-first","lineType":"elbow","elbowDirection":"vertical-first",
                    "x1":100,"y1":100,"x2":300,"y2":180,"stroke":"334155","strokeWidth":1,
                    "dash":"solid","beginArrow":"diamond","endArrow":"triangle"}]}]}
    })).unwrap();
    let bytes = pptx::write(&document).unwrap();
    let imported = pptx::read(&bytes).unwrap();
    let Element::Line(line) = &imported.template_json.presentation.slides[0].elements[0] else {
        panic!("line expected")
    };
    assert_eq!(line.line_type, LineType::Elbow);
    assert_eq!(line.elbow_direction, ElbowDirection::HorizontalFirst);
    assert_eq!(
        (line.x1, line.y1, line.x2, line.y2),
        (300.0, 180.0, 100.0, 100.0)
    );
    assert_eq!(line.begin_arrow, Some(ArrowType::Triangle));
    assert_eq!(line.end_arrow, Some(ArrowType::Diamond));
}

#[test]
fn inherited_decorations_placeholder_properties_and_nested_transforms_match_reference() {
    for (name, bytes, reference) in [
        (
            "table",
            include_bytes!("../../fixtures/slide_templates/pptx/table.pptx").as_slice(),
            include_str!("../../fixtures/slide_templates/pptx/table.reference.json"),
        ),
        (
            "fallback-warnings",
            include_bytes!("../../fixtures/slide_templates/pptx/fallback-warnings.pptx").as_slice(),
            include_str!("../../fixtures/slide_templates/pptx/fallback-warnings.reference.json"),
        ),
        (
            "empty",
            include_bytes!("../../fixtures/slide_templates/pptx/empty.pptx").as_slice(),
            include_str!("../../fixtures/slide_templates/pptx/empty.reference.json"),
        ),
        (
            "inherited",
            include_bytes!("../../fixtures/slide_templates/pptx/inherited.pptx").as_slice(),
            include_str!("../../fixtures/slide_templates/pptx/inherited.reference.json"),
        ),
        (
            "hidden-master",
            include_bytes!("../../fixtures/slide_templates/pptx/hidden-master.pptx").as_slice(),
            include_str!("../../fixtures/slide_templates/pptx/hidden-master.reference.json"),
        ),
        (
            "nested-transform",
            include_bytes!("../../fixtures/slide_templates/pptx/nested-transform.pptx").as_slice(),
            include_str!("../../fixtures/slide_templates/pptx/nested-transform.reference.json"),
        ),
    ] {
        let expected: Value = serde_json::from_str(reference).unwrap();
        let imported = pptx::read(bytes).unwrap();
        let actual = serde_json::json!({"templateJson": compact_document(&imported.template_json), "warnings": imported.warnings});
        assert_reference_fields(&actual, &expected, name);
    }
}

#[test]
fn package_validation_matches_zip64_multidisk_path_and_expansion_rejections() {
    let original = include_bytes!("../../fixtures/slide_templates/pptx/styled.pptx");
    pptx::validate_package(original).unwrap();
    let eocd = original
        .windows(4)
        .rposition(|bytes| bytes == b"PK\x05\x06")
        .unwrap();
    let entry = original
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap();
    for (offset, replacement, status, code) in [
        (eocd + 10, vec![255, 255], 422, "invalid_powerpoint"),
        (eocd + 4, vec![1, 0], 422, "invalid_powerpoint"),
        (entry + 46, vec![b'/'], 422, "invalid_powerpoint"),
        (
            entry + 24,
            (50_u32 * 1024 * 1024 + 1).to_le_bytes().to_vec(),
            413,
            "powerpoint_too_large",
        ),
    ] {
        let mut bytes = original.to_vec();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        let error = pptx::validate_package(&bytes).unwrap_err();
        assert_eq!(
            error.kind,
            if status == 413 {
                crate::domains::templates::slide_templates::FailureKind::TooLarge
            } else {
                crate::domains::templates::slide_templates::FailureKind::Invalid
            }
        );
        assert_eq!(error.code, code);
    }
    assert_eq!(
        pptx::validate_package(b"PK").unwrap_err().code,
        "invalid_powerpoint"
    );
}

#[test]
fn branding_uses_inherited_layout_and_image_names_do_not_collide_across_duplicate_slide_ids() {
    use crate::domains::templates::slide_templates::{ImageElement, ImageFit};
    use std::io::{Cursor, Read};
    let mut document = pptx::read(include_bytes!(
        "../../fixtures/slide_templates/pptx/styled.pptx"
    ))
    .unwrap()
    .template_json;
    document.presentation.show_branding = true;
    document.presentation.slides[0]
        .elements
        .push(Element::Image(ImageElement {
            id: "image".to_owned(),
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            src: format!(
                "data:image/png;base64,{}",
                include_str!(
                "../../../src/domains/templates/slide_templates/pptx/assets/west_monroe_logo.b64"
            )
                .trim()
            ),
            fit: ImageFit::Stretch,
            crop: None,
            alt_text: "Image".to_owned(),
            opacity: 1.0,
            rotate: 0.0,
            flip_h: false,
            flip_v: false,
            border_radius: 0.0,
        }));
    document
        .presentation
        .slides
        .push(document.presentation.slides[0].clone());
    let bytes = pptx::write(&document).unwrap();
    let mut archive = zip_slide::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut layout = String::new();
    archive
        .by_name("ppt/slideLayouts/slideLayout1.xml")
        .unwrap()
        .read_to_string(&mut layout)
        .unwrap();
    assert!(layout.contains("brand-frame-"));
    for index in 1..=2 {
        let mut xml = String::new();
        archive
            .by_name(&format!("ppt/slides/slide{index}.xml"))
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(!xml.contains("brand-frame-"));
        assert!(archive
            .by_name(&format!("ppt/media/slide-{index}-image-1.png"))
            .is_ok());
    }
    let imported = pptx::read(&bytes).unwrap();
    assert_eq!(imported.template_json.presentation.slides.len(), 2);
    assert!(imported
        .template_json
        .presentation
        .slides
        .iter()
        .all(|slide| slide.elements.len() == 262));
}

#[test]
fn generated_powerpoint_round_trip_matches_reference_exporter() {
    let imported = pptx::read(include_bytes!(
        "../../fixtures/slide_templates/pptx/styled.pptx"
    ))
    .unwrap();
    let bytes = pptx::write(&imported.template_json).unwrap();
    let exported = pptx::read(&bytes).unwrap();
    let actual = serde_json::json!({"templateJson":compact_document(&exported.template_json), "warnings":exported.warnings});
    let reference = serde_json::from_str(include_str!(
        "../../fixtures/slide_templates/pptx/styled.export-reference.json"
    ))
    .unwrap();
    assert_reference_fields(&actual, &reference, "exported");
}
