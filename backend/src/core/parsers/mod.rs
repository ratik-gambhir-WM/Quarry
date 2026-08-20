pub mod docx;
pub mod image;
pub mod pdf;
pub mod powerpoint;
pub mod spreadsheet;

use std::{
    fs::{self, File, Metadata},
    path::{Path, PathBuf},
};

use self::{
    docx::{parse_docx_chunks_from_bytes, DocxAssembly},
    pdf::{parse_pdf_by_bytes, PdfDocumentAssembly},
};

#[derive(Debug)]
pub enum QuarryFile {
    Pdf {
        bytes: Vec<u8>,
        path: Option<PathBuf>,
        file_name: String,
    },
    Docx {
        bytes: Vec<u8>,
        path: Option<PathBuf>,
        file_name: String,
    },
}

#[derive(Debug)]
pub enum ParsedQuarryFile {
    Pdf(PdfDocumentAssembly),
    Docx(DocxAssembly),
}

/// Reads filesystem metadata from an already-open file without consuming it.
pub fn generate_file_metadata(file: &File) -> Result<Metadata, String> {
    file.metadata()
        .map_err(|error| format!("failed to read file metadata: {error}"))
}

impl QuarryFile {
    pub fn from_local_path(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("failed to derive filename from {}", path.display()))?
            .to_string();
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        Self::from_parts(file_name, bytes, Some(path))
    }

    /// Builds a parser input from browser-uploaded bytes without inventing a
    /// server-local path for the document.
    pub fn from_bytes(file_name: impl Into<String>, bytes: Vec<u8>) -> Result<Self, String> {
        Self::from_parts(file_name.into(), bytes, None)
    }

    /// Parses the file into Quarry's graph-ready document/chunk assembly.
    /// User identity is explicit so graph nodes are never written unscoped.
    pub fn parse(self, user_id: &str) -> Result<ParsedQuarryFile, String> {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return Err("user_id cannot be empty".to_string());
        }

        match self {
            Self::Pdf {
                bytes,
                path,
                file_name,
            } => {
                let mut assembly = parse_pdf_by_bytes(bytes, path.as_deref(), user_id)?;
                assembly.document.file_name = file_name;
                Ok(ParsedQuarryFile::Pdf(assembly))
            }
            Self::Docx {
                bytes,
                path,
                file_name,
            } => {
                let mut assembly = parse_docx_chunks_from_bytes(bytes, path.as_deref(), user_id)?;
                assembly.document.file_name = file_name;
                Ok(ParsedQuarryFile::Docx(assembly))
            }
        }
    }

    fn from_parts(
        file_name: String,
        bytes: Vec<u8>,
        path: Option<PathBuf>,
    ) -> Result<Self, String> {
        if bytes.is_empty() {
            return Err("file is empty".to_string());
        }
        let extension = Path::new(&file_name)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| "invalid file format".to_string())?;

        match extension.as_str() {
            "pdf" => Ok(Self::Pdf {
                bytes,
                path,
                file_name,
            }),
            "docx" => Ok(Self::Docx {
                bytes,
                path,
                file_name,
            }),
            _ => Err("invalid file format".to_string()),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/core/parsers/mod_tests.rs"]
mod tests;
