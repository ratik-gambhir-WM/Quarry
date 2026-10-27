//! Atomic classification state, dual embeddings, and FTS publication.
use super::*;
struct Row {
    state: StoredSlideClassification,
    lease: Option<String>,
    embedding_lease: Option<String>,
}

fn map_row(row: &rusqlite::Row<'_>) -> anyhow::Result<Row> {
    let metadata = row
        .get::<_, Option<String>>("metadata_json")?
        .map(|json| normalize_slide_retrieval_metadata(serde_json::from_str(&json)?))
        .transpose()?;
    let dimensions = row
        .get::<_, Option<i64>>("embedding_dimensions")?
        .map(usize::try_from)
        .transpose()?;
    let subject_embedding = row.get::<_, Option<Vec<u8>>>("subject_embedding")?;
    let subject_vector = match (subject_embedding, dimensions) {
        (Some(bytes), Some(dimensions)) => Some(deserialize_embedding(&bytes, dimensions)?),
        _ => None,
    };
    let capability_embedding = row.get::<_, Option<Vec<u8>>>("capability_embedding")?;
    let capability_vector = match (capability_embedding, dimensions) {
        (Some(bytes), Some(dimensions)) => Some(deserialize_embedding(&bytes, dimensions)?),
        _ => None,
    };
    Ok(Row {
        lease: row.get("lease_expires_at")?,
        embedding_lease: row.get("embedding_lease_expires_at")?,
        state: StoredSlideClassification {
            template_id: row.get("template_id")?,
            status: row.get("status")?,
            metadata,
            embedding_status: row.get("embedding_status")?,
            capability_embedding_document: row.get("capability_embedding_document")?,
            capability_embedding_fingerprint: row.get("capability_embedding_fingerprint")?,
            capability_vector,
            embedding_model: row.get("embedding_model")?,
            embedding_dimensions: dimensions,
            embedding_attempt_count: usize::try_from(
                row.get::<_, i64>("embedding_attempt_count")?,
            )?,
            embedding_next_attempt_at: row.get("embedding_next_attempt_at")?,
            embedding_last_error_code: row.get("embedding_last_error_code")?,
            input_fingerprint: row.get("input_fingerprint")?,
            classified_fingerprint: row.get("classified_fingerprint")?,
            schema_version: row.get("schema_version")?,
            prompt_version: row.get("prompt_version")?,
            model: row.get("model")?,
            attempt_count: usize::try_from(row.get::<_, i64>("attempt_count")?)?,
            next_attempt_at: row.get("next_attempt_at")?,
            classified_at: row.get("classified_at")?,
            last_error_code: row.get("last_error_code")?,
            subject_embedding_document: row.get("subject_embedding_document")?,
            subject_embedding_fingerprint: row.get("subject_embedding_fingerprint")?,
            subject_vector,
        },
    })
}

fn save(connection: &SqliteTransaction<'_>, row: &Row) -> anyhow::Result<()> {
    let state = &row.state;
    let metadata = state.metadata.as_ref();
    let metadata_json = metadata.map(serde_json::to_string).transpose()?;
    let subject_embedding = state
        .subject_vector
        .as_deref()
        .map(serialize_embedding)
        .transpose()?;
    let capability_embedding = state
        .capability_vector
        .as_deref()
        .map(serialize_embedding)
        .transpose()?;
    connection.write(
        SqlBuilder::update("slide_classifications")
            .set("status", &state.status)
            .set("metadata_json", metadata_json)
            .set(
                "archetype",
                metadata.map(|m| m.template_fit.archetype.as_str()),
            )
            .set(
                "layout_type",
                metadata.map(|m| m.visual.layout_type.as_str()),
            )
            .set(
                "content_density",
                metadata.map(|m| m.visual.content_density.as_str()),
            )
            .set("embedding_status", &state.embedding_status)
            .set(
                "subject_embedding_document",
                state.subject_embedding_document.as_deref(),
            )
            .set("subject_embedding", subject_embedding)
            .set(
                "subject_embedding_fingerprint",
                state.subject_embedding_fingerprint.as_deref(),
            )
            .set(
                "capability_embedding_document",
                state.capability_embedding_document.as_deref(),
            )
            .set("capability_embedding", capability_embedding)
            .set(
                "capability_embedding_fingerprint",
                state.capability_embedding_fingerprint.as_deref(),
            )
            .set("embedding_model", state.embedding_model.as_deref())
            .set(
                "embedding_dimensions",
                state.embedding_dimensions.map(i64::try_from).transpose()?,
            )
            .set(
                "embedding_attempt_count",
                i64::try_from(state.embedding_attempt_count)?,
            )
            .set(
                "embedding_next_attempt_at",
                state.embedding_next_attempt_at.as_deref(),
            )
            .set("embedding_lease_expires_at", row.embedding_lease.as_deref())
            .set(
                "embedding_last_error_code",
                state.embedding_last_error_code.as_deref(),
            )
            .set("input_fingerprint", &state.input_fingerprint)
            .set(
                "classified_fingerprint",
                state.classified_fingerprint.as_deref(),
            )
            .set("schema_version", state.schema_version)
            .set("prompt_version", &state.prompt_version)
            .set("model", state.model.as_deref())
            .set("attempt_count", i64::try_from(state.attempt_count)?)
            .set("next_attempt_at", state.next_attempt_at.as_deref())
            .set("lease_expires_at", row.lease.as_deref())
            .set("classified_at", state.classified_at.as_deref())
            .set("last_error_code", state.last_error_code.as_deref())
            .and_where(Condition::equal("template_id", &state.template_id))
            .build(),
    )?;
    Ok(())
}
fn fts(
    connection: &SqliteTransaction<'_>,
    id: &str,
    metadata: &SlideRetrievalMetadata,
) -> anyhow::Result<()> {
    let json = connection
        .read_one_with(
            SqlBuilder::select("templates")
                .columns(["template_json"])
                .and_where(Condition::equal("template_id", id))
                .build(),
            |r| r.get::<_, String>(0),
        )?
        .context("missing template")?;
    let document: CanvasDocument = decode_document(&json)?;
    connection.write(
        SqlBuilder::delete_from("slide_classifications_fts")
            .and_where(Condition::equal("template_id", id))
            .build(),
    )?;
    let visual_structure = std::iter::once(metadata.visual.layout_type.as_str())
        .chain(metadata.visual.visual_elements.iter().map(String::as_str))
        .chain(
            metadata
                .visual
                .structural_features
                .iter()
                .map(String::as_str),
        )
        .collect::<Vec<_>>()
        .join(" ");
    connection.write(
        SqlBuilder::insert_into("slide_classifications_fts")
            .value("template_id", id)
            .value("title", &document.presentation.title)
            .value("subject_summary", &metadata.subject.summary)
            .value(
                "domains",
                metadata
                    .subject
                    .domains
                    .iter()
                    .map(|domain| domain.id.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .value(
                "topics",
                metadata
                    .subject
                    .domains
                    .iter()
                    .flat_map(|domain| domain.topics.iter())
                    .chain(metadata.subject.other_topics.iter())
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .value("technologies", metadata.subject.technologies.join(" "))
            .value("entities", metadata.subject.entities.join(" "))
            .value("claims", metadata.subject.claims.join(" "))
            .value("synonyms", metadata.subject.synonyms.join(" "))
            .value("intents", metadata.communication.intents.join(" "))
            .value(
                "information_types",
                metadata.communication.information_types.join(" "),
            )
            .value("audience", metadata.communication.audience.join(" "))
            .value("archetype", &metadata.template_fit.archetype)
            .value(
                "content_slots",
                metadata
                    .template_fit
                    .content_slots
                    .iter()
                    .map(|slot| format!("{} {}", slot.role, slot.capacity))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .value("visual_structure", visual_structure)
            .value("retrieval_keywords", metadata.retrieval_keywords.join(" "))
            .build(),
    )?;
    Ok(())
}
fn load(tx: &SqliteTransaction<'_>, id: &str) -> anyhow::Result<Option<Row>> {
    Ok(tx
        .read_result_with(
            SqlBuilder::select("slide_classifications")
                .and_where(Condition::equal("template_id", id))
                .build(),
            map_row,
        )?
        .pop())
}
fn all_rows(tx: &SqliteTransaction<'_>) -> anyhow::Result<Vec<Row>> {
    tx.read_result_with(SqlBuilder::select("slide_classifications").build(), map_row)
}
impl SqliteTemplateRepository {
    pub fn complete_slide_classification(
        &self,
        result: &CompleteSlideClassification,
    ) -> anyhow::Result<()> {
        self.publish(result, None, None)
    }
    pub fn complete_slide_processing(
        &self,
        result: &CompleteSlideProcessing,
    ) -> anyhow::Result<()> {
        self.publish(&result.classification, Some(result), None)
    }
    pub(super) fn publish(
        &self,
        result: &CompleteSlideClassification,
        processed: Option<&CompleteSlideProcessing>,
        cancellation: Option<&super::super::RequestCancellation>,
    ) -> anyhow::Result<()> {
        self.client.transaction_result(|tx| {
            let mut row =
                load(tx, &result.template_id)?.context("classification no longer exists")?;
            ensure!(
                row.state.status == "processing"
                    && row.state.input_fingerprint == result.classified_fingerprint,
                "classification lease no longer active"
            );
            let metadata =
                normalize_slide_retrieval_metadata(serde_json::to_value(&result.metadata)?)?;
            row.state.status = "ready".into();
            row.state.metadata = Some(metadata.clone());
            row.state.classified_fingerprint = Some(result.classified_fingerprint.clone());
            row.state.model = Some(result.model.clone());
            row.state.classified_at = Some(result.classified_at.clone());
            row.lease = None;
            row.state.next_attempt_at = None;
            row.state.last_error_code = None;
            row.state.subject_embedding_document = Some(result.subject_embedding_document.clone());
            row.state.subject_embedding_fingerprint =
                Some(result.subject_embedding_fingerprint.clone());
            row.state.capability_embedding_document =
                Some(result.capability_embedding_document.clone());
            row.state.capability_embedding_fingerprint =
                Some(result.capability_embedding_fingerprint.clone());
            row.state.embedding_next_attempt_at = None;
            row.embedding_lease = None;
            row.state.embedding_last_error_code = None;
            if let Some(p) = processed {
                serialize_embedding(&p.subject_vector)?;
                serialize_embedding(&p.capability_vector)?;
                ensure!(
                    p.subject_vector.len() == p.embedding_dimensions
                        && p.capability_vector.len() == p.embedding_dimensions,
                    "Embedding dimensions do not match the vector."
                );
                row.state.subject_vector = Some(p.subject_vector.clone());
                row.state.capability_vector = Some(p.capability_vector.clone());
                row.state.embedding_model = Some(p.embedding_model.clone());
                row.state.embedding_dimensions = Some(p.embedding_dimensions);
                row.state.embedding_status = "ready".into();
                row.state.embedding_attempt_count = 1;
            } else {
                row.state.subject_vector = None;
                row.state.capability_vector = None;
                row.state.embedding_model = None;
                row.state.embedding_dimensions = None;
                row.state.embedding_status = "pending".into();
            }
            check_publication(cancellation)?;
            save(tx, &row)?;
            fts(tx, &result.template_id, &metadata)?;
            check_publication(cancellation)?;
            Ok(())
        })
    }
    pub fn complete_slide_embedding(&self, result: &CompleteSlideEmbedding) -> anyhow::Result<()> {
        self.client.transaction_result(|c| {
            let mut row = load(c, &result.template_id)?.context("embedding no longer exists")?;
            ensure!(
                row.state.embedding_status == "processing"
                    && row.state.subject_embedding_fingerprint.as_deref()
                        == Some(&result.subject_fingerprint)
                    && row.state.capability_embedding_fingerprint.as_deref()
                        == Some(&result.capability_fingerprint),
                "embedding lease no longer active"
            );
            serialize_embedding(&result.subject_vector)?;
            serialize_embedding(&result.capability_vector)?;
            ensure!(
                result.subject_vector.len() == result.dimensions
                    && result.capability_vector.len() == result.dimensions,
                "Embedding dimensions do not match the vector."
            );
            row.state.subject_vector = Some(result.subject_vector.clone());
            row.state.capability_vector = Some(result.capability_vector.clone());
            row.state.embedding_model = Some(result.model.clone());
            row.state.embedding_dimensions = Some(result.dimensions);
            row.state.embedding_status = "ready".into();
            row.embedding_lease = None;
            row.state.embedding_next_attempt_at = None;
            row.state.embedding_last_error_code = None;
            save(c, &row)
        })
    }
    pub fn record_slide_classification_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> anyhow::Result<()> {
        self.client.transaction_result(|c| {
            if let Some(mut row) = load(c, id)?.filter(|r| r.state.status == "processing") {
                row.state.status = if retry.is_some() { "pending" } else { "failed" }.into();
                row.state.next_attempt_at = retry.map(str::to_owned);
                row.state.last_error_code = Some(code.into());
                row.lease = None;
                save(c, &row)?;
            }
            Ok(())
        })
    }
    pub fn record_slide_embedding_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> anyhow::Result<()> {
        self.client.transaction_result(|c| {
            if let Some(mut row) = load(c, id)?.filter(|r| r.state.embedding_status == "processing")
            {
                row.state.embedding_status =
                    if retry.is_some() { "pending" } else { "failed" }.into();
                row.state.embedding_next_attempt_at = retry.map(str::to_owned);
                row.state.embedding_last_error_code = Some(code.into());
                row.embedding_lease = None;
                save(c, &row)?;
            }
            Ok(())
        })
    }
    pub fn refresh_pending_slide_classification(
        &self,
        id: &str,
        pending: &PendingSlideClassification,
    ) -> anyhow::Result<bool> {
        self.client.transaction_result(|c| {
            let Some(mut row) = load(c, id)? else {
                return Ok(false);
            };
            if row.state.input_fingerprint == pending.input_fingerprint {
                return Ok(false);
            };
            row.state.status = "pending".into();
            row.state.input_fingerprint = pending.input_fingerprint.clone();
            row.state.schema_version = pending.schema_version;
            row.state.prompt_version = pending.prompt_version.clone();
            row.state.attempt_count = 0;
            row.state.next_attempt_at = None;
            row.state.last_error_code = None;
            row.lease = None;
            save(c, &row)?;
            Ok(true)
        })
    }
    pub fn refresh_pending_slide_embedding(
        &self,
        id: &str,
        subject_document: &str,
        subject_fingerprint: &str,
        capability_document: &str,
        capability_fingerprint: &str,
    ) -> anyhow::Result<bool> {
        self.client.transaction_result(|c| {
            let Some(mut row) = load(c, id)? else {
                return Ok(false);
            };
            if row.state.metadata.is_none()
                || (row.state.subject_embedding_fingerprint.as_deref() == Some(subject_fingerprint)
                    && row.state.capability_embedding_fingerprint.as_deref()
                        == Some(capability_fingerprint))
            {
                return Ok(false);
            };
            row.state.embedding_status = "pending".into();
            row.state.subject_embedding_document = Some(subject_document.into());
            row.state.subject_embedding_fingerprint = Some(subject_fingerprint.into());
            row.state.capability_embedding_document = Some(capability_document.into());
            row.state.capability_embedding_fingerprint = Some(capability_fingerprint.into());
            row.state.subject_vector = None;
            row.state.capability_vector = None;
            row.state.embedding_attempt_count = 0;
            row.state.embedding_next_attempt_at = None;
            row.state.embedding_last_error_code = None;
            row.embedding_lease = None;
            save(c, &row)?;
            Ok(true)
        })
    }

    pub fn insert_with_pending_classification(
        &self,
        r: &TemplateInsert,
        p: &PendingSlideClassification,
    ) -> anyhow::Result<()> {
        self.insert_classification(r, p, false, None)
    }
    pub fn insert_with_processing_classification(
        &self,
        r: &TemplateInsert,
        p: &PendingSlideClassification,
    ) -> anyhow::Result<()> {
        self.insert_classification(r, p, true, None)
    }
    pub(super) fn insert_classification(
        &self,
        r: &TemplateInsert,
        p: &PendingSlideClassification,
        processing: bool,
        cancellation: Option<&super::super::RequestCancellation>,
    ) -> anyhow::Result<()> {
        self.client.transaction_result(|tx| {
            check_publication(cancellation)?;
            Self::write_new(tx, &r.template, &r.assets, &r.metadata, r.preview.as_ref())?;
            membership(tx, &self.app_id, &r.template.template_id)?;
            tx.write(
                SqlBuilder::insert_into("slide_classifications")
                    .value("template_id", &r.template.template_id)
                    .value("status", if processing { "processing" } else { "pending" })
                    .value("input_fingerprint", &p.input_fingerprint)
                    .value("prompt_version", &p.prompt_version)
                    .value("schema_version", p.schema_version)
                    .value("attempt_count", i64::from(processing))
                    .build(),
            )?;
            check_publication(cancellation)
        })
    }
    pub fn find_slide_classification(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<StoredSlideClassification>> {
        self.client.transaction_result(|tx| {
            if !visible(tx, &self.app_id, id)? {
                return Ok(None);
            }
            Ok(load(tx, id)?.map(|r| r.state))
        })
    }
    pub fn claim_next_slide_classification(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> anyhow::Result<Option<SlideClassificationJob>> {
        self.client.transaction_result(|tx| {
            let mut rows = all_rows(tx)?;
            for row in &mut rows {
                if row.state.status == "processing"
                    && row.lease.as_deref().is_some_and(|t| t <= now)
                {
                    row.state.status = "pending".into();
                    row.lease = None;
                    save(tx, row)?;
                }
            }
            rows.sort_by(|a, b| {
                (&a.state.next_attempt_at, &a.state.template_id)
                    .cmp(&(&b.state.next_attempt_at, &b.state.template_id))
            });
            let Some(mut row) = rows.into_iter().find(|r| {
                r.state.status == "pending"
                    && r.state.next_attempt_at.as_deref().is_none_or(|t| t <= now)
            }) else {
                return Ok(None);
            };
            row.state.status = "processing".into();
            row.state.attempt_count += 1;
            row.lease = Some(lease_expires_at.into());
            row.state.next_attempt_at = None;
            row.state.last_error_code = None;
            save(tx, &row)?;
            let json = tx
                .read_one_with(
                    SqlBuilder::select("templates")
                        .columns(["template_json"])
                        .and_where(Condition::equal("template_id", &row.state.template_id))
                        .build(),
                    |r| r.get::<_, String>(0),
                )?
                .context("missing template")?;
            let document = decode_document(&json)?;
            let kind = metadata_in(tx, &row.state.template_id)?
                .context("missing metadata")?
                .kind
                .as_str()
                .to_owned();
            let preview = tx.read_one_with(
                SqlBuilder::select("template_previews")
                    .and_where(Condition::equal("template_id", &row.state.template_id))
                    .build(),
                map_preview,
            )?;
            let job = SlideClassificationJob {
                attempt_count: row.state.attempt_count,
                input_fingerprint: row.state.input_fingerprint,
                kind: if kind == "commentary" {
                    TemplateKind::Commentary
                } else {
                    TemplateKind::Diagram
                },
                preview,
                title: document.presentation.title.clone(),
                template: StoredTemplate {
                    template_id: row.state.template_id,
                    template_json: document,
                },
            };
            Ok(Some(job))
        })
    }
    pub fn claim_next_slide_embedding(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> anyhow::Result<Option<SlideEmbeddingJob>> {
        self.client.transaction_result(|tx| {
            let mut rows = all_rows(tx)?;
            for row in &mut rows {
                if row.state.embedding_status == "processing"
                    && row.embedding_lease.as_deref().is_some_and(|t| t <= now)
                {
                    row.state.embedding_status = "pending".into();
                    row.embedding_lease = None;
                    save(tx, row)?;
                }
            }
            rows.sort_by(|a, b| {
                (&a.state.embedding_next_attempt_at, &a.state.template_id)
                    .cmp(&(&b.state.embedding_next_attempt_at, &b.state.template_id))
            });
            let Some(mut row) = rows.into_iter().find(|r| {
                r.state.embedding_status == "pending"
                    && r.state.subject_embedding_document.is_some()
                    && r.state.subject_embedding_fingerprint.is_some()
                    && r.state.capability_embedding_document.is_some()
                    && r.state.capability_embedding_fingerprint.is_some()
                    && r.state
                        .embedding_next_attempt_at
                        .as_deref()
                        .is_none_or(|t| t <= now)
            }) else {
                return Ok(None);
            };
            row.state.embedding_status = "processing".into();
            row.state.embedding_attempt_count += 1;
            row.embedding_lease = Some(lease_expires_at.into());
            row.state.embedding_next_attempt_at = None;
            row.state.embedding_last_error_code = None;
            save(tx, &row)?;
            let job = SlideEmbeddingJob {
                attempt_count: row.state.embedding_attempt_count,
                subject_document: row
                    .state
                    .subject_embedding_document
                    .context("missing subject embedding document")?,
                subject_fingerprint: row
                    .state
                    .subject_embedding_fingerprint
                    .context("missing subject embedding fingerprint")?,
                capability_document: row
                    .state
                    .capability_embedding_document
                    .context("missing capability embedding document")?,
                capability_fingerprint: row
                    .state
                    .capability_embedding_fingerprint
                    .context("missing capability embedding fingerprint")?,
                template_id: row.state.template_id,
            };
            Ok(Some(job))
        })
    }

    pub fn list_retrieval_classifications(
        &self,
        app_id: &str,
    ) -> anyhow::Result<Vec<RetrievalClassificationRecord>> {
        self.client.transaction_result(|tx| {
            let rows = tx.read_result_with(
                SqlBuilder::select("slide_classifications")
                    .columns(["slide_classifications.*"])
                    .join(JoinClause::inner("app_templates").on(JoinCondition::equal(
                        "app_templates.template_id",
                        "slide_classifications.template_id",
                    )))
                    .and_where(Condition::equal("app_templates.app_id", app_id))
                    .and_where(Condition::is_not_null("metadata_json"))
                    .and_where(Condition::equal("schema_version", 2))
                    .build(),
                map_row,
            )?;
            let mut result = Vec::new();
            for row in rows {
                let id = &row.state.template_id;
                let meta = metadata_in(tx, id)?.context("missing metadata")?;
                let preview_available = tx
                    .read_one(
                        SqlBuilder::select("template_previews")
                            .and_where(Condition::equal("template_id", id))
                            .build(),
                    )?
                    .is_some();
                result.push(RetrievalClassificationRecord {
                    template_id: id.clone(),
                    classification: row.state,
                    created_at: timestamp_iso(meta.created_at)?,
                    kind: meta.kind,
                    preview_available,
                    title: meta.title,
                });
            }
            result.sort_by(|a, b| {
                b.created_at
                    .cmp(&a.created_at)
                    .then(a.template_id.cmp(&b.template_id))
            });
            Ok(result)
        })
    }
    pub fn search_slide_classifications(
        &self,
        app_id: &str,
        fts_query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<String>> {
        Ok(self.client.read_with(
            SqlBuilder::select("slide_classifications_fts")
                .column("slide_classifications_fts.template_id")
                .inner_join(
                    "app_templates",
                    "app_templates.template_id",
                    "slide_classifications_fts.template_id",
                )
                .inner_join(
                    "slide_classifications",
                    "slide_classifications.template_id",
                    "slide_classifications_fts.template_id",
                )
                .and_where(Condition::equal("app_templates.app_id", app_id))
                .and_where(Condition::is_not_null(
                    "slide_classifications.metadata_json",
                ))
                .and_where(Condition::equal("slide_classifications.schema_version", 2))
                .and_where(Condition::full_text_match(
                    "slide_classifications_fts",
                    fts_query,
                ))
                .order_by_bm25(
                    "slide_classifications_fts",
                    vec![
                        0.0, 2.0, 5.0, 5.0, 5.0, 2.0, 2.0, 3.0, 3.0, 4.0, 3.0, 1.0, 4.0, 4.0, 2.0,
                        3.0,
                    ],
                )
                .order_by(
                    "slide_classifications_fts.template_id",
                    SortDirection::Ascending,
                )
                .limit(limit)
                .build(),
            |r| r.get(0),
        )?)
    }
}
