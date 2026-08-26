use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    pub workspace_id: String,
    pub file_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileVersionNode {
    pub workspace_id: String,
    pub file_id: String,
    pub version_id: String,
    pub mime_type: String,
    pub content_sha256: String,
    pub byte_size: i64,
    pub index_generation: String,
    pub indexed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChunkNode {
    pub chunk_id: String,
    pub workspace_id: String,
    pub file_id: String,
    pub version_id: String,
    pub index_generation: String,
    pub chunk_index: i64,
    pub text: String,
    pub embedding: Vec<f32>,
    pub chunk_sha256: String,
    pub token_count: i64,
    pub page_start: Option<i64>,
    pub page_end: Option<i64>,
    pub char_start: i64,
    pub char_end: i64,
    pub section_path: String,
    pub created_at: String,
}
