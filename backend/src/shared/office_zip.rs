use std::io::{Cursor, Read};

use zip::ZipArchive;

/// Maximum number of members accepted in an OOXML package.
pub const MAX_OFFICE_ZIP_ENTRIES: usize = 2_048;
/// Maximum declared uncompressed size for any individual OOXML member.
pub const MAX_OFFICE_ZIP_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum cumulative declared uncompressed size for an OOXML package.
pub const MAX_OFFICE_ZIP_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
/// Maximum amount read from `[Content_Types].xml` while validating a package.
pub const MAX_OFFICE_CONTENT_TYPES_BYTES: usize = 1024 * 1024;

/// The OOXML package family expected by a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeDocumentKind {
    Word,
    PowerPoint,
    Spreadsheet,
}

impl OfficeDocumentKind {
    fn required_root(self) -> &'static str {
        match self {
            Self::Word => "word/document.xml",
            Self::PowerPoint => "ppt/presentation.xml",
            Self::Spreadsheet => "xl/workbook.xml",
        }
    }

    fn content_type_marker(self) -> &'static str {
        match self {
            Self::Word => "wordprocessingml.document",
            Self::PowerPoint => "presentationml.presentation",
            Self::Spreadsheet => "spreadsheetml.sheet",
        }
    }
}

/// A deliberately detail-free OOXML package validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfficePackageError;

/// Checks an OOXML central directory without extracting arbitrary members.
///
/// This validates the package family, declared expansion limits, and a bounded
/// content-types manifest before a format parser touches the archive.
pub fn validate_office_package(
    bytes: &[u8],
    kind: OfficeDocumentKind,
) -> Result<(), OfficePackageError> {
    if bytes.is_empty() {
        return Err(OfficePackageError);
    }

    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| OfficePackageError)?;
    if archive.len() > MAX_OFFICE_ZIP_ENTRIES {
        return Err(OfficePackageError);
    }

    let mut total_uncompressed_bytes = 0u64;
    let mut has_required_root = false;
    let mut content_types = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|_| OfficePackageError)?;
        let entry_size = entry.size();
        if entry_size > MAX_OFFICE_ZIP_ENTRY_BYTES {
            return Err(OfficePackageError);
        }
        total_uncompressed_bytes = total_uncompressed_bytes
            .checked_add(entry_size)
            .ok_or(OfficePackageError)?;
        if total_uncompressed_bytes > MAX_OFFICE_ZIP_TOTAL_BYTES {
            return Err(OfficePackageError);
        }
        if entry.name() == kind.required_root() {
            has_required_root = true;
        }
        if entry.name() == "[Content_Types].xml" {
            if entry_size > MAX_OFFICE_CONTENT_TYPES_BYTES as u64 {
                return Err(OfficePackageError);
            }
            let mut manifest = String::new();
            entry
                .take(MAX_OFFICE_CONTENT_TYPES_BYTES as u64 + 1)
                .read_to_string(&mut manifest)
                .map_err(|_| OfficePackageError)?;
            if manifest.len() > MAX_OFFICE_CONTENT_TYPES_BYTES {
                return Err(OfficePackageError);
            }
            content_types = Some(manifest);
        }
    }

    if has_required_root
        && content_types.is_some_and(|manifest| manifest.contains(kind.content_type_marker()))
    {
        Ok(())
    } else {
        Err(OfficePackageError)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/shared/office_zip_tests.rs"]
mod tests;
