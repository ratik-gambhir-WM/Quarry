use std::path::{Path, PathBuf};

use helix_db::dsl::prelude::*;
use helix_db::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const DEFAULT_HELIX_URL: &str = "http://localhost:6969";

pub struct HelixClient {
    pub client: Client,
}

pub fn generate_queries_bundle(path: impl AsRef<Path>) -> Result<PathBuf, String> {
    helix_db::generate_to_path(path)
        .map_err(|err| format!("failed to generate Helix queries bundle: {err}"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFileChunkInput {
    pub chunk_id: String,
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub text: String,
    pub text_hash: String,
    pub chunk_index: i64,
    pub token_count: i64,
    pub page_start: i64,
    pub page_end: i64,
    pub embedded_at: Option<String>,
}

impl HelixClient {
    pub fn new() -> Result<Self, String> {
        let url = std::env::var("HELIX_URL").unwrap_or_else(|_| default_helix_url());
        let api_key = std::env::var("HELIX_API_KEY").ok();

        Self::with_config(url.as_str(), api_key.as_deref())
    }

    pub fn with_config(url: &str, api_key: Option<&str>) -> Result<Self, String> {
        let client = Client::new(Some(url))
            .map_err(|err| format!("failed to create Helix client for `{url}`: {err}"))?
            .with_api_key(api_key);

        Ok(Self { client })
    }

    pub async fn execute_dynamic_query<R, F>(&self, build_query: F) -> Result<R, String>
    where
        R: DeserializeOwned,
        F: FnOnce() -> DynamicQueryRequest,
    {
        self.client
            .query()
            .dynamic(build_query())
            .send()
            .await
            .map_err(|err| format!("failed to execute Helix query: {err}"))
    }
}

#[allow(clippy::too_many_arguments)]
#[register]
pub fn add_file_chunk(
    chunk_id: String,
    file_id: String,
    file_name: String,
    file_path: String,
    text: String,
    text_hash: String,
    chunk_index: i64,
    token_count: i64,
    page_start: i64,
    page_end: i64,
    embedded_at: String,
) -> WriteBatch {
    let _ = (
        &chunk_id,
        &file_id,
        &file_name,
        &file_path,
        &text,
        &text_hash,
        &chunk_index,
        &token_count,
        &page_start,
        &page_end,
        &embedded_at,
    );

    write_batch()
        .var_as(
            "file_chunk",
            g().add_n(
                "FileChunk",
                vec![
                    ("chunk_id", PropertyInput::param("chunk_id")),
                    ("file_id", PropertyInput::param("file_id")),
                    ("file_name", PropertyInput::param("file_name")),
                    ("file_path", PropertyInput::param("file_path")),
                    ("text", PropertyInput::param("text")),
                    ("text_hash", PropertyInput::param("text_hash")),
                    ("chunk_index", PropertyInput::param("chunk_index")),
                    ("token_count", PropertyInput::param("token_count")),
                    ("page_start", PropertyInput::param("page_start")),
                    ("page_end", PropertyInput::param("page_end")),
                    ("embedded_at", PropertyInput::param("embedded_at")),
                ],
            )
            .project(file_chunk_projection()),
        )
        .returning(["file_chunk"])
}

#[register]
pub fn file_chunk_by_chunk_id(chunk_id: String) -> ReadBatch {
    let _ = &chunk_id;

    read_batch()
        .var_as(
            "file_chunk",
            g().n_with_label("FileChunk")
                .where_(Predicate::eq_param("chunk_id", "chunk_id"))
                .project(file_chunk_projection()),
        )
        .returning(["file_chunk"])
}

#[register]
pub fn file_chunks_by_file_id(file_id: String) -> ReadBatch {
    let _ = &file_id;

    read_batch()
        .var_as(
            "file_chunks",
            g().n_with_label("FileChunk")
                .where_(Predicate::eq_param("file_id", "file_id"))
                .project(file_chunk_projection()),
        )
        .returning(["file_chunks"])
}

fn file_chunk_projection() -> Vec<PropertyProjection> {
    vec![
        PropertyProjection::renamed("$id", "id"),
        PropertyProjection::new("chunk_id"),
        PropertyProjection::new("file_id"),
        PropertyProjection::new("file_name"),
        PropertyProjection::new("file_path"),
        PropertyProjection::new("text"),
        PropertyProjection::new("text_hash"),
        PropertyProjection::new("chunk_index"),
        PropertyProjection::new("token_count"),
        PropertyProjection::new("page_start"),
        PropertyProjection::new("page_end"),
        PropertyProjection::new("embedded_at"),
    ]
}

fn default_helix_url() -> String {
    let endpoint = std::env::var("HELIX_ENDPOINT").ok();
    let port = std::env::var("HELIX_PORT").ok();

    match (endpoint, port) {
        (Some(endpoint), Some(port)) if !endpoint.trim().is_empty() && !port.trim().is_empty() => {
            format!("{}:{}", endpoint.trim_end_matches('/'), port)
        }
        (Some(endpoint), _) if !endpoint.trim().is_empty() => endpoint,
        _ => DEFAULT_HELIX_URL.to_string(),
    }
}
