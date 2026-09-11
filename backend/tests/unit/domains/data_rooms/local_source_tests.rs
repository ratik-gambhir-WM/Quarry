//! Tests for local data-room source validation.

use super::*;

#[test]
fn validates_pdf_bytes_in_one_shared_boundary() {
    assert!(validate_pdf_bytes(b"%PDF-1.4\n", "the PDF").is_ok());
    assert_eq!(
        validate_pdf_bytes(b"not a PDF", "the PDF").unwrap_err(),
        "the PDF does not contain a valid PDF header"
    );
    assert_eq!(
        validate_pdf_bytes(&[], "the PDF").unwrap_err(),
        "the PDF is empty"
    );
}
