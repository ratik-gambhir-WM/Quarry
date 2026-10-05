use std::io::{Cursor, Write};

use super::*;

#[test]
fn accepts_matching_package_roots_and_content_type_markers() {
    for (kind, root, marker) in [
        (
            OfficeDocumentKind::Word,
            "word/document.xml",
            "wordprocessingml.document",
        ),
        (
            OfficeDocumentKind::PowerPoint,
            "ppt/presentation.xml",
            "presentationml.presentation",
        ),
        (
            OfficeDocumentKind::Spreadsheet,
            "xl/workbook.xml",
            "spreadsheetml.sheet",
        ),
    ] {
        assert!(validate_office_package(&office_zip(root, marker), kind).is_ok());
    }
}

#[test]
fn rejects_invalid_roots_markers_and_declared_expansion_limits() {
    assert!(validate_office_package(
        &office_zip("ppt/presentation.xml", "presentationml.presentation"),
        OfficeDocumentKind::Spreadsheet,
    )
    .is_err());
    assert!(validate_office_package(b"not an archive", OfficeDocumentKind::Spreadsheet).is_err());

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer
            .write_all(b"<Types>spreadsheetml.sheet</Types>")
            .unwrap();
        writer.start_file("xl/workbook.xml", options).unwrap();
        writer.write_all(b"<workbook/>").unwrap();
        writer.start_file("xl/oversized.bin", options).unwrap();
        writer
            .write_all(&vec![
                0_u8;
                usize::try_from(MAX_OFFICE_ZIP_ENTRY_BYTES + 1)
                    .unwrap()
            ])
            .unwrap();
        writer.finish().unwrap();
    }
    assert!(validate_office_package(&bytes.into_inner(), OfficeDocumentKind::Spreadsheet).is_err());
}

fn office_zip(root: &str, marker: &str) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        write!(writer, "<Types>{marker}</Types>").unwrap();
        writer.start_file(root, options).unwrap();
        writer.write_all(b"<xml/>").unwrap();
        writer.finish().unwrap();
    }
    bytes.into_inner()
}
