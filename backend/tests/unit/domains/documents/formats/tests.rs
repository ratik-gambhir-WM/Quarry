use std::io::Cursor;

use docx_rust::{document::Paragraph, Docx};

use super::*;

#[test]
fn uploaded_docx_keeps_filename_without_a_local_path() {
    let mut docx = Docx::default();
    docx.document
        .push(Paragraph::default().push_text("Generated Quarry report."));
    let bytes = docx.write(Cursor::new(Vec::new())).unwrap().into_inner();

    let parsed = QuarryFile::from_bytes("report.docx", bytes)
        .unwrap()
        .parse("user-1")
        .unwrap();
    let ParsedQuarryFile::Docx(assembly) = parsed else {
        panic!("expected DOCX assembly");
    };

    assert_eq!(assembly.document.file_name, "report.docx");
    assert_eq!(assembly.document.user_id, "user-1");
    assert!(assembly.document.local_path.is_none());
    assert_eq!(assembly.chunks[0].text, "Generated Quarry report.");
}

#[test]
fn parser_requires_user_scope() {
    let error = QuarryFile::from_bytes("report.pdf", b"not a pdf".to_vec())
        .unwrap()
        .parse("")
        .unwrap_err();

    assert_eq!(error, "user_id cannot be empty");
}
