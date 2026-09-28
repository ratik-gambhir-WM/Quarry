use std::path::{Path, PathBuf};

use crate::{
    domains::documents::{
        formats::text_chunking::token_bounded_ranges,
        model::{Document, DocumentChunk},
    },
    shared::ids::{document_id_from_content, sha256_hex},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct OfficeTextAssembly {
    pub document: Document,
    pub chunks: Vec<DocumentChunk>,
}

/// Builds the same durable document and chunk identity shape used by the PDF
/// and DOCX parsers for an Office format whose parser produces plain text.
pub fn build_office_text_assembly(
    bytes: &[u8],
    path: Option<&Path>,
    user_id: &str,
    source_type: &str,
    default_file_name: &str,
    text: String,
) -> Result<OfficeTextAssembly, String> {
    if text.trim().is_empty() {
        return Err(format!("{source_type} did not contain readable text"));
    }

    let file_size_bytes = u64::try_from(bytes.len())
        .map_err(|_| format!("{source_type} byte length does not fit in u64"))?;
    let content_hash = sha256_hex(bytes);
    let document_id = document_id_from_content(user_id, &content_hash);
    let chunks = chunk_nodes_from_text(&text, &document_id, user_id)?;
    let token_count = chunks
        .iter()
        .map(|chunk| u64::from(chunk.token_count))
        .sum();
    let source_path = path.map(|path| path.canonicalize().unwrap_or_else(|_| PathBuf::from(path)));
    let document = Document {
        file_id: Uuid::new_v4().to_string(),
        document_id,
        user_id: user_id.to_string(),
        file_name: source_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or(default_file_name)
            .to_string(),
        source_type: source_type.to_string(),
        local_path: source_path.map(|path| path.to_string_lossy().into_owned()),
        file_size_bytes,
        token_count,
        content_hash,
        rendered_pdf_path: None,
    };

    Ok(OfficeTextAssembly { document, chunks })
}

fn chunk_nodes_from_text(
    text: &str,
    document_id: &str,
    user_id: &str,
) -> Result<Vec<DocumentChunk>, String> {
    token_bounded_ranges(text)
        .into_iter()
        .enumerate()
        .map(|(sequence_index, range)| {
            let sequence_number = u32::try_from(sequence_index + 1)
                .map_err(|_| "Office document has too many chunks".to_string())?;
            let token_count = u32::try_from(range.token_count)
                .map_err(|_| "Office document chunk has too many tokens".to_string())?;
            let chunk_text = &text[range.start_offset..range.end_offset];
            let content_hash = sha256_hex(chunk_text.as_bytes());
            let chunk_id = sha256_hex(
                format!("{user_id}\0{document_id}\0{sequence_number}\0{content_hash}").as_bytes(),
            );

            Ok(DocumentChunk {
                chunk_id,
                document_id: document_id.to_string(),
                user_id: user_id.to_string(),
                text: chunk_text.to_string(),
                embedding: None,
                sequence_number,
                page_numbers: None,
                start_offset: range.start_offset,
                end_offset: range.end_offset,
                token_count,
                content_hash,
                section_title: None,
            })
        })
        .collect()
}
