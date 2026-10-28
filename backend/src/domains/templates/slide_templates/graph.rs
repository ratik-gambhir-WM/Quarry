//! Recoverable Helix projection. SQLite remains authoritative for documents and retrieval artifacts.
use super::{
    repository::DEFAULT_APP_ID, StoredSlideClassification, StoredTemplate, StoredTemplateMetadata,
};
use crate::adapters::helix::client::HelixClient;
use helix_db::{dsl::prelude::*, IndexDdlReceipt, IndexOperationStatus, OnNodes};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    num::NonZeroUsize,
    sync::Arc,
    time::Duration,
};
const ROOT: &str = "SlideTemplate";
const PART: &str = "SlideTemplatePart";
const SUBJECT: &str = "SlideTemplateSubject";
const CAPABILITY: &str = "SlideTemplateCapability";
const MAX_QUERY: usize = 2 * 1024 * 1024;
#[derive(Clone, Serialize)]
pub struct TemplateProjectionRecord {
    pub template: StoredTemplate,
    pub metadata: StoredTemplateMetadata,
    pub classification: Option<StoredSlideClassification>,
    pub preview: Option<TemplatePreviewDescriptor>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewDescriptor {
    pub content_type: String,
    pub width: u32,
    pub height: u32,
    pub url: String,
}
pub struct TemplateGraph {
    helix: Arc<HelixClient>,
    dimensions: NonZeroUsize,
    preparation: Arc<tokio::sync::Semaphore>,
}
impl TemplateGraph {
    pub fn new(helix: Arc<HelixClient>, dimensions: NonZeroUsize) -> Self {
        Self {
            helix,
            dimensions,
            preparation: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
    pub async fn initialize(&self) -> Result<(), String> {
        let mut batch = write_batch();
        let mut names = Vec::new();
        for (label, column, vector) in [
            (ROOT, "projection_key", false),
            (PART, "part_key", false),
            (SUBJECT, "projection_key", false),
            (CAPABILITY, "projection_key", false),
            (SUBJECT, "embedding", true),
            (CAPABILITY, "embedding", true),
        ] {
            let name = format!("template_index_{}", names.len());
            names.push(name.clone());
            let spec = if vector {
                IndexSpec::node_vector(
                    label,
                    column,
                    self.dimensions,
                    VectorDistanceMetric::Cosine,
                    Some("app_id"),
                )
            } else {
                IndexSpec::node_unique_equality(label, column)
            };
            batch = batch.var_as(&name, g().create_index_if_not_exists(spec));
        }
        let receipts: BTreeMap<String, IndexDdlReceipt> = self
            .helix
            .execute_write_query(
                "helix.templates.indexes",
                QueryRequest::write(batch.returning(names.clone())),
            )
            .await?;
        if names.iter().any(|name| !receipts.contains_key(name)) {
            return Err("template index initialization omitted required receipts".into());
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
                    let response: Status = self
                        .helix
                        .execute_read_query(
                            "helix.templates.index_status",
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
                    match response.status {
                        IndexOperationStatus::Succeeded { .. } => break,
                        IndexOperationStatus::Queued { .. }
                        | IndexOperationStatus::Running { .. }
                            if tokio::time::Instant::now() < deadline =>
                        {
                            tokio::time::sleep(Duration::from_millis(100)).await
                        }
                        _ => return Err("template indexes are not ready".into()),
                    }
                }
            }
        }
        Ok(())
    }
    pub async fn upsert(&self, record: TemplateProjectionRecord) -> Result<(), String> {
        let dimensions = self.dimensions;
        let permit = self
            .preparation
            .clone()
            .try_acquire_owned()
            .map_err(|_| "template projection preparation is busy")?;
        let prepared = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            prepare_record(record, dimensions)
        })
        .await
        .map_err(|_| "template projection worker failed")??;
        for query in prepared {
            self.helix
                .execute_write_query::<Value>("helix.templates.publish", query)
                .await?;
        }
        Ok(())
    }
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let mut batch = write_batch();
        let mut names = Vec::new();
        for label in [ROOT, PART, SUBJECT, CAPABILITY] {
            let name = format!("removed_{}", names.len());
            names.push(name.clone());
            batch = batch.var_as(&name, scoped(label, id).drop());
        }
        self.helix
            .execute_write_query::<Value>(
                "helix.templates.delete",
                bounded(QueryRequest::write(batch.returning(names)))?,
            )
            .await?;
        Ok(())
    }
    pub async fn reconcile(&self, canonical_ids: &[String]) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Id {
            template_id: String,
        }
        #[derive(Deserialize)]
        struct Ids {
            templates: Vec<Id>,
        }
        // Include unfinished parts, so interrupted prepublication writes are discovered on restart.
        let mut found = HashSet::new();
        for label in [ROOT, PART, SUBJECT, CAPABILITY] {
            let query = QueryRequest::read(
                read_batch()
                    .var_as(
                        "templates",
                        g().n_with_label(label)
                            .where_(Predicate::eq("app_id", DEFAULT_APP_ID))
                            .project(vec![PropertyProjection::new("template_id")]),
                    )
                    .returning(["templates"]),
            );
            let response: Ids = self
                .helix
                .execute_read_query("helix.templates.reconcile", query)
                .await?;
            for id in response.templates {
                found.insert(id.template_id);
            }
        }
        let canonical = canonical_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for id in found {
            if !canonical.contains(id.as_str()) {
                self.delete(&id).await?;
            }
        }
        Ok(())
    }
}
fn scoped(label: &str, id: &str) -> Traversal<OnNodes> {
    g().n_with_label(label)
        .where_(Predicate::eq("app_id", DEFAULT_APP_ID))
        .where_(Predicate::eq("template_id", id.to_owned()))
}
fn bounded(query: QueryRequest) -> Result<QueryRequest, String> {
    if query
        .to_json_bytes()
        .map_err(|_| "invalid template query")?
        .len()
        > MAX_QUERY
    {
        return Err("template projection query exceeds 2 MiB".into());
    }
    Ok(query)
}
/// Stage bounded document/metadata parts, then atomically replace the searchable root and both vector roles.
pub(crate) fn prepare_record(
    record: TemplateProjectionRecord,
    dimensions: NonZeroUsize,
) -> Result<Vec<QueryRequest>, String> {
    let id = &record.template.template_id;
    let bytes = serde_json::to_vec(&record).map_err(|_| "invalid template projection")?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("template projection exceeds 64 MiB".into());
    }
    let generation = format!("{:x}", Sha256::digest(&bytes));
    let text = String::from_utf8(bytes).map_err(|_| "invalid template encoding")?;
    let key = format!("{DEFAULT_APP_ID}:{id}");
    let mut queries = Vec::new();
    let mut offset = 0;
    let mut part = 0;
    while offset < text.len() {
        let mut end = (offset + 128 * 1024).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let part_key = format!("{key}:{generation}:{part}");
        queries.push(bounded(QueryRequest::write(
            write_batch()
                .var_as(
                    "old",
                    g().n_with_label(PART)
                        .where_(Predicate::eq("part_key", part_key.clone()))
                        .drop(),
                )
                .var_as(
                    "part",
                    g().add_n(
                        PART,
                        vec![
                            ("part_key", PropertyInput::from(part_key)),
                            ("app_id", PropertyInput::from(DEFAULT_APP_ID)),
                            ("template_id", PropertyInput::from(id.clone())),
                            ("generation", PropertyInput::from(generation.clone())),
                            ("part_index", PropertyInput::from(i64::from(part))),
                            (
                                "json_fragment",
                                PropertyInput::from(text[offset..end].to_owned()),
                            ),
                        ],
                    ),
                )
                .returning(["part"]),
        ))?);
        offset = end;
        part += 1;
    }
    let mut batch = write_batch()
        .var_as("old_root", scoped(ROOT, id).drop())
        .var_as("old_subject", scoped(SUBJECT, id).drop())
        .var_as("old_capability", scoped(CAPABILITY, id).drop());
    let mut properties = vec![
        ("projection_key", PropertyInput::from(key.clone())),
        ("app_id", PropertyInput::from(DEFAULT_APP_ID)),
        ("template_id", PropertyInput::from(id.clone())),
        ("generation", PropertyInput::from(generation.clone())),
        ("title", PropertyInput::from(record.metadata.title.clone())),
        (
            "description",
            PropertyInput::from(record.metadata.description.clone()),
        ),
        ("kind", PropertyInput::from(record.metadata.kind.as_str())),
        (
            "source",
            PropertyInput::from(record.metadata.source.as_str()),
        ),
        (
            "created_at",
            PropertyInput::from(record.metadata.created_at),
        ),
        ("part_count", PropertyInput::from(i64::from(part))),
        (
            "preview_available",
            PropertyInput::from(record.preview.is_some()),
        ),
    ];
    if let Some(c) = &record.classification {
        properties.push((
            "classification_status",
            PropertyInput::from(c.status.clone()),
        ));
        properties.push((
            "embedding_status",
            PropertyInput::from(c.embedding_status.clone()),
        ));
        if let Some(m) = &c.metadata {
            properties.push((
                "subject_summary",
                PropertyInput::from(m.subject.summary.clone()),
            ));
            properties.push((
                "retrieval_keywords",
                PropertyInput::from(m.retrieval_keywords.join(" ")),
            ));
        }
    }
    batch = batch.var_as("template", g().add_n(ROOT, properties));
    if let Some(c) = record.classification.filter(|c| {
        c.status == "ready"
            && c.embedding_status == "ready"
            && c.classified_fingerprint.as_deref() == Some(c.input_fingerprint.as_str())
    }) {
        if c.embedding_dimensions != Some(dimensions.get()) {
            return Err("template embedding dimension differs from Helix index".into());
        }
        for (name, label, vector, document, fingerprint) in [
            (
                "subject",
                SUBJECT,
                c.subject_vector,
                c.subject_embedding_document,
                c.subject_embedding_fingerprint,
            ),
            (
                "capability",
                CAPABILITY,
                c.capability_vector,
                c.capability_embedding_document,
                c.capability_embedding_fingerprint,
            ),
        ] {
            let vector = vector.ok_or("template embedding is missing")?;
            super::validate_embedding(&vector, Some(dimensions.get()))
                .map_err(|_| "invalid template embedding")?;
            let vector = vector
                .into_iter()
                .map(|v| {
                    let f = v as f32;
                    if !f.is_finite() {
                        Err("template embedding is outside f32 range")
                    } else {
                        Ok(f)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            batch = batch.var_as(
                name,
                g().add_n(
                    label,
                    vec![
                        ("projection_key", PropertyInput::from(key.clone())),
                        ("app_id", PropertyInput::from(DEFAULT_APP_ID)),
                        ("template_id", PropertyInput::from(id.clone())),
                        (
                            "embedding",
                            PropertyInput::from(PropertyValue::F32Array(vector)),
                        ),
                        (
                            "embedding_model",
                            PropertyInput::from(c.embedding_model.clone().unwrap_or_default()),
                        ),
                        (
                            "embedding_dimensions",
                            PropertyInput::from(
                                i64::try_from(dimensions.get())
                                    .map_err(|_| "invalid dimensions")?,
                            ),
                        ),
                        (
                            "embedding_document",
                            PropertyInput::from(document.unwrap_or_default()),
                        ),
                        (
                            "embedding_fingerprint",
                            PropertyInput::from(fingerprint.unwrap_or_default()),
                        ),
                    ],
                ),
            );
        }
    }
    queries.push(bounded(QueryRequest::write(batch.returning(["template"])))?);
    queries.push(bounded(QueryRequest::write(
        write_batch()
            .var_as(
                "removed",
                scoped(PART, id)
                    .where_(Predicate::neq("generation", generation))
                    .drop(),
            )
            .returning(["removed"]),
    ))?);
    Ok(queries)
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/templates/slide_templates/graph_tests.rs"]
mod tests;
