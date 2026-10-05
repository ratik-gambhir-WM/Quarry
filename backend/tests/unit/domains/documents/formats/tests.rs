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

#[test]
fn upload_dispatch_selects_modern_office_formats_case_insensitively() {
    let spreadsheet = QuarryFile::from_bytes("model.XLSX", b"bytes".to_vec()).unwrap();
    let presentation = QuarryFile::from_bytes("board.PpTx", b"bytes".to_vec()).unwrap();

    assert!(matches!(
        spreadsheet,
        QuarryFile::Spreadsheet { path: None, .. }
    ));
    assert!(matches!(
        presentation,
        QuarryFile::Powerpoint { path: None, .. }
    ));
    assert!(QuarryFile::from_bytes("legacy.xls", b"bytes".to_vec()).is_err());
    assert!(QuarryFile::from_bytes("legacy.ppt", b"bytes".to_vec()).is_err());
    assert!(QuarryFile::from_bytes("notes.txt", b"bytes".to_vec()).is_err());
}
