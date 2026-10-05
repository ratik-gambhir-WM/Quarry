use super::*;
use std::io::{Cursor, Write};

#[test]
fn formats_rows_as_ordered_text() {
    let rows = vec![
        SpreadsheetRow {
            sheet_name: "Sheet1".to_string(),
            row_number: 1,
            values: vec!["Name".to_string(), "Amount".to_string()],
        },
        SpreadsheetRow {
            sheet_name: "Sheet1".to_string(),
            row_number: 2,
            values: vec!["".to_string(), "".to_string()],
        },
        SpreadsheetRow {
            sheet_name: "Sheet1".to_string(),
            row_number: 3,
            values: vec!["Acme".to_string(), "42".to_string()],
        },
    ];

    let text = rows_to_text(rows);

    assert_eq!(text, "Sheet1 row 1: Name\tAmount\nSheet1 row 3: Acme\t42");
}

#[test]
fn converts_cells_to_strings() {
    assert_eq!(cell_to_string(&Data::Empty), "");
    assert_eq!(cell_to_string(&Data::String("hello".to_string())), "hello");
    assert_eq!(cell_to_string(&Data::Float(1.25)), "1.25");
    assert_eq!(cell_to_string(&Data::Int(7)), "7");
    assert_eq!(cell_to_string(&Data::Bool(true)), "true");
    assert_eq!(
        cell_to_string(&Data::DateTimeIso("2026-05-10T00:00:00".to_string())),
        "2026-05-10T00:00:00"
    );
    assert_eq!(
        cell_to_string(&Data::DurationIso("PT1H".to_string())),
        "PT1H"
    );
}

#[test]
fn parses_xlsx_bytes_into_a_pathless_graph_ready_assembly() {
    let bytes = xlsx_bytes();
    let text = parse_spreadsheet_from_bytes(&bytes).unwrap();
    let assembly = parse_spreadsheet_chunks_from_bytes(&bytes, None, "user-1").unwrap();

    assert_eq!(
        text,
        "Overview row 1: Name\tAmount\nOverview row 2: Acme\t42\nNotes row 1: Follow up"
    );
    assert_eq!(assembly.document.file_name, "Document.xlsx");
    assert_eq!(assembly.document.source_type, "xlsx");
    assert!(assembly.document.local_path.is_none());
    assert_eq!(assembly.document.file_size_bytes, bytes.len() as u64);
    assert_eq!(assembly.document.content_hash, sha256_bytes(&bytes));
    assert_eq!(
        assembly
            .chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>(),
        text,
    );
    assert_eq!(assembly.chunks[0].section_title.as_deref(), None);
    assert_eq!(assembly.chunks[0].start_offset, 0);
    assert_eq!(assembly.chunks.last().unwrap().end_offset, text.len());
}

#[test]
fn rejects_empty_invalid_and_textless_xlsx_bytes() {
    assert_eq!(
        parse_spreadsheet_from_bytes(b""),
        Err("invalid Excel workbook".to_string())
    );
    assert_eq!(
        parse_spreadsheet_from_bytes(b"not an archive"),
        Err("invalid Excel workbook".to_string())
    );

    let bytes = xlsx_bytes_with_contents("", "");
    assert_eq!(
        parse_spreadsheet_from_bytes(&bytes),
        Err("XLSX did not contain readable text".to_string())
    );
}

fn xlsx_bytes() -> Vec<u8> {
    xlsx_bytes_with_contents("<row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>Name</t></is></c><c r=\"B1\" t=\"inlineStr\"><is><t>Amount</t></is></c></row><row r=\"2\"><c r=\"A2\" t=\"inlineStr\"><is><t>Acme</t></is></c><c r=\"B2\"><v>42</v></c></row>", "Follow up")
}

fn xlsx_bytes_with_contents(cells: &str, notes: &str) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/worksheets/sheet2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>").unwrap();
        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>").unwrap();
        zip.start_file("xl/workbook.xml", options).unwrap();
        zip.write_all(b"<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Overview\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"Notes\" sheetId=\"2\" r:id=\"rId2\"/></sheets></workbook>").unwrap();
        zip.start_file("xl/_rels/workbook.xml.rels", options)
            .unwrap();
        zip.write_all(b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet2.xml\"/></Relationships>").unwrap();
        zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
        zip.write_all(format!("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>{cells}</sheetData></worksheet>").as_bytes()).unwrap();
        zip.start_file("xl/worksheets/sheet2.xml", options).unwrap();
        zip.write_all(format!("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>{notes}</t></is></c></row></sheetData></worksheet>").as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    bytes.into_inner()
}
