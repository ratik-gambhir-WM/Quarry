use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    domains::documents::{
        formats::text_chunking::token_bounded_ranges,
        model::{Document, DocumentChunk},
    },
    shared::{
        ids::document_id_from_content,
        office_zip::{validate_office_package, OfficeDocumentKind},
    },
};

#[derive(Debug, Clone)]
pub struct SpreadsheetAssembly {
    pub document: Document,
    pub chunks: Vec<DocumentChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpreadsheetRow {
    sheet_name: String,
    row_number: usize,
    values: Vec<String>,
}

#[derive(Debug, Clone)]
struct SheetTextRange {
    name: String,
    start_offset: usize,
    end_offset: usize,
}

/// Parses an XLSX package from borrowed upload bytes.
pub fn parse_spreadsheet_from_bytes(bytes: &[u8]) -> Result<String, String> {
    validate_office_package(bytes, OfficeDocumentKind::Spreadsheet)
        .map_err(|_| "invalid Excel workbook".to_string())?;
    let rows = parse_spreadsheet_rows_from_bytes(bytes)?;
    let text = rows_to_text(rows);
    if text.trim().is_empty() {
        Err("XLSX did not contain readable text".to_string())
    } else {
        Ok(text)
    }
}

/// Compatibility wrapper for callers that intentionally parse a local XLSX.
pub fn parse_spreadsheet(path: impl AsRef<Path>) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|_| "failed to read Excel workbook".to_string())?;
    parse_spreadsheet_from_bytes(&bytes)
}

/// Parses and chunks raw XLSX bytes into a graph-ready assembly. Browser
/// uploads pass `None` for `path`, so no server-local path is invented.
pub fn parse_spreadsheet_chunks_from_bytes(
    bytes: &[u8],
    path: Option<&Path>,
    user_id: impl Into<String>,
) -> Result<SpreadsheetAssembly, String> {
    let file_size_bytes = u64::try_from(bytes.len())
        .map_err(|_| "XLSX byte length does not fit in u64".to_string())?;
    let content_hash = sha256_bytes(bytes);
    validate_office_package(bytes, OfficeDocumentKind::Spreadsheet)
        .map_err(|_| "invalid Excel workbook".to_string())?;
    let rows = parse_spreadsheet_rows_from_bytes(bytes)?;
    let (text, sheet_ranges) = rows_to_text_with_sheet_ranges(rows);
    if text.trim().is_empty() {
        return Err("XLSX did not contain readable text".to_string());
    }
    let source_path = path.map(|path| path.canonicalize().unwrap_or_else(|_| PathBuf::from(path)));
    let user_id = user_id.into();
    let document_id = document_id_from_content(&user_id, &content_hash);
    let chunks = chunk_nodes_from_text(&text, &sheet_ranges, &document_id, &user_id);
    let token_count = chunks
        .iter()
        .map(|chunk| u64::from(chunk.token_count))
        .sum();

    Ok(SpreadsheetAssembly {
        document: Document {
            file_id: Uuid::new_v4().to_string(),
            document_id,
            user_id,
            file_name: source_path
                .as_deref()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or("Document.xlsx")
                .to_string(),
            source_type: "xlsx".to_string(),
            local_path: source_path.map(|path| path.to_string_lossy().into_owned()),
            file_size_bytes,
            token_count,
            content_hash,
            rendered_pdf_path: None,
        },
        chunks,
    })
}

fn parse_spreadsheet_rows_from_bytes(bytes: &[u8]) -> Result<Vec<SpreadsheetRow>, String> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes))
        .map_err(|_| "invalid Excel workbook".to_string())?;
    let mut rows = Vec::new();

    for sheet_name in workbook.sheet_names().to_owned() {
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|_| "invalid Excel workbook".to_string())?;
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

fn rows_to_text(rows: Vec<SpreadsheetRow>) -> String {
    rows_to_text_with_sheet_ranges(rows).0
}

fn rows_to_text_with_sheet_ranges(rows: Vec<SpreadsheetRow>) -> (String, Vec<SheetTextRange>) {
    let mut text = String::new();
    let mut ranges = Vec::new();
    let mut current_sheet: Option<SheetTextRange> = None;

    for row in rows {
        let row_text = row_to_text(&row);
        if row_text.trim().is_empty() {
            continue;
        }
        if current_sheet
            .as_ref()
            .is_some_and(|range| range.name != row.sheet_name)
        {
            if let Some(range) = current_sheet.take() {
                ranges.push(range);
            }
        }
        if !text.is_empty() {
            text.push('\n');
        }
        let row_start = text.len();
        text.push_str(&row_text);
        let row_end = text.len();
        match &mut current_sheet {
            Some(range) => range.end_offset = row_end,
            None => {
                current_sheet = Some(SheetTextRange {
                    name: row.sheet_name,
                    start_offset: row_start,
                    end_offset: row_end,
                });
            }
        }
    }
    if let Some(range) = current_sheet {
        ranges.push(range);
    }
    (text, ranges)
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

fn chunk_nodes_from_text(
    text: &str,
    sheet_ranges: &[SheetTextRange],
    document_id: &str,
    user_id: &str,
) -> Vec<DocumentChunk> {
    token_bounded_ranges(text)
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            let sequence_number =
                u32::try_from(index + 1).expect("XLSX chunk count should fit in u32");
            let chunk_text = &text[range.start_offset..range.end_offset];
            let content_hash = sha256_bytes(chunk_text.as_bytes());
            let chunk_id = sha256_bytes(
                format!("{user_id}\0{document_id}\0{sequence_number}\0{content_hash}").as_bytes(),
            );
            let section_title = sheet_ranges
                .iter()
                .find(|sheet| {
                    range.start_offset >= sheet.start_offset && range.end_offset <= sheet.end_offset
                })
                .map(|sheet| sheet.name.clone());

            DocumentChunk {
                chunk_id,
                document_id: document_id.to_string(),
                user_id: user_id.to_string(),
                text: chunk_text.to_string(),
                embedding: None,
                sequence_number,
                page_numbers: None,
                start_offset: range.start_offset,
                end_offset: range.end_offset,
                token_count: u32::try_from(range.token_count)
                    .expect("XLSX chunk token count should fit in u32"),
                content_hash,
                section_title,
            }
        })
        .collect()
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(value) => format!("ERROR: {value:?}"),
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/formats/spreadsheet_tests.rs"]
mod tests;
