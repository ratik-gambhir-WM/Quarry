//! Namespaced, immutable revision projections. Readers must join the canonical SQLite revision.
use super::model::{Note, PIPELINE};
use crate::adapters::{helix::client::HelixClient, openai::client::OpenAiClient};
use futures_util::future::BoxFuture;
use helix_db::{dsl::prelude::*, IndexDdlReceipt, IndexOperationStatus};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc, time::Duration};

pub(super) trait SavePipeline: Send + Sync {
    fn fingerprint(&self) -> String;
    fn published<'a>(
        &'a self,
        owner: i64,
        note: &'a Note,
        hash: &'a str,
    ) -> BoxFuture<'a, Result<bool, &'static str>>;
    fn embed<'a>(
        &'a self,
        chunks: &'a [String],
    ) -> BoxFuture<'a, Result<Vec<Vec<f32>>, &'static str>>;
    fn publish<'a>(
        &'a self,
        owner: i64,
        note: &'a Note,
        hash: &'a str,
        chunks: &'a [String],
        vectors: Vec<Vec<f32>>,
    ) -> BoxFuture<'a, Result<(), &'static str>>;
    fn delete(&self, owner: i64, note: String) -> BoxFuture<'_, Result<(), &'static str>>;
}
pub(crate) struct NotebookPipeline {
    helix: Arc<HelixClient>,
    openai: Option<Arc<OpenAiClient>>,
    dimensions: NonZeroUsize,
    model: String,
}
const ROOT: &str = "QuarryNotebookRevision";
const CHUNK: &str = "QuarryNotebookChunk";
impl NotebookPipeline {
    pub(crate) fn new(
        helix: Arc<HelixClient>,
        openai: Option<Arc<OpenAiClient>>,
        dimensions: NonZeroUsize,
        model: String,
    ) -> Self {
        Self {
            helix,
            openai,
            dimensions,
            model,
        }
    }
    pub(crate) async fn initialize(&self) -> Result<(), String> {
        let query = QueryRequest::write(
            write_batch()
                .var_as(
                    "root",
                    g().create_index_if_not_exists(IndexSpec::node_unique_equality(
                        ROOT,
                        "projection_key",
                    )),
                )
                .var_as(
                    "chunk",
                    g().create_index_if_not_exists(IndexSpec::node_unique_equality(
                        CHUNK,
                        "projection_key",
                    )),
                )
                .var_as(
                    "vector",
                    g().create_index_if_not_exists(IndexSpec::node_vector(
                        CHUNK,
                        "embedding",
                        self.dimensions,
                        VectorDistanceMetric::Cosine,
                        Some("owner_id"),
                    )),
                )
                .returning(["root", "chunk", "vector"]),
        );
        let receipts: BTreeMap<String, IndexDdlReceipt> = self
            .helix
            .execute_write_query("helix.notebooks.indexes", query)
            .await?;
        if receipts.len() != 3 {
            return Err("notebook index receipts missing".into());
        }
        for receipt in receipts.into_values() {
            if let IndexDdlReceipt::Accepted { operation_id, .. }
            | IndexDdlReceipt::ExistingOperation { operation_id } = receipt
            {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
                loop {
                    #[derive(Deserialize)]
                    struct Status {
                        status: IndexOperationStatus,
                    }
                    let status: Status = self
                        .helix
                        .execute_read_query(
                            "helix.notebooks.index_status",
                            QueryRequest::read(
                                read_batch()
                                    .var_as(
                                        "status",
                                        g().get_index_operation(operation_id.to_string()),
                                    )
                                    .returning(["status"]),
                            ),
                        )
                        .await?;
                    match status.status {
                        IndexOperationStatus::Succeeded { .. } => break,
                        IndexOperationStatus::Queued { .. }
                        | IndexOperationStatus::Running { .. }
                            if tokio::time::Instant::now() < deadline =>
                        {
                            tokio::time::sleep(Duration::from_millis(100)).await
                        }
                        _ => return Err("notebook indexes are not ready".into()),
                    }
                }
            }
        }
        Ok(())
    }
}
fn scope(label: &str, owner: i64, note: &str) -> Traversal<OnNodes> {
    g().n_with_label(label)
        .where_(Predicate::eq("owner_id", owner.to_string()))
        .where_(Predicate::eq("note_id", note.to_owned()))
}
fn key(owner: i64, note: &Note, hash: &str) -> String {
    format!(
        "{owner}:{}:{}:{}:{hash}",
        note.summary.notebook_id, note.summary.note_id, note.summary.revision
    )
}
fn bounded(query: QueryRequest) -> Result<QueryRequest, &'static str> {
    if query.to_json_bytes().map_err(|_| "indexing_failed")?.len() > 2 * 1024 * 1024 {
        return Err("projection_limit");
    }
    Ok(query)
}
impl SavePipeline for NotebookPipeline {
    fn fingerprint(&self) -> String {
        format!("{PIPELINE}/{}/{}", self.model, self.dimensions)
    }
    fn published<'a>(
        &'a self,
        owner: i64,
        note: &'a Note,
        hash: &'a str,
    ) -> BoxFuture<'a, Result<bool, &'static str>> {
        Box::pin(async move {
            #[derive(Deserialize)]
            struct Found {
                roots: Vec<Value>,
            }
            let found: Found = self
                .helix
                .execute_read_query(
                    "helix.notebooks.published",
                    QueryRequest::read(
                        read_batch()
                            .var_as(
                                "roots",
                                g().n_with_label(ROOT)
                                    .where_(Predicate::eq("projection_key", key(owner, note, hash)))
                                    .project(vec![PropertyProjection::new("projection_key")]),
                            )
                            .returning(["roots"]),
                    ),
                )
                .await
                .map_err(|_| "indexing_failed")?;
            Ok(!found.roots.is_empty())
        })
    }
    fn embed<'a>(
        &'a self,
        chunks: &'a [String],
    ) -> BoxFuture<'a, Result<Vec<Vec<f32>>, &'static str>> {
        Box::pin(async move {
            if chunks.is_empty() {
                return Ok(Vec::new());
            }
            let openai = self.openai.as_ref().ok_or("embedding_unavailable")?;
            let mut vectors = Vec::with_capacity(chunks.len());
            for batch in chunks.chunks(16) {
                let inputs = batch.iter().map(String::as_str).collect::<Vec<_>>();
                let result = openai
                    .gen_private_embeddings(&inputs, Some(&self.model))
                    .await
                    .map_err(|_| "embedding_failed")?;
                if result.len() != batch.len() {
                    return Err("embedding_failed");
                }
                for vector in result {
                    if vector.len() != self.dimensions.get() {
                        return Err("embedding_failed");
                    }
                    let vector = vector
                        .into_iter()
                        .map(|v| {
                            let n = v as f32;
                            if n.is_finite() {
                                Ok(n)
                            } else {
                                Err("embedding_failed")
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    vectors.push(vector);
                }
            }
            Ok(vectors)
        })
    }
    fn publish<'a>(
        &'a self,
        owner: i64,
        note: &'a Note,
        hash: &'a str,
        chunks: &'a [String],
        vectors: Vec<Vec<f32>>,
    ) -> BoxFuture<'a, Result<(), &'static str>> {
        Box::pin(async move {
            let (note, hash, chunks, model, dimensions) = (
                note.clone(),
                hash.to_owned(),
                chunks.to_vec(),
                self.model.clone(),
                self.dimensions,
            );
            let queries = tokio::task::spawn_blocking(move || {
                prepare_projection(owner, &note, &hash, &chunks, vectors, dimensions, &model)
            })
            .await
            .map_err(|_| "worker_interrupted")??;
            for query in queries {
                self.helix
                    .execute_write_query::<Value>("helix.notebooks.publish", query)
                    .await
                    .map_err(|_| "indexing_failed")?;
            }
            Ok(())
        })
    }
    fn delete(&self, owner: i64, note: String) -> BoxFuture<'_, Result<(), &'static str>> {
        Box::pin(async move {
            self.helix
                .execute_write_query::<Value>(
                    "helix.notebooks.delete",
                    QueryRequest::write(
                        write_batch()
                            .var_as("roots", scope(ROOT, owner, &note).drop())
                            .var_as("chunks", scope(CHUNK, owner, &note).drop())
                            .returning(["roots", "chunks"]),
                    ),
                )
                .await
                .map_err(|_| "indexing_failed")?;
            Ok(())
        })
    }
}

/// Preflight the entire bounded publication before the first graph write, off the async worker.
fn prepare_projection(
    owner: i64,
    note: &Note,
    hash: &str,
    chunks: &[String],
    vectors: Vec<Vec<f32>>,
    dimensions: NonZeroUsize,
    model: &str,
) -> Result<Vec<QueryRequest>, &'static str> {
    let mut queries = Vec::with_capacity(chunks.len() + 1);
    if chunks.len() > 1024
        || vectors
            .iter()
            .any(|v| v.len() != dimensions.get() || v.iter().any(|n| !n.is_finite()))
    {
        return Err("embedding_failed");
    }
    if vectors.len() != chunks.len() {
        return Err("embedding_failed");
    }
    let root_key = key(owner, note, hash);
    let revision = note.summary.revision;
    // Stage immutable chunks with deterministic IDs. An absent root means unpublished data.
    for (ordinal, (text, vector)) in chunks.iter().zip(vectors).enumerate() {
        let ordinal = i64::try_from(ordinal).map_err(|_| "projection_limit")?;
        let chunk_key = format!("{root_key}:{ordinal}");
        let properties = vec![
            ("projection_key", PropertyInput::from(chunk_key.clone())),
            ("revision_key", PropertyInput::from(root_key.clone())),
            ("owner_id", PropertyInput::from(owner.to_string())),
            (
                "notebook_id",
                PropertyInput::from(note.summary.notebook_id.clone()),
            ),
            ("note_id", PropertyInput::from(note.summary.note_id.clone())),
            ("revision", PropertyInput::from(revision)),
            ("ordinal", PropertyInput::from(ordinal)),
            ("content_hash", PropertyInput::from(hash.to_owned())),
            ("pipeline_version", PropertyInput::from(PIPELINE)),
            ("embedding_model", PropertyInput::from(model.to_owned())),
            (
                "embedding_dimensions",
                PropertyInput::from(
                    i64::try_from(dimensions.get()).map_err(|_| "embedding_failed")?,
                ),
            ),
            ("text", PropertyInput::from(text.clone())),
            (
                "embedding",
                PropertyInput::from(PropertyValue::F32Array(vector)),
            ),
        ];
        let query = bounded(QueryRequest::write(
            write_batch()
                .var_as(
                    "old",
                    g().n_with_label(CHUNK)
                        .where_(Predicate::eq("projection_key", chunk_key))
                        .drop(),
                )
                .var_as("chunk", g().add_n(CHUNK, properties))
                .returning(["chunk"]),
        ))?;
        queries.push(query);
    }
    // Root publication is one durable write after all chunks are confirmed. Immutable per
    // revision roots prevent an older/expired worker from replacing a newer revision.
    let query = bounded(QueryRequest::write(
        write_batch()
            .var_as(
                "old",
                g().n_with_label(ROOT)
                    .where_(Predicate::eq("projection_key", root_key.clone()))
                    .drop(),
            )
            .var_as(
                "root",
                g().add_n(
                    ROOT,
                    vec![
                        ("projection_key", PropertyInput::from(root_key)),
                        ("owner_id", PropertyInput::from(owner.to_string())),
                        (
                            "notebook_id",
                            PropertyInput::from(note.summary.notebook_id.clone()),
                        ),
                        ("note_id", PropertyInput::from(note.summary.note_id.clone())),
                        ("revision", PropertyInput::from(revision)),
                        ("content_hash", PropertyInput::from(hash.to_owned())),
                        ("pipeline_version", PropertyInput::from(PIPELINE)),
                        ("embedding_model", PropertyInput::from(model.to_owned())),
                        (
                            "embedding_dimensions",
                            PropertyInput::from(
                                i64::try_from(dimensions.get()).map_err(|_| "embedding_failed")?,
                            ),
                        ),
                        (
                            "chunk_count",
                            PropertyInput::from(
                                i64::try_from(chunks.len()).map_err(|_| "projection_limit")?,
                            ),
                        ),
                    ],
                ),
            )
            .var_as(
                "old_roots",
                scope(ROOT, owner, &note.summary.note_id)
                    .where_(Predicate::lt("revision", revision))
                    .drop(),
            )
            .var_as(
                "old_chunks",
                scope(CHUNK, owner, &note.summary.note_id)
                    .where_(Predicate::lt("revision", revision))
                    .drop(),
            )
            .returning(["root"]),
    ))?;
    queries.push(query);
    Ok(queries)
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/graph_tests.rs"]
mod tests;
