use std::{
    collections::{BTreeMap, HashSet},
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};

use helix_db::{IndexDdlReceipt, IndexOperationStatus};
use serde::Deserialize;
use serde_json::Value;
use tokio::time::sleep;

use crate::{
    adapters::helix::client::HelixClient,
    domains::documents::graph::{
        model::{FileChunkNode, FileNode, FileVersionNode},
        query::{
            find_current_helix_document_by_content_hash as build_content_hash_lookup,
            get_current_helix_document as build_current_document_lookup,
            get_helix_document_version as build_document_version_lookup,
            get_helix_document_version_chunks as build_document_version_chunks_lookup,
            search_document_chunks_by_keyword as build_keyword_search,
            search_document_chunks_by_vector as build_vector_search, FileChunkKeywordSearch,
            FileChunkResult, FileChunkVectorSearch, HelixDocumentVersion, KeywordFileChunkHit,
            VectorFileChunkHit,
        },
        writer::{
            create_document_indexes, get_document_index_operation, insert_file_version_graph,
            FileVersionGraphWrite, DOCUMENT_INDEX_NAMES,
        },
    },
    domains::documents::store::identity::ChunkIdentity,
    shared::error::RepositoryError,
};

#[derive(Debug, Deserialize)]
pub(crate) struct HelixDocumentVersionResponse {
    file: Vec<FileNode>,
    version: Vec<FileVersionNode>,
}

#[derive(Debug, Deserialize)]
struct FileChunksResponse {
    chunks: Vec<FileChunkResult>,
}

#[derive(Debug, Deserialize)]
struct VectorSearchResponse {
    chunks: Vec<VectorFileChunkHit>,
}

#[derive(Debug, Deserialize)]
struct KeywordSearchResponse {
    chunks: Vec<KeywordFileChunkHit>,
}

#[derive(Debug, Deserialize)]
struct IndexOperationStatusResponse {
    status: IndexOperationStatus,
}

#[derive(Clone)]
pub struct DocumentGraphWriter {
    helix: Arc<HelixClient>,
}

/// Immutable Helix graph writes that have passed exact request-size validation.
/// The setup query clears the version's previous chunks; each following query
/// adds one bounded chunk batch for that version.
pub(crate) struct PreparedDocumentGraph {
    pub(crate) file_node: FileNode,
    pub(crate) version_node: FileVersionNode,
    pub(crate) chunk_nodes: Vec<FileChunkNode>,
    graph_write: FileVersionGraphWrite,
}

impl PreparedDocumentGraph {
    pub(crate) fn matches_identity(
        &self,
        deal_id: &str,
        file_id: &str,
        display_name: &str,
        version_id: &str,
    ) -> bool {
        self.file_node.deal_id == deal_id
            && self.file_node.file_id == file_id
            && self.file_node.display_name == display_name
            && self.version_node.deal_id == deal_id
            && self.version_node.file_id == file_id
            && self.version_node.version_id == version_id
            && self.chunk_nodes.iter().all(|chunk| {
                chunk.deal_id == deal_id
                    && chunk.file_id == file_id
                    && chunk.version_id == version_id
            })
    }
}

impl DocumentGraphWriter {
    pub fn new(helix: Arc<HelixClient>) -> Self {
        Self { helix }
    }

    pub async fn initialize(&self, vector_dimension: NonZeroUsize) -> Result<(), RepositoryError> {
        ensure_document_indexes(&self.helix, vector_dimension)
            .await
            .map(|_| ())
            .map_err(RepositoryError::storage)
    }

    pub async fn insert_graph(
        &self,
        file_node: FileNode,
        version_node: FileVersionNode,
        chunk_nodes: Vec<FileChunkNode>,
    ) -> Result<Value, RepositoryError> {
        let prepared = self.preflight_graph(file_node, version_node, chunk_nodes)?;
        self.insert_prepared_graph(prepared).await
    }

    pub(crate) fn preflight_graph(
        &self,
        file_node: FileNode,
        version_node: FileVersionNode,
        chunk_nodes: Vec<FileChunkNode>,
    ) -> Result<PreparedDocumentGraph, RepositoryError> {
        let graph_write =
            insert_file_version_graph(file_node.clone(), version_node.clone(), chunk_nodes.clone())
                .map_err(RepositoryError::storage)?;
        Ok(PreparedDocumentGraph {
            file_node,
            version_node,
            chunk_nodes,
            graph_write,
        })
    }

    pub(crate) async fn insert_prepared_graph(
        &self,
        prepared: PreparedDocumentGraph,
    ) -> Result<Value, RepositoryError> {
        self.helix
            .execute_write_query::<Value>(
                "helix.file_version.insert_setup",
                prepared.graph_write.setup_query,
            )
            .await
            .map_err(RepositoryError::storage)?;
        let batch_count = prepared.graph_write.chunk_queries.len();
        for (batch_index, query) in prepared.graph_write.chunk_queries.into_iter().enumerate() {
            self.helix
                .execute_write_query::<Value>("helix.file_version.insert_chunk_batch", query)
                .await
                .map_err(|error| {
                    RepositoryError::storage(format!(
                        "failed to index Helix chunk batch {} of {batch_count}: {error}",
                        batch_index + 1,
                    ))
                })?;
        }
        self.helix
            .execute_write_query(
                "helix.file_version.publish",
                prepared.graph_write.publish_query,
            )
            .await
            .map_err(|error| {
                RepositoryError::storage(format!(
                    "failed to publish Helix file version after indexing {batch_count} chunk batch(es): {error}"
                ))
            })
    }
}

#[derive(Clone)]
pub struct DocumentGraphReader {
    helix: Arc<HelixClient>,
}

impl DocumentGraphReader {
    pub fn new(helix: Arc<HelixClient>) -> Self {
        Self { helix }
    }

    pub async fn has_complete_chunks(
        &self,
        deal: &str,
        expected: &[ChunkIdentity],
    ) -> Result<bool, RepositoryError> {
        has_complete_chunks(&self.helix, deal, expected).await
    }

    pub async fn current_document(
        &self,
        deal_id: &str,
        file_id: &str,
    ) -> Result<Option<HelixDocumentVersion>, RepositoryError> {
        get_current_helix_document(&self.helix, deal_id, file_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn current_document_by_hash(
        &self,
        deal_id: &str,
        content_sha256: &str,
    ) -> Result<Option<HelixDocumentVersion>, RepositoryError> {
        find_current_helix_document_by_content_hash(&self.helix, deal_id, content_sha256)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn document_version(
        &self,
        deal_id: &str,
        file_id: &str,
        version_id: &str,
    ) -> Result<Option<HelixDocumentVersion>, RepositoryError> {
        get_helix_document_version(&self.helix, deal_id, file_id, version_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn document_version_chunks(
        &self,
        deal_id: &str,
        file_id: &str,
        version_id: &str,
    ) -> Result<Vec<FileChunkResult>, RepositoryError> {
        get_helix_document_version_chunks(&self.helix, deal_id, file_id, version_id)
            .await
            .map_err(RepositoryError::storage)
    }
}

#[derive(Clone)]
pub struct DocumentGraphSearch {
    helix: Arc<HelixClient>,
}

impl DocumentGraphSearch {
    pub fn new(helix: Arc<HelixClient>) -> Self {
        Self { helix }
    }

    pub async fn search_vector(
        &self,
        search: FileChunkVectorSearch,
    ) -> Result<Vec<VectorFileChunkHit>, RepositoryError> {
        search_document_chunks_by_vector(&self.helix, search)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn search_keyword(
        &self,
        search: FileChunkKeywordSearch,
    ) -> Result<Vec<KeywordFileChunkHit>, RepositoryError> {
        search_document_chunks_by_keyword(&self.helix, search)
            .await
            .map_err(RepositoryError::storage)
    }
}

const DOCUMENT_INDEX_READINESS_TIMEOUT: Duration = Duration::from_secs(60);
const DOCUMENT_INDEX_READINESS_POLL_INTERVAL: Duration = Duration::from_millis(100);

async fn ensure_document_indexes(
    helix: &HelixClient,
    vector_dimension: NonZeroUsize,
) -> Result<(), String> {
    let receipts: BTreeMap<String, IndexDdlReceipt> = helix
        .execute_write_query(
            "helix.document_indexes.initialize",
            create_document_indexes(vector_dimension),
        )
        .await?;
    let operation_ids = document_index_operation_ids(receipts)?;
    for operation_id in operation_ids {
        wait_for_document_index_operation(helix, &operation_id).await?;
    }
    Ok(())
}

fn document_index_operation_ids(
    receipts: BTreeMap<String, IndexDdlReceipt>,
) -> Result<Vec<String>, String> {
    let mut operation_ids = Vec::new();
    for index_name in DOCUMENT_INDEX_NAMES {
        let receipt = receipts
            .get(index_name)
            .ok_or_else(|| format!("Helix index initialization omitted `{index_name}`"))?;
        if let IndexDdlReceipt::Accepted { operation_id, .. }
        | IndexDdlReceipt::ExistingOperation { operation_id } = receipt
        {
            operation_ids.push(operation_id.to_string());
        }
    }
    Ok(operation_ids)
}

pub(crate) async fn wait_for_document_index_operation(
    helix: &HelixClient,
    operation_id: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + DOCUMENT_INDEX_READINESS_TIMEOUT;
    loop {
        let response: IndexOperationStatusResponse = helix
            .execute_read_query(
                "helix.document_indexes.operation_status",
                get_document_index_operation(operation_id.to_string()),
            )
            .await?;
        match response.status {
            IndexOperationStatus::Succeeded { .. } => return Ok(()),
            IndexOperationStatus::Queued { .. } | IndexOperationStatus::Running { .. } => {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "Helix index operation `{operation_id}` did not become usable before the bootstrap deadline"
                    ));
                }
                sleep(DOCUMENT_INDEX_READINESS_POLL_INTERVAL).await;
            }
            IndexOperationStatus::Blocked { .. } | IndexOperationStatus::Aborted { .. } => {
                tracing::error!(
                    operation_id,
                    "Helix index operation ended without becoming usable"
                );
                return Err(format!(
                    "Helix index operation `{operation_id}` did not become usable"
                ));
            }
        }
    }
}

async fn find_current_helix_document_by_content_hash(
    helix: &HelixClient,
    deal_id: &str,
    content_sha256: &str,
) -> Result<Option<HelixDocumentVersion>, String> {
    let query = build_content_hash_lookup(deal_id.to_string(), content_sha256.to_string())?;
    let response: HelixDocumentVersionResponse = helix
        .execute_read_query("helix.file_version.by_content_hash", query)
        .await?;
    map_document_version_response(response, deal_id, None, None, Some(content_sha256))
}

async fn get_current_helix_document(
    helix: &HelixClient,
    deal_id: &str,
    file_id: &str,
) -> Result<Option<HelixDocumentVersion>, String> {
    let query = build_current_document_lookup(deal_id.to_string(), file_id.to_string())?;
    let response: HelixDocumentVersionResponse = helix
        .execute_read_query("helix.file_version.current", query)
        .await?;
    map_document_version_response(response, deal_id, Some(file_id), None, None)
}

async fn get_helix_document_version(
    helix: &HelixClient,
    deal_id: &str,
    file_id: &str,
    version_id: &str,
) -> Result<Option<HelixDocumentVersion>, String> {
    let query = build_document_version_lookup(
        deal_id.to_string(),
        file_id.to_string(),
        version_id.to_string(),
    )?;
    let response: HelixDocumentVersionResponse = helix
        .execute_read_query("helix.file_version.by_id", query)
        .await?;
    map_document_version_response(response, deal_id, Some(file_id), Some(version_id), None)
}

async fn get_helix_document_version_chunks(
    helix: &HelixClient,
    deal_id: &str,
    file_id: &str,
    version_id: &str,
) -> Result<Vec<FileChunkResult>, String> {
    let query = build_document_version_chunks_lookup(
        deal_id.to_string(),
        file_id.to_string(),
        version_id.to_string(),
    )?;
    let response: FileChunksResponse = helix
        .execute_read_query("helix.file_version.chunks", query)
        .await?;
    let mut chunks = response.chunks;
    let mut indices = HashSet::with_capacity(chunks.len());
    for chunk in &chunks {
        if chunk.deal_id != deal_id || chunk.file_id != file_id || chunk.version_id != version_id {
            return Err(
                "Helix version-chunk response contained a mismatched graph identity".to_string(),
            );
        }
        if !indices.insert(chunk.chunk_index) {
            return Err(format!(
                "Helix version-chunk response contained duplicate chunk index {}",
                chunk.chunk_index
            ));
        }
    }
    chunks.sort_by_key(|chunk| chunk.chunk_index);
    Ok(chunks)
}

async fn search_document_chunks_by_vector(
    helix: &HelixClient,
    search: FileChunkVectorSearch,
) -> Result<Vec<VectorFileChunkHit>, String> {
    let deal_id = search.deal_id.clone();
    let query = build_vector_search(search)?;
    let response: VectorSearchResponse = helix
        .execute_read_query("helix.file_chunk.search_vector", query)
        .await?;
    validate_search_identities(&deal_id, response.chunks.iter().map(|hit| &hit.chunk))?;
    Ok(response.chunks)
}

async fn search_document_chunks_by_keyword(
    helix: &HelixClient,
    search: FileChunkKeywordSearch,
) -> Result<Vec<KeywordFileChunkHit>, String> {
    let deal_id = search.deal_id.clone();
    let query = build_keyword_search(search)?;
    let response: KeywordSearchResponse = helix
        .execute_read_query("helix.file_chunk.search_keyword", query)
        .await?;
    validate_search_identities(&deal_id, response.chunks.iter().map(|hit| &hit.chunk))?;
    Ok(response.chunks)
}

pub(crate) fn map_document_version_response(
    response: HelixDocumentVersionResponse,
    deal_id: &str,
    expected_file_id: Option<&str>,
    expected_version_id: Option<&str>,
    expected_content_sha256: Option<&str>,
) -> Result<Option<HelixDocumentVersion>, String> {
    let files = response.file;
    let versions = response.version;
    if files.is_empty() && versions.is_empty() {
        return Ok(None);
    }
    if files.len() != 1 || versions.len() != 1 {
        return Err(format!(
            "Helix document response integrity error: expected one file and one version, received {} file(s) and {} version(s)",
            files.len(),
            versions.len()
        ));
    }
    let file = files.into_iter().next().expect("one file was checked");
    let version = versions
        .into_iter()
        .next()
        .expect("one version was checked");
    if file.deal_id != deal_id
        || version.deal_id != deal_id
        || version.file_id != file.file_id
        || expected_file_id.is_some_and(|expected| file.file_id != expected)
        || expected_version_id.is_some_and(|expected| version.version_id != expected)
        || expected_content_sha256.is_some_and(|expected| version.content_sha256 != expected)
    {
        return Err("Helix document response contained a mismatched graph identity".to_string());
    }
    Ok(Some(HelixDocumentVersion { file, version }))
}

fn validate_search_identities<'a>(
    deal_id: &str,
    chunks: impl Iterator<Item = &'a FileChunkResult>,
) -> Result<(), String> {
    for chunk in chunks {
        if chunk.deal_id != deal_id {
            return Err(format!(
                "Helix search response chunk `{}` belongs to deal_id `{}`, not `{deal_id}`",
                chunk.chunk_id, chunk.deal_id
            ));
        }
    }
    Ok(())
}
impl DocumentGraphSearch {
    pub async fn vector_current(
        &self,
        search: FileChunkVectorSearch,
        versions: Vec<String>,
    ) -> Result<Vec<VectorFileChunkHit>, RepositoryError> {
        if versions.is_empty() {
            return Ok(Vec::new());
        }
        let deal = search.deal_id.clone();
        let query = super::query::current_vector_candidates(
            search.deal_id,
            versions,
            search.query_embedding,
            i64::try_from(search.limit).map_err(|_| RepositoryError::invalid("invalid limit"))?,
        )
        .map_err(|e| RepositoryError::storage(e.to_string()))?;
        let response: VectorSearchResponse = self
            .helix
            .execute_read_query("helix.file_chunk.current_vector", query)
            .await
            .map_err(RepositoryError::storage)?;
        validate_search_identities(&deal, response.chunks.iter().map(|h| &h.chunk))
            .map_err(RepositoryError::storage)?;
        Ok(response.chunks)
    }
    pub async fn keyword_current(
        &self,
        search: FileChunkKeywordSearch,
        versions: Vec<String>,
    ) -> Result<Vec<KeywordFileChunkHit>, RepositoryError> {
        if versions.is_empty() {
            return Ok(Vec::new());
        }
        let query = super::query::current_keyword_candidates(search.deal_id.clone(), versions)
            .map_err(|e| RepositoryError::storage(e.to_string()))?;
        let response: FileChunksResponse = self
            .helix
            .execute_read_query("helix.file_chunk.current_keyword", query)
            .await
            .map_err(RepositoryError::storage)?;
        validate_search_identities(&search.deal_id, response.chunks.iter())
            .map_err(RepositoryError::storage)?;
        if response.chunks.len() > 10000 {
            return Err(RepositoryError::Capacity);
        }
        tokio::task::spawn_blocking(move || {
            rank_keywords(response.chunks, &search.query_text, search.limit)
        })
        .await
        .map_err(|e| RepositoryError::BlockingWorker(e.to_string()))
    }
}
/// Bounded lexical relevance over canonical current candidates. The pinned SDK has no within-stream text ranking.
fn rank_keywords(
    chunks: Vec<FileChunkResult>,
    query: &str,
    limit: usize,
) -> Vec<KeywordFileChunkHit> {
    let terms = query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<HashSet<_>>();
    let mut hits = chunks
        .into_iter()
        .filter_map(|chunk| {
            let text = chunk.text.to_lowercase();
            let score = terms
                .iter()
                .map(|term| text.matches(term.as_str()).count() as f64)
                .sum::<f64>()
                / (1.0 + text.split_whitespace().count() as f64).sqrt();
            (score > 0.0).then_some(KeywordFileChunkHit { chunk, score })
        })
        .collect::<Vec<_>>();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.chunk.chunk_id.cmp(&b.chunk.chunk_id))
    });
    hits.truncate(limit);
    hits
}

impl DocumentGraphSearch {
    pub async fn verify_candidates(
        &self,
        deal: String,
        expected: Vec<ChunkIdentity>,
    ) -> Result<(), RepositoryError> {
        if !has_complete_chunks(&self.helix, &deal, &expected).await? {
            return Err(RepositoryError::Unavailable(
                "document indexing is incomplete; retry the original file before searching".into(),
            ));
        }
        Ok(())
    }
}

/// Share the same canonical header check between search and exact-byte upload retries.
async fn has_complete_chunks(
    helix: &HelixClient,
    deal: &str,
    expected: &[ChunkIdentity],
) -> Result<bool, RepositoryError> {
    #[derive(Deserialize)]
    struct Headers {
        chunks: Vec<ChunkIdentity>,
    }
    // Uploads may exceed the search candidate bound. Keep each verification query bounded.
    for batch in expected.chunks(10_000) {
        let query = super::query::current_chunk_headers(
            deal.into(),
            batch.iter().map(|c| c.chunk_id.clone()).collect(),
        )
        .map_err(|e| RepositoryError::storage(e.to_string()))?;
        let response: Headers = helix
            .execute_read_query("helix.file_chunk.verify_candidates", query)
            .await
            .map_err(RepositoryError::storage)?;
        if response.chunks.len() != batch.len()
            || response.chunks.iter().collect::<HashSet<_>>()
                != batch.iter().collect::<HashSet<_>>()
        {
            return Ok(false);
        }
    }
    Ok(true)
}
