use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub file_id: String,
    pub document_id: String,
    pub user_id: String,
    pub file_name: String,
    pub source_type: String,
    pub local_path: Option<String>,
    pub file_size_bytes: u64,
    pub token_count: u64,
    pub content_hash: String,
    pub rendered_pdf_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub chunk_id: String,
    pub document_id: String,
    pub user_id: String,
    pub text: String,
    pub embedding: Option<Vec<f32>>,
    pub sequence_number: u32,
    pub page_numbers: Option<Vec<u32>>,
    pub start_offset: usize,
    pub end_offset: usize,
    pub token_count: u32,
    pub content_hash: String,
    pub section_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFileData {
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_type: String,
    pub file_hash: String,
    pub file_size_bytes: i64,
    pub ingested_at: String,
    pub total_tokens: i64,
    pub total_chunks: i64,
    pub file_chunks: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub chunk_id: String,
    pub text: String,
    pub text_hash: String,
    pub chunk_index: i64,
    pub token_count: i64,
    pub page_start: i64,
    pub page_end: i64,
    pub embedded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpreadsheetTextChunk {
    pub chunk_index: usize,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ImageEmbeddingResult {
    pub image: Vec<u8>,
    pub description: String,
    pub embedding: Vec<f64>,
}
