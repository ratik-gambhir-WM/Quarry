pub mod docx;
pub mod image;
pub mod pdf;
// pub mod powerpoint;
// pub mod spreadsheet;

use crate::core::parsers::docx::{parse_docx_chunks_from_bytes, DocxAssembly};
use crate::core::parsers::pdf::{parse_pdf_by_bytes, PdfDocumentAssembly};
// use crate::core::clients::openai::OpenAiClient;
// use crate::core::parsers::image::parse_image_file;
// use crate::core::parsers::powerpoint::parse_powerpoint_file;
// use crate::core::parsers::spreadsheet::parse_spreadsheet;
use std::fs::{self, File, Metadata};
use std::path::PathBuf;

#[derive(Debug)]
pub enum QuarryFile {
    Pdf { bytes: Vec<u8>, path: PathBuf },
    Docx { bytes: Vec<u8>, path: PathBuf },
    // Powerpoint { bytes: Vec<u8>, path: PathBuf },
    // Image { bytes: Vec<u8>, path: PathBuf },
    // Spreadsheet { bytes: Vec<u8>, path: PathBuf },
}

#[derive(Debug)]
pub enum ParsedQuarryFile {
    Pdf(PdfDocumentAssembly),
    Docx(DocxAssembly),
}

/// Reads filesystem metadata from an already-open file without consuming it.
pub fn generate_file_metadata(file: &File) -> Result<Metadata, String> {
    file.metadata()
        .map_err(|err| format!("failed to read file metadata: {err}"))
}

impl QuarryFile {
    pub fn from_local_path(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .ok_or_else(|| "invalid file format".to_owned())?;
        if !matches!(extension.as_str(), "pdf" | "docx") {
            return Err("invalid file format".to_owned());
        }

        let bytes =
            fs::read(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;

        match extension.as_str() {
            "pdf" => Ok(Self::Pdf { bytes, path }),
            "docx" => Ok(Self::Docx { bytes, path }),
            // "pptx" | "ppt" => Ok(Self::Powerpoint { bytes, path }),
            // "gif" | "jpg" | "jpeg" | "png" | "webp" => Ok(Self::Image { bytes, path }),
            //"xlsx" => Ok(Self::Spreadsheet { bytes, path }),
            _ => unreachable!("supported extensions were validated before opening the file"),
        }
    }

    pub async fn parse_for_user(self, user_id: &str) -> Result<ParsedQuarryFile, String> {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return Err("user_id cannot be empty".to_string());
        }

        match self {
            // Self::Powerpoint { path, .. } => parse_powerpoint_file(&path),
            // Self::Image { path, .. } => {
            //     let openai_client = OpenAiClient::new()?;
            //     parse_image_file(&path, &openai_client).await
            // }
            // Self::Spreadsheet { path, .. } => {
            //     parse_spreadsheet(&path).map_err(|err| err.to_string())
            // }
            Self::Pdf { bytes, path } => {
                parse_pdf_by_bytes(bytes, Some(&path), user_id).map(ParsedQuarryFile::Pdf)
            }
            Self::Docx { bytes, path } => parse_docx_chunks_from_bytes(bytes, Some(&path), user_id)
                .map(ParsedQuarryFile::Docx),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/core/parsers/mod_tests.rs"]
mod tests;
