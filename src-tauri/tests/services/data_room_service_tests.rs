use super::*;

use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    repository::deal_repository::{create_deal, CreateDealRecord},
    state::AppState,
};

const ACTUAL_PDF_RELATIVE_PATH: &str = "4 Security and Compliance/4.1 Cybersecurity/4.1.2 Cybersecurity testing and remediation/BetaNXT Standard - Application Security Testing.pdf";

#[test]
fn every_mock_deal_has_a_configured_data_room_root() {
    let state = AppState::new_for_test().expect("test state should initialize");
    for deal_id in ["project-alpha", "project-beta", "logistics-merger"] {
        assert!(deal_data_room_root(&state, deal_id).unwrap().is_some());
    }
}

#[test]
fn actual_pdf_fixture_builds_a_native_preview() {
    let state = AppState::new_for_test().expect("test state should initialize");
    let fixture = deal_data_room_root(&state, "project-alpha")
        .unwrap()
        .unwrap()
        .join(ACTUAL_PDF_RELATIVE_PATH);
    if !fixture.is_file() {
        return;
    }

    let preview = build_document_preview(&state, "project-alpha", ACTUAL_PDF_RELATIVE_PATH)
        .expect("actual PDF fixture should build a preview");
    let bytes = general_purpose::STANDARD
        .decode(preview.pdf_base64)
        .expect("preview should contain valid base64");

    assert_eq!(preview.mime_type, "application/pdf");
    assert_eq!(preview.source_kind, "native");
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn persisted_deal_root_is_used_without_returning_the_absolute_path() {
    let state = AppState::new_for_test().expect("test state should initialize");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("quarry-data-room-{unique}"));
    fs::create_dir_all(&root).expect("fixture root should be created");
    fs::write(root.join("readme.pdf"), b"%PDF-1.4\n%%EOF").expect("fixture PDF should be created");

    let deal = create_deal(
        &state,
        CreateDealRecord {
            deal_name: "Persisted Root",
            main_data_room_folder: root.to_str().expect("fixture path should be UTF-8"),
            deal_type: "Sell-side",
            pe_firm: "West Monroe Capital",
            target_company: Some("Target Co"),
            buyer_or_platform_company: None,
            parent_or_seller_company: None,
            carve_out_business: None,
        },
    )
    .expect("fixture deal should be created");

    let listing = list_deal_data_room(&state, deal.id.to_string())
        .expect("stored data-room root should reopen");

    assert_eq!(listing.deal_id, deal.id.to_string());
    assert_eq!(
        listing.root_name,
        root.file_name().unwrap().to_string_lossy()
    );
    assert_eq!(listing.root_path, listing.root_name);
    assert_ne!(listing.root_path, root.display().to_string());
    assert_eq!(listing.tree.len(), 1);

    fs::remove_dir_all(root).expect("fixture root should be removed");
}
