//! Classification persistence. Rows are written only inside the canonical document
//! transaction or by reclassifying a committed document; there are no staged rows,
//! claims or leases. No provider work ever runs inside these transactions.
use std::collections::HashMap;

use super::{
    model::{ErrorCode, RunStatus, MODEL, POLICY_VERSION, PROMPT_VERSION, SAMPLE_VERSION},
    sample::EvidenceSample,
    taxonomy::TAXONOMY_VERSION,
};
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError, PostgresTransaction, Row},
        query::{Condition as C, LockWait, RowLock, SortDirection as D, SqlBuilder as Q},
    },
    domains::{
        deals::coordination::lock_authority,
        memberships::{model::Capability, require_deal},
    },
    shared::error::RepositoryError,
};

const TABLE: &str = "document_classifications";

/// One classification outcome with the evidence and pinned profile that produced it.
#[derive(Clone, Debug)]
pub(crate) struct ClassificationRecord {
    pub sample: EvidenceSample,
    pub fingerprint: String,
    pub status: RunStatus,
    pub result_json: Option<String>,
    pub error: Option<ErrorCode>,
}

#[derive(Clone, Debug)]
pub(crate) struct CommittedVersion {
    pub deal: String,
    pub content_sha256: String,
    pub version_id: String,
    pub original_filename: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredClassification {
    pub generation: i64,
    pub sample: EvidenceSample,
    pub sample_version: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ReclassifyTarget {
    pub version: CommittedVersion,
    pub stored: Option<StoredClassification>,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredChunk {
    pub index: i64,
    pub token_count: u64,
    pub sha256: String,
    pub section: Option<String>,
    pub pages: Option<(i64, i64)>,
}

/// Write the classification inside the canonical document transaction. A replayed commit
/// of the same content keeps the existing row.
pub(crate) async fn insert_with_document(
    tx: &PostgresTransaction,
    deal: &str,
    hash: &str,
    actor: i64,
    record: &ClassificationRecord,
) -> Result<(), RepositoryError> {
    let inserted = tx
        .write(
            Q::insert_into(TABLE)
                .value("deal_id", deal)
                .value("input_hash", hash)
                .value("requested_by_user_id", actor)
                .value("generation", 1)
                .value("request_fingerprint", &record.fingerprint)
                .value("sample_text", &record.sample.text)
                .value("sample_provenance", &record.sample.provenance)
                .value("sample_hash", &record.sample.hash)
                .value("model", MODEL)
                .value("taxonomy_version", TAXONOMY_VERSION)
                .value("prompt_version", PROMPT_VERSION)
                .value("sample_version", SAMPLE_VERSION)
                .value("policy_version", POLICY_VERSION)
                .value("run_status", record.status.as_str())
                .value("last_error_code", record.error.map(ErrorCode::as_str))
                .value("result_json", record.result_json.clone())
                .on_conflict_do_nothing(["deal_id", "input_hash"])
                .build(),
        )
        .await?
        .rows_affected
        == 1;
    if inserted && record.status == RunStatus::Completed {
        stamp_completion(tx, deal, hash, 1).await?;
    }
    Ok(())
}

/// Database-clock completion time for exactly this generation.
async fn stamp_completion(
    tx: &PostgresTransaction,
    deal: &str,
    hash: &str,
    generation: i64,
) -> Result<(), RepositoryError> {
    tx.write(
        Q::update(TABLE)
            .set_epoch_seconds_after("completed_at", 0)
            .and_where(C::equal("deal_id", deal))
            .and_where(C::equal("input_hash", hash))
            .and_where(C::equal("generation", generation))
            .build(),
    )
    .await?;
    Ok(())
}

#[derive(Clone)]
pub(crate) struct ClassificationRepository {
    db: PostgresClient,
}

impl ClassificationRepository {
    pub(crate) fn new(db: PostgresClient) -> Self {
        Self { db }
    }

    /// Current committed version of a caller-visible file plus its stored classification.
    pub(crate) async fn reclassify_target(
        &self,
        deal: &str,
        file_id: &str,
        actor: i64,
    ) -> Result<ReclassifyTarget, RepositoryError> {
        let (deal, file_id) = (deal.to_string(), file_id.to_string());
        self.db
            .read_transaction_result_async(move |tx| async move {
                require_deal(&tx, &deal, actor, Capability::Upload).await?;
                let version = current_version(&tx, &deal, &file_id)
                    .await?
                    .ok_or_else(|| RepositoryError::NotFound("document not found".into()))?;
                let stored = tx
                    .read_one_with(
                        Q::select(TABLE)
                            .columns([
                                "generation",
                                "sample_text",
                                "sample_provenance",
                                "sample_hash",
                                "sample_version",
                            ])
                            .and_where(C::equal("deal_id", &deal))
                            .and_where(C::equal("input_hash", &version.content_sha256))
                            .build(),
                        map_stored,
                    )
                    .await?;
                Ok(ReclassifyTarget { version, stored })
            })
            .await
    }

    /// Generation compare-and-set under the authority and row locks. Returns the new
    /// generation and its database completion time.
    pub(crate) async fn apply_reclassification(
        &self,
        target: &ReclassifyTarget,
        actor: i64,
        expected: i64,
        record: ClassificationRecord,
    ) -> Result<(i64, Option<i64>), RepositoryError> {
        let version = target.version.clone();
        self.db
            .transaction_result_async(move |tx| async move {
                lock_authority(&tx, &version.deal, RowLock::Share).await?;
                require_deal(&tx, &version.deal, actor, Capability::Upload).await?;
                current_version(&tx, &version.deal, &version.version_id)
                    .await?
                    .filter(|current| current.content_sha256 == version.content_sha256)
                    .ok_or_else(|| RepositoryError::NotFound("document not found".into()))?;
                let generation = tx
                    .lock_with(
                        Q::select(TABLE)
                            .columns(["generation"])
                            .and_where(C::equal("deal_id", &version.deal))
                            .and_where(C::equal("input_hash", &version.content_sha256))
                            .lock(RowLock::Update, LockWait::Wait)
                            .build(),
                        |r| r.get::<_, i64>(0),
                    )
                    .await?
                    .pop();
                let next = match generation {
                    None if expected == 0 => {
                        insert_with_document(
                            &tx,
                            &version.deal,
                            &version.content_sha256,
                            actor,
                            &record,
                        )
                        .await?;
                        1
                    }
                    Some(current) if current == expected => {
                        let next = current + 1;
                        let completed = record.status == RunStatus::Completed;
                        tx.write(
                            Q::update(TABLE)
                                .set("generation", next)
                                .set("requested_by_user_id", actor)
                                .set("request_fingerprint", record.fingerprint)
                                .set("sample_text", record.sample.text)
                                .set("sample_provenance", record.sample.provenance)
                                .set("sample_hash", record.sample.hash)
                                .set("model", MODEL)
                                .set("taxonomy_version", TAXONOMY_VERSION)
                                .set("prompt_version", PROMPT_VERSION)
                                .set("sample_version", SAMPLE_VERSION)
                                .set("policy_version", POLICY_VERSION)
                                .set("run_status", record.status.as_str())
                                .set("last_error_code", record.error.map(ErrorCode::as_str))
                                .set("result_json", record.result_json)
                                .set("completed_at", Option::<i64>::None)
                                .set_epoch_seconds_after("updated_at", 0)
                                .and_where(C::equal("deal_id", &version.deal))
                                .and_where(C::equal("input_hash", &version.content_sha256))
                                .and_where(C::equal("generation", current))
                                .build(),
                        )
                        .await?;
                        if completed {
                            stamp_completion(&tx, &version.deal, &version.content_sha256, next)
                                .await?;
                        }
                        next
                    }
                    _ => {
                        return Err(RepositoryError::Conflict(
                            "classification generation changed".into(),
                        ))
                    }
                };
                let completed_at = tx
                    .read_one_with(
                        Q::select(TABLE)
                            .columns(["completed_at"])
                            .and_where(C::equal("deal_id", &version.deal))
                            .and_where(C::equal("input_hash", &version.content_sha256))
                            .build(),
                        |r| r.get::<_, Option<i64>>(0),
                    )
                    .await?
                    .flatten();
                Ok((next, completed_at))
            })
            .await
    }

    /// Ordered chunk metadata for one committed version; no text or vectors.
    pub(crate) async fn chunk_metadata(
        &self,
        version_id: &str,
    ) -> Result<Vec<StoredChunk>, RepositoryError> {
        let query = Q::select("document_chunk_identities")
            .columns([
                "document_chunk_identities.chunk_index",
                "document_chunk_identities.chunk_sha256",
                "document_chunks.token_count",
                "document_chunks.section_path",
                "document_chunks.page_start",
                "document_chunks.page_end",
            ])
            .inner_join(
                "document_chunks",
                "document_chunk_identities.chunk_id",
                "document_chunks.chunk_id",
            )
            .and_where(C::equal("document_chunk_identities.version_id", version_id))
            .order_by("document_chunk_identities.chunk_index", D::Ascending)
            .build();
        Ok(self
            .db
            .read_with_async(query, |r| {
                let section: String = r.get(3)?;
                let pages = match (r.get::<_, Option<i64>>(4)?, r.get::<_, Option<i64>>(5)?) {
                    (Some(first), Some(last)) => Some((first, last)),
                    _ => None,
                };
                Ok(StoredChunk {
                    index: r.get(0)?,
                    sha256: r.get(1)?,
                    token_count: u64::try_from(r.get::<_, i64>(2)?).unwrap_or(0),
                    section: (!section.is_empty()).then_some(section),
                    pages,
                })
            })
            .await?)
    }

    /// Only the selected chunk texts, never embedding columns.
    pub(crate) async fn chunk_texts(
        &self,
        version_id: &str,
        indices: &[i64],
    ) -> Result<HashMap<i64, String>, RepositoryError> {
        if indices.is_empty() {
            return Ok(HashMap::new());
        }
        let query = Q::select("document_chunk_identities")
            .columns([
                "document_chunk_identities.chunk_index",
                "document_chunks.text",
            ])
            .inner_join(
                "document_chunks",
                "document_chunk_identities.chunk_id",
                "document_chunks.chunk_id",
            )
            .and_where(C::equal("document_chunk_identities.version_id", version_id))
            .and_where(C::in_values(
                "document_chunk_identities.chunk_index",
                indices.iter().copied(),
            ))
            .build();
        Ok(self
            .db
            .read_with_async(query, |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .await?
            .into_iter()
            .collect())
    }
}

fn map_stored(r: &Row<'_>) -> Result<StoredClassification, PostgresError> {
    let provenance: String = r.get(2)?;
    let sampled = serde_json::from_str::<serde_json::Value>(&provenance)
        .ok()
        .and_then(|value| value.get("sampled").and_then(serde_json::Value::as_bool))
        .unwrap_or(true);
    Ok(StoredClassification {
        generation: r.get(0)?,
        sample: EvidenceSample {
            text: r.get(1)?,
            provenance,
            hash: r.get(3)?,
            sampled,
        },
        sample_version: r.get(4)?,
    })
}

async fn current_version(
    tx: &PostgresTransaction,
    deal: &str,
    file_id: &str,
) -> Result<Option<CommittedVersion>, RepositoryError> {
    Ok(tx
        .read_one_with(
            Q::select("quarry_files")
                .columns([
                    "quarry_files.deal_id",
                    "quarry_file_versions.content_sha256",
                    "quarry_file_versions.version_id",
                    "quarry_file_versions.original_filename",
                ])
                .inner_join(
                    "quarry_file_versions",
                    "quarry_files.file_id",
                    "quarry_file_versions.file_id",
                )
                .inner_join(
                    "document_input_identities",
                    "quarry_files.file_id",
                    "document_input_identities.file_id",
                )
                .and_where(C::equal("quarry_files.deal_id", deal))
                .and_where(C::equal("quarry_files.file_id", file_id))
                .and_where(C::is_null("quarry_files.deleted_at"))
                .and_where(C::equal("quarry_file_versions.is_current", 1))
                .build(),
            |r| {
                Ok(CommittedVersion {
                    deal: r.get(0)?,
                    content_sha256: r.get(1)?,
                    version_id: r.get(2)?,
                    original_filename: r.get(3)?,
                })
            },
        )
        .await?)
}

#[cfg(test)]
#[path = "../../../../tests/integration/domains/documents/classification/repository_tests.rs"]
mod tests;
