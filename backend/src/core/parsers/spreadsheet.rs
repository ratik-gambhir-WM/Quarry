use std::path::Path;

use anyhow::Result;
use calamine::{open_workbook_auto, Data, Reader};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpreadsheetRow {
    sheet_name: String,
    row_number: usize,
    values: Vec<String>,
}

pub fn parse_spreadsheet(path: impl AsRef<Path>) -> Result<String> {
    let rows = parse_spreadsheet_rows(path)?;
    Ok(rows_to_text(rows))
}

fn rows_to_text(rows: Vec<SpreadsheetRow>) -> String {
    rows.iter()
        .map(row_to_text)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_spreadsheet_rows(path: impl AsRef<Path>) -> Result<Vec<SpreadsheetRow>> {
    let mut workbook = open_workbook_auto(path)?;
    let mut rows = Vec::new();

    for sheet_name in workbook.sheet_names().to_owned() {
        let range = workbook.worksheet_range(&sheet_name)?;

        for (index, row) in range.rows().enumerate() {
            rows.push(SpreadsheetRow {
                sheet_name: sheet_name.clone(),
                row_number: index + 1,
                values: row.iter().map(cell_to_string).collect(),
            });
        }
    }

    Ok(rows)
}

fn row_to_text(row: &SpreadsheetRow) -> String {
    let values = row
        .values
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\t");

    if values.trim().is_empty() {
        return String::new();
    }

    format!("{} row {}: {}", row.sheet_name, row.row_number, values)
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => "".to_string(),
        Data::String(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("ERROR: {:?}", e),
    }
}

#[cfg(test)]
#[path = "../../../tests/core/parsers/spreadsheet_tests.rs"]
mod tests;
