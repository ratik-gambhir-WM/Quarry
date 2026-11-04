use super::*;
use reqwest::header::HeaderMap;

fn request() -> MultipartRequest {
    MultipartRequest {
        path: SINGLE.into(),
        fields: vec![],
        files: vec![super::super::models::MultipartFile {
            field_name: "files".into(),
            filename: "Template.PPTX".into(),
            mime_type: POWERPOINT_CONTENT_TYPE.into(),
            data_base64: general_purpose::STANDARD.encode(b"pptx"),
        }],
    }
}
#[test]
fn classified_upload_has_exact_routes_and_one_bounded_pptx() {
    assert_eq!(import_mode(SINGLE), Some("single"));
    assert_eq!(import_mode(BATCH), Some("batch"));
    assert_eq!(import_mode(&format!("{SINGLE}?appId=other")), None);
    assert_eq!(validate_upload(request()).unwrap(), b"pptx");
    let mut invalid = request();
    invalid.files.push(invalid.files[0].clone());
    assert!(validate_upload(invalid).is_err());
    let mut invalid = request();
    invalid.files[0].filename = "../test.pptx".into();
    assert!(validate_upload(invalid).is_err());
    let mut invalid = request();
    invalid.files[0].mime_type = "text/plain".into();
    assert!(validate_upload(invalid).is_err());
    let mut invalid = request();
    invalid.files[0].data_base64.clear();
    assert!(validate_upload(invalid).is_err());
    let mut invalid = request();
    invalid.files[0].data_base64 = "A".repeat(MAX_ENCODED_BYTES + 1);
    assert!(validate_upload(invalid).is_err());
}
#[test]
fn classified_acknowledgement_requires_saved_preview_or_deck_count() {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/json".parse().unwrap());
    headers.insert("x-template-id", "template-1".parse().unwrap());
    headers.insert("x-template-preview-status", "ready".parse().unwrap());
    headers.insert("x-powerpoint-warning-count", "0".parse().unwrap());
    let single = acknowledgement(201, &headers, "single").unwrap();
    assert_eq!(single["importedCount"], 1);
    assert!(acknowledgement(200, &headers, "single").is_err());
    assert!(acknowledgement(201, &headers, "batch").is_err());
    headers.insert("x-imported-template-count", "12".parse().unwrap());
    assert_eq!(
        acknowledgement(201, &headers, "batch").unwrap()["importedCount"],
        12
    );
    headers.insert("x-imported-template-count", "101".parse().unwrap());
    assert!(acknowledgement(201, &headers, "batch").is_err());
    headers.insert("x-template-preview-status", "unavailable".parse().unwrap());
    assert!(acknowledgement(201, &headers, "single").is_err());
}
