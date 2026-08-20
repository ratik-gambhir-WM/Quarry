use super::*;

fn input(deal_type: &str) -> SaveDealAndExtractInput {
    SaveDealAndExtractInput {
        deal_name: "Project Test".to_string(),
        main_data_room_folder: "/tmp".to_string(),
        deal_type: deal_type.to_string(),
        pe_firm: "Test Capital".to_string(),
        target_company: Some("Target".to_string()),
        buyer_or_platform_company: Some("Buyer".to_string()),
        parent_or_seller_company: Some("Seller".to_string()),
        carve_out_business: Some("Division".to_string()),
    }
}

#[test]
fn matches_source_file_names() {
    assert_eq!(
        matching_terms("Admin/SOW and Project Timeline.docx").len(),
        2
    );
    assert!(matching_terms("financials.xlsx").is_empty());
}

#[test]
fn validates_supported_deal_types_and_fields() {
    assert!(validate_deal_input(&input("Buy-side"), true).is_ok());
    assert!(validate_deal_input(&input("Unknown"), true).is_err());
    let mut invalid = input("Carve-out");
    invalid.carve_out_business = None;
    assert!(validate_deal_input(&invalid, true).is_err());
}

#[test]
fn parses_fenced_extraction_json() {
    let extraction = parse_deal_extraction("```json\n{\"keyQuestions\":[\"Why?\"]}\n```").unwrap();
    assert_eq!(extraction.key_questions, vec!["Why?"]);
}
