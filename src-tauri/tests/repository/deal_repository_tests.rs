use super::*;

fn create_fixture_deal(state: &AppState, name: &str) -> Deal {
    create_deal(
        state,
        CreateDealRecord {
            deal_name: name,
            main_data_room_folder: "/tmp/quarry-fixture",
            deal_type: "Sell-side",
            pe_firm: "West Monroe Capital",
            target_company: Some("Target Co"),
            buyer_or_platform_company: None,
            parent_or_seller_company: None,
            carve_out_business: None,
        },
    )
    .expect("fixture deal should be created")
}

#[test]
fn list_and_get_deals_include_metadata_without_archived_rows() {
    let state = AppState::new_for_test().expect("test state should initialize");
    let first = create_fixture_deal(&state, "First Deal");
    let second = create_fixture_deal(&state, "Second Deal");

    upsert_deal_metadata(
        &state,
        UpsertDealMetadataRecord {
            deal_id: second.id,
            key_questions_json: r#"["What changed?"]"#,
            legacy_investment_thesis: Some("Legacy desktop thesis"),
            document_count: 4,
            data_room_size_bytes: 4096,
        },
    )
    .expect("metadata should be persisted");

    archive_deal(&state, first.id).expect("archive should succeed");

    let active = list_deals(&state).expect("active deals should load");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].deal.id, second.id);
    let metadata = active[0]
        .metadata
        .as_ref()
        .expect("metadata should be returned with the deal");
    assert_eq!(metadata.key_questions_json, r#"["What changed?"]"#);
    assert_eq!(metadata.document_count, 4);
    assert_eq!(
        metadata.legacy_investment_thesis.as_deref(),
        Some("Legacy desktop thesis")
    );

    let fetched = get_deal_with_metadata(&state, second.id)
        .expect("deal lookup should succeed")
        .expect("deal should exist");
    assert_eq!(fetched.deal.deal_name, "Second Deal");
    assert!(fetched.metadata.is_some());
}

#[test]
fn archiving_is_idempotent_and_does_not_delete_the_deal() {
    let state = AppState::new_for_test().expect("test state should initialize");
    let deal = create_fixture_deal(&state, "Archive Me");

    let archived = archive_deal(&state, deal.id)
        .expect("first archive should succeed")
        .expect("deal should exist");
    assert_eq!(archived.status, "archived");

    let archived_again = archive_deal(&state, deal.id)
        .expect("second archive should succeed")
        .expect("deal should still exist");
    assert_eq!(archived_again.status, "archived");
    assert!(get_deal_by_id(&state, deal.id)
        .expect("lookup should succeed")
        .is_some());
}
