use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentNode {
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
pub struct ChunkNode {
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
