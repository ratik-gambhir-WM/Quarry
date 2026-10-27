use serde_json::json;

use super::*;

#[test]
fn insert_many_rolls_back_every_record_when_one_insert_fails() {
    let repository =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:"))
            .expect("in-memory repository");
    let record = template_insert("duplicate-template");

    assert!(repository.insert_many(&[record.clone(), record]).is_err());
    assert!(repository
        .find_by_id("duplicate-template")
        .expect("template lookup")
        .is_none());
    assert!(repository.list(None).expect("template list").is_empty());
}

fn template_insert(template_id: &str) -> TemplateInsert {
    let template_json = serde_json::from_value(json!({
        "presentation": {
            "title": "Atomic batch",
            "preserveElementOrder": true,
            "showBranding": false,
            "slides": [{
                "id": "slide-1",
                "name": "Atomic batch",
                "width": 1280,
                "height": 720,
                "backgroundColor": "FFFFFF",
                "elements": []
            }]
        }
    }))
    .expect("valid canvas document");
    TemplateInsert {
        template: StoredTemplate {
            template_id: template_id.to_owned(),
            template_json,
        },
        assets: Vec::new(),
        metadata: StoredTemplateMetadata {
            template_id: template_id.to_owned(),
            kind: TemplateKind::Diagram,
            title: "Atomic batch".to_owned(),
            description: "Atomic batch test".to_owned(),
            source: TemplateSource::Import,
            created_at: 1,
            catalog_version: None,
            catalog_checksum: None,
        },
        preview: None,
    }
}
