use super::*;
use std::collections::HashSet;

#[test]
fn twenty_eight_unique_categories_map_totally_onto_five_workstreams() {
    assert_eq!(Category::ALL.len(), 28);
    let ids: HashSet<_> = Category::ALL.iter().map(|c| c.id()).collect();
    assert_eq!(ids.len(), 28);
    assert!(!ids.contains(UNCLASSIFIED));
    let labels: HashSet<_> = Category::ALL.iter().map(|c| c.label()).collect();
    assert_eq!(labels.len(), 28);
    let workstreams: HashSet<_> = Category::ALL.iter().map(|c| c.workstream()).collect();
    assert_eq!(workstreams.len(), Workstream::ALL.len());
    for workstream in Workstream::ALL {
        let members = Category::ALL
            .iter()
            .filter(|c| c.workstream() == workstream)
            .count();
        let expected = if workstream == Workstream::TransactionDiligenceManagement {
            4
        } else {
            6
        };
        assert_eq!(members, expected, "{}", workstream.id());
    }
}

#[test]
fn wire_ids_round_trip_through_serde_and_lookup() {
    for category in Category::ALL {
        assert_eq!(Category::from_id(category.id()), Some(category));
        assert_eq!(
            serde_json::to_value(category).unwrap(),
            serde_json::Value::String(category.id().into())
        );
        assert!(category
            .id()
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'_'));
        assert!(category
            .choice_description()
            .contains(category.workstream().label()));
        assert_eq!(
            category.coverage_question(),
            format!("covers_{}", category.id())
        );
    }
    for workstream in Workstream::ALL {
        assert_eq!(
            serde_json::to_value(workstream).unwrap(),
            serde_json::Value::String(workstream.id().into())
        );
    }
    assert_eq!(Category::from_id(UNCLASSIFIED), None);
    assert_eq!(Category::from_id("Product_Security"), None);
}

#[test]
fn labels_keep_the_requested_taxonomy_text() {
    assert_eq!(
        Category::Sdlc.label(),
        "Software Development Lifecycle (SDLC)"
    );
    assert_eq!(
        Category::RemediationInvestmentPlanning.label(),
        "Remediation Projects & Investment Planning"
    );
    assert_eq!(Workstream::DataAi.label(), "Data & AI");
}
