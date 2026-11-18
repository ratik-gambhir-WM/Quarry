//! Durable classification/embedding stages, typed vector pairs and lexical publication.
//!
//! Every stage mutation locks the canonical template parent first, then fences on the
//! stage's opaque claim token, `processing` status and database-clock lease. Classification
//! and embedding stages are updated independently, so neither can reset the other.
use super::*;
use crate::adapters::postgres::{query::SqlValue, vector::CosineVector};

pub(super) const STATUS: &str = "template_classification_status";

/// A stage attempt no longer owns its row: a successor claimed it, it was deleted, or its
/// database-clock lease expired. The caller must not treat its prepared result as published.
#[derive(Debug, thiserror::Error)]
#[error("template stage claim is no longer active")]
pub struct StaleStageClaim;

struct StageColumns {
    status: &'static str,
    attempts: &'static str,
    retry: &'static str,
    token: &'static str,
    lease: &'static str,
    error: &'static str,
}

const fn columns(stage: SlideProcessingStage) -> StageColumns {
    match stage {
        SlideProcessingStage::Classification => StageColumns {
            status: "status",
            attempts: "attempt_count",
            retry: "next_attempt_at",
            token: "claim_token",
            lease: "lease_until",
            error: "last_error_code",
        },
        SlideProcessingStage::Embedding => StageColumns {
            status: "embedding_status",
            attempts: "embedding_attempt_count",
            retry: "embedding_next_attempt_at",
            token: "embedding_claim_token",
            lease: "embedding_lease_until",
            error: "embedding_last_error_code",
        },
    }
}

fn epoch_iso(seconds: Option<i64>) -> anyhow::Result<Option<String>> {
    seconds
        .map(|seconds| {
            chrono::DateTime::from_timestamp(seconds, 0)
                .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
                .context("invalid stage retry time")
        })
        .transpose()
}

pub(super) fn map_state(
    row: &crate::adapters::postgres::client::Row<'_>,
) -> anyhow::Result<StoredSlideClassification> {
    let metadata = row
        .get::<_, Option<String>>("metadata_json")?
        .map(|json| normalize_slide_retrieval_metadata(serde_json::from_str(&json)?))
        .transpose()?;
    Ok(StoredSlideClassification {
        template_id: row.get("template_id")?,
        status: row.get("status")?,
        metadata,
        embedding_status: row.get("embedding_status")?,
        capability_embedding_document: row.get("capability_embedding_document")?,
        capability_embedding_fingerprint: row.get("capability_embedding_fingerprint")?,
        capability_vector: None,
        embedding_model: None,
        embedding_dimensions: None,
        embedding_profile_id: row.get("embedding_profile_id")?,
        embedding_attempt_count: row.get::<_, usize>("embedding_attempt_count")?,
        embedding_next_attempt_at: epoch_iso(row.get("embedding_next_attempt_at")?)?,
        embedding_last_error_code: row.get("embedding_last_error_code")?,
        input_fingerprint: row.get("input_fingerprint")?,
        classified_fingerprint: row.get("classified_fingerprint")?,
        schema_version: row.get("schema_version")?,
        prompt_version: row.get("prompt_version")?,
        model: row.get("model")?,
        attempt_count: row.get::<_, usize>("attempt_count")?,
        next_attempt_at: epoch_iso(row.get("next_attempt_at")?)?,
        classified_at: row.get("classified_at")?,
        last_error_code: row.get("last_error_code")?,
        subject_embedding_document: row.get("subject_embedding_document")?,
        subject_embedding_fingerprint: row.get("subject_embedding_fingerprint")?,
        subject_vector: None,
    })
}

async fn fts(
    connection: &PostgresTransaction,
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
        )
        .await?
        .context("missing template")?;
    let document: CanvasDocument = decode_document(json).await?;
    clear_lexical(connection, id).await?;
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
    let query = SqlBuilder::insert_into("slide_classifications_fts")
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
        .build()?;
    let fields = query
        .parameters()
        .iter()
        .skip(1)
        .map(|v| match v {
            SqlValue::Text(s) => Ok(s.clone()),
            _ => Err(anyhow::anyhow!("invalid lexical field")),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    connection.write(Ok(query)).await?;
    crate::adapters::postgres::query::lexical::index(connection, id, &fields).await?;
    Ok(())
}

/// Postings cascade from the lexical row.
async fn clear_lexical(tx: &PostgresTransaction, id: &str) -> anyhow::Result<()> {
    tx.write(
        SqlBuilder::delete_from("slide_classifications_fts")
            .and_where(Condition::equal("template_id", id))
            .build(),
    )
    .await?;
    Ok(())
}

async fn clear_embeddings(tx: &PostgresTransaction, id: &str) -> anyhow::Result<()> {
    tx.write(
        SqlBuilder::delete_from("template_embeddings")
            .and_where(Condition::equal("template_id", id))
            .build(),
    )
    .await?;
    Ok(())
}

async fn load(
    tx: &PostgresTransaction,
    id: &str,
) -> anyhow::Result<Option<StoredSlideClassification>> {
    Ok(tx
        .read_result_with(
            SqlBuilder::select(STATUS)
                .and_where(Condition::equal("template_id", id))
                .build(),
            map_state,
        )
        .await?
        .pop())
}

// Canonical parent first: refresh, delete, publication and claims share this order.
async fn lock_template(tx: &PostgresTransaction, id: &str) -> anyhow::Result<bool> {
    Ok(!tx
        .lock_with(
            SqlBuilder::select("templates")
                .columns(["template_id"])
                .and_where(Condition::equal("template_id", id))
                .lock(RowLock::Update, LockWait::Wait)
                .build(),
            |_| Ok(()),
        )
        .await?
        .is_empty())
}

/// Token, status and database-clock lease predicates for one active stage attempt.
fn fenced(
    update: crate::adapters::postgres::query::UpdateBuilder,
    stage: SlideProcessingStage,
    claim: &StageClaim,
) -> crate::adapters::postgres::query::UpdateBuilder {
    let c = columns(stage);
    update
        .and_where(Condition::equal("template_id", &claim.template_id))
        .and_where(Condition::equal(c.token, &claim.token))
        .and_where(Condition::equal(c.status, "processing"))
        .and_where(Condition::unexpired(c.lease))
}

/// Reset an embedding stage that has no current input. Vectors must be cleared in the
/// same transaction so the deferred pair check sees no rows for a non-ready stage.
fn reset_embedding(
    update: crate::adapters::postgres::query::UpdateBuilder,
    status: &str,
) -> crate::adapters::postgres::query::UpdateBuilder {
    update
        .set("embedding_status", status)
        .set("embedding_profile_id", None::<String>)
        .set("embedding_attempt_count", 0_i64)
        .set("embedding_next_attempt_at", None::<i64>)
        .set("embedding_claim_token", None::<String>)
        .set("embedding_lease_until", None::<i64>)
        .set("embedding_last_error_code", None::<String>)
}

// Claims and idle detection must apply identical ownership and stage prerequisites.
fn stage_jobs(
    scope: &PostgresTemplateRepository,
    stage: SlideProcessingStage,
) -> crate::adapters::postgres::query::SelectBuilder {
    let mut query = SqlBuilder::select(STATUS)
        .inner_join(
            "templates",
            "templates.template_id",
            "template_classification_status.template_id",
        )
        .and_where(scope.owner_condition());
    if stage == SlideProcessingStage::Embedding {
        query = query.and_where(Condition::equal(
            "template_classification_status.status",
            "ready",
        ));
        for column in [
            "subject_embedding_document",
            "subject_embedding_fingerprint",
            "capability_embedding_document",
            "capability_embedding_fingerprint",
        ] {
            query = query.and_where(Condition::is_not_null(column));
        }
    }
    query
}

/// Claim one due stage with a fresh token and a database-clock lease. Expired leases are
/// reclaimed first; due pending rows follow, unscheduled work first.
async fn claim_next(
    tx: &PostgresTransaction,
    scope: &PostgresTemplateRepository,
    stage: SlideProcessingStage,
    lease_seconds: u32,
) -> anyhow::Result<Option<(StageClaim, usize)>> {
    let c = columns(stage);
    for (state, due, order) in [
        ("processing", Condition::expired(c.lease), c.lease),
        ("pending", Condition::is_null(c.retry), c.retry),
        ("pending", Condition::expired(c.retry), c.retry),
    ] {
        let candidate = tx
            .read_one_with(
                stage_jobs(scope, stage)
                    .columns(["template_classification_status.template_id"])
                    .and_where(Condition::equal(c.status, state))
                    .and_where(due.clone())
                    .order_by(order, SortDirection::Ascending)
                    .order_by(
                        "template_classification_status.template_id",
                        SortDirection::Ascending,
                    )
                    .limit(1)
                    .build(),
                |r| r.get::<_, String>(0),
            )
            .await?;
        let Some(id) = candidate else { continue };
        if !lock_template(tx, &id).await? {
            continue;
        }
        let token = uuid::Uuid::new_v4().to_string();
        // Repeat the due predicate after the parent lock wait; a competing claim wins cleanly.
        let attempts = tx
            .write_returning_with(
                SqlBuilder::update(STATUS)
                    .set(c.status, "processing")
                    .set(c.token, &token)
                    .set_epoch_seconds_after(c.lease, lease_seconds)
                    .increment(c.attempts)
                    .set(c.retry, None::<i64>)
                    .set(c.error, None::<String>)
                    .and_where(Condition::equal("template_id", &id))
                    .and_where(Condition::equal(c.status, state))
                    .and_where(due)
                    .returning(c.attempts)
                    .build(),
                |r| r.get::<_, usize>(0),
            )
            .await?
            .pop();
        if let Some(attempts) = attempts {
            return Ok(Some((
                StageClaim {
                    template_id: id,
                    token,
                },
                attempts,
            )));
        }
    }
    Ok(None)
}

#[async_trait::async_trait]
impl TemplateProcessingRepository for PostgresTemplateRepository {
    async fn has_pending_slide_processing(&self) -> RepositoryResult<bool> {
        repository_boundary(async {
            for stage in [
                SlideProcessingStage::Classification,
                SlideProcessingStage::Embedding,
            ] {
                let query = stage_jobs(self, stage)
                    .columns(["template_classification_status.template_id"])
                    .and_where(Condition::in_values(
                        columns(stage).status,
                        ["pending", "processing"],
                    ));
                if self
                    .client
                    .read_one_with_async(query.limit(1).build(), |_| Ok(()))
                    .await
                    .context("read pending slide work")?
                    .is_some()
                {
                    return Ok(true);
                }
            }
            Ok(false)
        })
        .await
    }
    /// Publish metadata, lexical postings and queued embedding input under the claim fence.
    /// Obsolete vectors are removed in the same transaction.
    async fn complete_slide_classification(
        &self,
        claim: &StageClaim,
        result: &CompleteSlideClassification,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            ensure!(
                claim.template_id == result.template_id,
                "classification claim does not match its result"
            );
            let metadata =
                normalize_slide_retrieval_metadata(serde_json::to_value(&result.metadata)?)?;
            let metadata_json = serde_json::to_string(&metadata)?;
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !lock_template(tx, &claim.template_id).await? {
                        return Err(StaleStageClaim.into());
                    }
                    clear_embeddings(tx, &claim.template_id).await?;
                    fts(tx, &claim.template_id, &metadata).await?;
                    // The fence is the final mutation: a lease lost while writing postings rolls back.
                    let update = reset_embedding(
                        SqlBuilder::update(STATUS)
                            .set("status", "ready")
                            .set("metadata_json", metadata_json)
                            .set("archetype", metadata.template_fit.archetype.as_str())
                            .set("layout_type", metadata.visual.layout_type.as_str())
                            .set("content_density", metadata.visual.content_density.as_str())
                            .set("classified_fingerprint", &result.classified_fingerprint)
                            .set("model", &result.model)
                            .set("classified_at", &result.classified_at)
                            .set("claim_token", None::<String>)
                            .set("lease_until", None::<i64>)
                            .set("next_attempt_at", None::<i64>)
                            .set("last_error_code", None::<String>)
                            .set(
                                "subject_embedding_document",
                                &result.subject_embedding_document,
                            )
                            .set(
                                "subject_embedding_fingerprint",
                                &result.subject_embedding_fingerprint,
                            )
                            .set(
                                "capability_embedding_document",
                                &result.capability_embedding_document,
                            )
                            .set(
                                "capability_embedding_fingerprint",
                                &result.capability_embedding_fingerprint,
                            ),
                        "pending",
                    );
                    let changed = tx
                        .write(
                            fenced(update, SlideProcessingStage::Classification, claim)
                                .and_where(Condition::equal(
                                    "input_fingerprint",
                                    &result.classified_fingerprint,
                                ))
                                .build(),
                        )
                        .await?;
                    if changed.rows_affected != 1 {
                        return Err(StaleStageClaim.into());
                    }
                    Ok(())
                })
                .await
        })
        .await
    }
    /// Publish a complete subject/capability pair and embedding readiness in one transaction.
    /// A missing role, profile mismatch or lost fence rolls back the whole pair.
    async fn complete_slide_embedding(
        &self,
        claim: &StageClaim,
        prepared: &PreparedSlideEmbeddings,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            let profile_id = prepared.profile.id();
            let dimensions = i64::try_from(prepared.profile.dimensions)?;
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !lock_template(tx, &claim.template_id).await? {
                        return Err(StaleStageClaim.into());
                    }
                    let fence = |query: crate::adapters::postgres::query::SelectBuilder| {
                        query
                            .and_where(Condition::equal("template_id", &claim.template_id))
                            .and_where(Condition::equal("embedding_claim_token", &claim.token))
                            .and_where(Condition::equal("embedding_status", "processing"))
                            .and_where(Condition::unexpired("embedding_lease_until"))
                            .and_where(Condition::equal("status", "ready"))
                            .and_where(Condition::equal(
                                "subject_embedding_fingerprint",
                                &prepared.subject_fingerprint,
                            ))
                            .and_where(Condition::equal(
                                "capability_embedding_fingerprint",
                                &prepared.capability_fingerprint,
                            ))
                    };
                    if tx
                        .read_one(
                            fence(SqlBuilder::select(STATUS).columns(["template_id"])).build(),
                        )
                        .await?
                        .is_none()
                    {
                        return Err(StaleStageClaim.into());
                    }
                    tx.write(
                        SqlBuilder::insert_into("template_embedding_profiles")
                            .value("profile_id", &profile_id)
                            .value("provider", TemplateEmbeddingProfile::PROVIDER)
                            .value("model", &prepared.profile.model)
                            .value("dimensions", dimensions)
                            .value("metric", TemplateEmbeddingProfile::METRIC)
                            .value(
                                "preparation_version",
                                TemplateEmbeddingProfile::PREPARATION_VERSION,
                            )
                            .on_conflict_do_nothing(["profile_id"])
                            .build(),
                    )
                    .await?;
                    clear_embeddings(tx, &claim.template_id).await?;
                    let row = |role: EmbeddingRole, fingerprint: &str, vector: &CosineVector| {
                        vec![
                            SqlValue::from(claim.template_id.as_str()),
                            role.as_str().into(),
                            profile_id.as_str().into(),
                            dimensions.into(),
                            fingerprint.into(),
                            SqlValue::Vector(vector.clone()),
                        ]
                    };
                    let inserted = tx
                        .write(
                            SqlBuilder::insert_batch(
                                "template_embeddings",
                                [
                                    "template_id",
                                    "role",
                                    "profile_id",
                                    "dimensions",
                                    "input_fingerprint",
                                    "embedding",
                                ],
                                vec![
                                    row(
                                        EmbeddingRole::Subject,
                                        &prepared.subject_fingerprint,
                                        &prepared.subject,
                                    ),
                                    row(
                                        EmbeddingRole::Capability,
                                        &prepared.capability_fingerprint,
                                        &prepared.capability,
                                    ),
                                ],
                            )
                            .build(),
                        )
                        .await?;
                    ensure!(
                        inserted.rows_affected == 2,
                        "embedding pair was not inserted"
                    );
                    // Repeat the token/status/DB-clock fence after row waits and pair insertion.
                    let changed = tx
                        .write(
                            fenced(
                                SqlBuilder::update(STATUS)
                                    .set("embedding_status", "ready")
                                    .set("embedding_profile_id", &profile_id)
                                    .set("embedding_claim_token", None::<String>)
                                    .set("embedding_lease_until", None::<i64>)
                                    .set("embedding_next_attempt_at", None::<i64>)
                                    .set("embedding_last_error_code", None::<String>),
                                SlideProcessingStage::Embedding,
                                claim,
                            )
                            .and_where(Condition::equal("status", "ready"))
                            .and_where(Condition::equal(
                                "subject_embedding_fingerprint",
                                &prepared.subject_fingerprint,
                            ))
                            .and_where(Condition::equal(
                                "capability_embedding_fingerprint",
                                &prepared.capability_fingerprint,
                            ))
                            .build(),
                        )
                        .await?;
                    if changed.rows_affected != 1 {
                        return Err(StaleStageClaim.into());
                    }
                    Ok(())
                })
                .await
        })
        .await
    }
    /// Record a provider failure only for the same claim. An old timeout cannot fail a
    /// reclaimed successor, even when both attempts have identical input.
    async fn record_slide_stage_failure(
        &self,
        stage: SlideProcessingStage,
        claim: &StageClaim,
        code: &str,
        retry_after_seconds: Option<u32>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            let c = columns(stage);
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !lock_template(tx, &claim.template_id).await? {
                        return Ok(());
                    }
                    let mut update = SqlBuilder::update(STATUS)
                        .set(
                            c.status,
                            if retry_after_seconds.is_some() {
                                "pending"
                            } else {
                                "failed"
                            },
                        )
                        .set(c.error, code)
                        .set(c.token, None::<String>)
                        .set(c.lease, None::<i64>);
                    update = match retry_after_seconds {
                        Some(seconds) => update.set_epoch_seconds_after(c.retry, seconds),
                        None => update.set(c.retry, None::<i64>),
                    };
                    tx.write(
                        update
                            .and_where(Condition::equal("template_id", &claim.template_id))
                            .and_where(Condition::equal(c.token, &claim.token))
                            .and_where(Condition::equal(c.status, "processing"))
                            .build(),
                    )
                    .await?;
                    Ok(())
                })
                .await
        })
        .await
    }
    async fn retry_failed_slide_processing(
        &self,
        id: &str,
    ) -> RepositoryResult<RetrySlideProcessingOutcome> {
        repository_boundary(async {
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? || !lock_template(tx, id).await? {
                        return Ok(RetrySlideProcessingOutcome::NotFound);
                    }
                    let Some(row) = load(tx, id).await? else {
                        return Ok(RetrySlideProcessingOutcome::NotFailed);
                    };
                    // A duplicate request or an active worker must never reset an in-flight lease.
                    if row.status == "processing" || row.embedding_status == "processing" {
                        return Ok(RetrySlideProcessingOutcome::NotFailed);
                    }
                    let (update, stage) = if row.status == "failed" {
                        // Classification will enqueue new embedding input. Until then no stale
                        // embedding work or vectors may remain selectable.
                        clear_embeddings(tx, id).await?;
                        (
                            reset_embedding(
                                SqlBuilder::update(STATUS)
                                    .set("status", "pending")
                                    .set("attempt_count", 0_i64)
                                    .set("next_attempt_at", None::<i64>)
                                    .set("last_error_code", None::<String>)
                                    .and_where(Condition::equal("status", "failed")),
                                "not_ready",
                            ),
                            SlideProcessingStage::Classification,
                        )
                    } else if row.status == "ready" && row.embedding_status == "failed" {
                        ensure!(
                            row.metadata.is_some()
                                && row.subject_embedding_document.is_some()
                                && row.subject_embedding_fingerprint.is_some()
                                && row.capability_embedding_document.is_some()
                                && row.capability_embedding_fingerprint.is_some(),
                            "failed embedding job is missing classification artifacts"
                        );
                        (
                            reset_embedding(
                                SqlBuilder::update(STATUS)
                                    .and_where(Condition::equal("status", "ready"))
                                    .and_where(Condition::equal("embedding_status", "failed")),
                                "pending",
                            ),
                            SlideProcessingStage::Embedding,
                        )
                    } else {
                        return Ok(RetrySlideProcessingOutcome::NotFailed);
                    };
                    let changed = tx
                        .write(
                            update
                                .and_where(Condition::equal("template_id", id))
                                .build(),
                        )
                        .await?;
                    if changed.rows_affected != 1 {
                        return Ok(RetrySlideProcessingOutcome::NotFailed);
                    }
                    Ok(RetrySlideProcessingOutcome::Queued(stage))
                })
                .await
        })
        .await
    }
    /// Queue new classification input. Metadata, lexical postings, embedding input and
    /// vectors for the previous content are invalidated in the same transaction.
    async fn refresh_pending_slide_classification(
        &self,
        id: &str,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<bool> {
        repository_boundary(async {
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !lock_template(tx, id).await? {
                        return Ok(false);
                    }
                    let Some(row) = load(tx, id).await? else {
                        return Ok(false);
                    };
                    if row.input_fingerprint == pending.input_fingerprint {
                        return Ok(false);
                    };
                    clear_embeddings(tx, id).await?;
                    clear_lexical(tx, id).await?;
                    tx.write(
                        reset_embedding(
                            SqlBuilder::update(STATUS)
                                .set("status", "pending")
                                .set("input_fingerprint", &pending.input_fingerprint)
                                .set("schema_version", pending.schema_version)
                                .set("prompt_version", &pending.prompt_version)
                                .set("attempt_count", 0_i64)
                                .set("next_attempt_at", None::<i64>)
                                .set("claim_token", None::<String>)
                                .set("lease_until", None::<i64>)
                                .set("last_error_code", None::<String>)
                                .set("metadata_json", None::<String>)
                                .set("archetype", None::<String>)
                                .set("layout_type", None::<String>)
                                .set("content_density", None::<String>)
                                .set("classified_fingerprint", None::<String>)
                                .set("subject_embedding_document", None::<String>)
                                .set("subject_embedding_fingerprint", None::<String>)
                                .set("capability_embedding_document", None::<String>)
                                .set("capability_embedding_fingerprint", None::<String>),
                            "not_ready",
                        )
                        .and_where(Condition::equal("template_id", id))
                        .build(),
                    )
                    .await?;
                    Ok(true)
                })
                .await
        })
        .await
    }
    /// Queue new embedding input without re-running classification. Unchanged input is a no-op.
    async fn refresh_pending_slide_embedding(
        &self,
        id: &str,
        subject_document: &str,
        subject_fingerprint: &str,
        capability_document: &str,
        capability_fingerprint: &str,
    ) -> RepositoryResult<bool> {
        repository_boundary(async {
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !lock_template(tx, id).await? {
                        return Ok(false);
                    }
                    let Some(row) = load(tx, id).await? else {
                        return Ok(false);
                    };
                    if row.metadata.is_none()
                        || row.status != "ready"
                        || (row.subject_embedding_fingerprint.as_deref()
                            == Some(subject_fingerprint)
                            && row.capability_embedding_fingerprint.as_deref()
                                == Some(capability_fingerprint))
                    {
                        return Ok(false);
                    };
                    clear_embeddings(tx, id).await?;
                    tx.write(
                        reset_embedding(
                            SqlBuilder::update(STATUS)
                                .set("subject_embedding_document", subject_document)
                                .set("subject_embedding_fingerprint", subject_fingerprint)
                                .set("capability_embedding_document", capability_document)
                                .set("capability_embedding_fingerprint", capability_fingerprint),
                            "pending",
                        )
                        .and_where(Condition::equal("template_id", id))
                        .build(),
                    )
                    .await?;
                    Ok(true)
                })
                .await
        })
        .await
    }
    /// Scoped stage state; vectors are read only for the separately gated inspection view.
    async fn find_slide_classification(
        &self,
        id: &str,
    ) -> RepositoryResult<Option<StoredSlideClassification>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    let Some(mut state) = load(tx, id).await? else {
                        return Ok(None);
                    };
                    if let Some(profile_id) = state.embedding_profile_id.clone() {
                        let (model, dimensions) = tx
                            .read_one_with(
                                SqlBuilder::select("template_embedding_profiles")
                                    .columns(["model", "dimensions"])
                                    .and_where(Condition::equal("profile_id", &profile_id))
                                    .build(),
                                |r| Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?)),
                            )
                            .await?
                            .context("missing template embedding profile")?;
                        state.embedding_model = Some(model);
                        state.embedding_dimensions = Some(dimensions);
                        for (role, vector) in tx
                            .read_with(
                                SqlBuilder::select("template_embeddings")
                                    .columns(["role", "embedding"])
                                    .and_where(Condition::equal("template_id", id))
                                    .and_where(Condition::equal("profile_id", &profile_id))
                                    .build(),
                                |r| Ok((r.get::<_, String>(0)?, r.get::<_, CosineVector>(1)?)),
                            )
                            .await?
                        {
                            let values =
                                Some(vector.as_slice().iter().map(|v| f64::from(*v)).collect());
                            match role.as_str() {
                                "subject" => state.subject_vector = values,
                                "capability" => state.capability_vector = values,
                                _ => anyhow::bail!("unknown template embedding role"),
                            }
                        }
                    }
                    Ok(Some(state))
                })
                .await
        })
        .await
    }
    async fn claim_next_slide_classification(
        &self,
        lease_seconds: u32,
    ) -> RepositoryResult<Option<SlideClassificationJob>> {
        repository_boundary(async {
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let Some((claim, attempt_count)) = claim_next(
                        tx,
                        self,
                        SlideProcessingStage::Classification,
                        lease_seconds,
                    )
                    .await?
                    else {
                        return Ok(None);
                    };
                    let id = claim.template_id.clone();
                    let input_fingerprint = tx
                        .read_one_with(
                            SqlBuilder::select(STATUS)
                                .columns(["input_fingerprint"])
                                .and_where(Condition::equal("template_id", &id))
                                .build(),
                            |r| r.get::<_, String>(0),
                        )
                        .await?
                        .context("missing classification")?;
                    let (json, revision) = tx
                        .read_one_with(
                            SqlBuilder::select("templates")
                                .columns(["template_json", "revision"])
                                .and_where(Condition::equal("template_id", &id))
                                .build(),
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
                        )
                        .await?
                        .context("missing template")?;
                    let document = decode_document(json).await?;
                    let kind = metadata_in(tx, &id)
                        .await?
                        .context("missing metadata")?
                        .kind;
                    let preview = tx
                        .read_one_with(
                            SqlBuilder::select("template_previews")
                                .columns(["template_previews.*", "templates.revision"])
                                .inner_join(
                                    "templates",
                                    "templates.template_id",
                                    "template_previews.template_id",
                                )
                                .and_where(Condition::equal("template_previews.template_id", &id))
                                .build(),
                            map_preview,
                        )
                        .await?;
                    Ok(Some(SlideClassificationJob {
                        attempt_count,
                        claim,
                        input_fingerprint,
                        kind,
                        preview,
                        title: document.presentation.title.clone(),
                        template: StoredTemplate {
                            revision,
                            template_id: id,
                            template_json: document,
                        },
                    }))
                })
                .await
        })
        .await
    }
    async fn claim_next_slide_embedding(
        &self,
        lease_seconds: u32,
    ) -> RepositoryResult<Option<SlideEmbeddingJob>> {
        repository_boundary(async {
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let Some((claim, attempt_count)) =
                        claim_next(tx, self, SlideProcessingStage::Embedding, lease_seconds)
                            .await?
                    else {
                        return Ok(None);
                    };
                    let state = load(tx, &claim.template_id)
                        .await?
                        .context("missing classification")?;
                    Ok(Some(SlideEmbeddingJob {
                        attempt_count,
                        subject_document: state
                            .subject_embedding_document
                            .context("missing subject embedding document")?,
                        subject_fingerprint: state
                            .subject_embedding_fingerprint
                            .context("missing subject embedding fingerprint")?,
                        capability_document: state
                            .capability_embedding_document
                            .context("missing capability embedding document")?,
                        capability_fingerprint: state
                            .capability_embedding_fingerprint
                            .context("missing capability embedding fingerprint")?,
                        template_id: claim.template_id.clone(),
                        claim,
                    }))
                })
                .await
        })
        .await
    }
}

impl PostgresTemplateRepository {
    pub(super) async fn insert_classification(
        &self,
        r: &TemplateInsert,
        p: &PendingSlideClassification,
        cancellation: Option<&super::super::RequestCancellation>,
    ) -> anyhow::Result<()> {
        self.client
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                check_publication(cancellation)?;
                self.write_classified_record(tx, r, p).await?;
                check_publication(cancellation)
            })
            .await
    }
    pub(super) async fn insert_classifications(
        &self,
        records: &[(TemplateInsert, PendingSlideClassification)],
        cancellation: Option<&super::super::RequestCancellation>,
    ) -> anyhow::Result<()> {
        self.client
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                check_publication(cancellation)?;
                for (record, pending) in records {
                    check_publication(cancellation)?;
                    self.write_classified_record(tx, record, pending).await?;
                }
                check_publication(cancellation)
            })
            .await
    }
    async fn write_classified_record(
        &self,
        tx: &PostgresTransaction,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> anyhow::Result<()> {
        self.write_new(
            tx,
            &record.template,
            &record.assets,
            &record.metadata,
            record.preview.as_ref(),
        )
        .await?;
        write_pending(tx, &record.template.template_id, pending).await
    }
}

async fn write_pending(
    tx: &PostgresTransaction,
    id: &str,
    pending: &PendingSlideClassification,
) -> anyhow::Result<()> {
    tx.write(
        SqlBuilder::insert_into(STATUS)
            .value("template_id", id)
            .value("status", "pending")
            .value("input_fingerprint", &pending.input_fingerprint)
            .value("prompt_version", &pending.prompt_version)
            .value("schema_version", pending.schema_version)
            .value("attempt_count", 0_i64)
            .build(),
    )
    .await?;
    Ok(())
}
