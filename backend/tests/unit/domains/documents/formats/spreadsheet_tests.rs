use super::*;

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
