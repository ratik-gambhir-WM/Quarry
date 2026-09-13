//! Tests for the desktop Quarry API relay service.

use super::*;

#[test]
fn restricts_proxy_paths_to_the_versioned_quarry_api() {
    assert!(validate_api_path("/api/v1/deals").is_ok());
    assert!(validate_api_path("/api/v1/templates/previews?page=1").is_ok());
    assert!(validate_api_path("/api/v1/templates/template%2Fone").is_ok());
    assert!(validate_api_path("/api/v1/templates/import?mode=single").is_ok());
    assert!(validate_api_path("/api/v1/templates/import?mode=batch").is_ok());
    assert!(validate_api_path("https://example.com/api/v1/deals").is_err());
    assert!(validate_api_path("/api/v1/../secrets").is_err());
}

#[test]
fn restricts_pdf_proxy_to_deal_document_preview_routes() {
    assert!(validate_pdf_api_path("/api/v1/deals/DEAL-1/documents/file-1/pdf").is_ok());
    assert!(validate_pdf_api_path("/api/v1/deals").is_err());
    assert!(validate_pdf_api_path("/api/v1/deals/DEAL-1/documents/file-1").is_err());
    assert!(validate_pdf_api_path("/api/v1/users/example/pdf").is_err());
}
