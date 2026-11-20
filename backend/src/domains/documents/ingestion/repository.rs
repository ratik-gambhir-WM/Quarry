//! Durable immutable input, bounded admission and database-clock attempt fencing.
use super::service::{ProcessedDocument, UploadedDocument};
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError, PostgresTransaction},
        query::{
            Condition as C, LockWait, RowLock, SelectExpression as E, SortDirection as D,
            SqlBuilder as Q, SqlQuery,
        },
    },
    domains::{
        documents::store::search_repository::EmbeddingProfile, memberships::model::Capability,
    },
    shared::{
        error::RepositoryError,
        ids::{document_id_from_content, sha256_hex},
    },
};

#[derive(Clone)]
pub(crate) struct DocumentJobRepository {
    pub db: PostgresClient,
    pub profile: EmbeddingProfile,
}
#[derive(Clone)]
pub(crate) struct Attempt {
    pub id: String,
    pub token: String,
    pub deal: String,
    pub actor: i64,
    pub filename: String,
    pub file_id: String,
    pub hash: String,
    pub profile_id: String,
}
pub(crate) struct Claimed {
    pub attempt: Attempt,
    pub file: UploadedDocument,
    pub image: Option<crate::domains::documents::formats::image::ImageAssembly>,
}
pub(crate) struct Job {
    pub deal: String,
    pub filename: String,
    pub status: String,
    pub sequence: i64,
    pub result: Option<ProcessedDocument>,
}
impl DocumentJobRepository {
    pub async fn prune(&self, seconds: u32) -> Result<(), RepositoryError> {
        self.db
            .transaction_result_async(|tx| async move {
                // Request-owned work has no recovery worker. Expire it under the same
                // mutation/row locks as claims and promotion, fencing any late owner.
                let abandoned = tx
                    .lock_with(
                        Q::select("document_jobs")
                            .columns(["job_id", "filename"])
                            .and_where(abandoned_sync())
                            .lock(RowLock::Update, LockWait::Wait)
                            .build(),
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    )
                    .await?;
                for (id, filename) in abandoned {
                    let result = serde_json::to_string(&failed_result(filename))
                        .map_err(PostgresError::decode)?;
                    tx.write(
                        Q::update("document_jobs")
                            .set("status", "failed")
                            .set("stage", "terminal")
                            .set("result_json", result)
                            .set("attempt_token", Option::<String>::None)
                            .set("lease_until", Option::<i64>::None)
                            .set_epoch_seconds_after("updated_at", 0)
                            .increment("sequence")
                            .and_where(C::equal("job_id", id))
                            .and_where(abandoned_sync())
                            .build(),
                    )
                    .await?;
                }
                tx.write(prune_jobs(seconds)).await?;
                Ok(())
            })
            .await
    }
    pub async fn admit(
        &self,
        deal: String,
        actor: i64,
        file: UploadedDocument,
        background: bool,
    ) -> Result<(String, String), RepositoryError> {
        let (file, hash) = tokio::task::spawn_blocking(move || {
            let hash = sha256_hex(&file.bytes);
            (file, hash)
        })
        .await
        .map_err(|_| RepositoryError::BlockingWorker("document hashing interrupted".into()))?;
        let profile = self.profile.id.clone();
        self.db
            .transaction_result_async(move |tx| async move {
                crate::domains::deals::coordination::lock_authority(&tx, &deal, RowLock::Share)
                    .await?;
                crate::domains::memberships::require_deal(&tx, &deal, actor, Capability::Upload)
                    .await?;
                // Retained global mutation serialization bounds queue capacity across processes.
                // No provider or canonical document work happens during admission.
                if let Some((id, status, accepted_filename)) = tx
                    .read_one_with(
                        Q::select("document_jobs")
                            .columns(["job_id", "status", "filename"])
                            .and_where(C::equal("deal_id", &deal))
                            .and_where(C::equal("actor_id", actor))
                            .and_where(C::equal("input_hash", &hash))
                            .build(),
                        |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, String>(2)?,
                            ))
                        },
                    )
                    .await?
                {
                    if status != "failed" {
                        if background && matches!(status.as_str(), "queued" | "processing") {
                            // Schedule recovery without replacing a live attempt's token or lease.
                            tx.write(
                                Q::update("document_jobs")
                                    .set("background", 1)
                                    .and_where(C::equal("job_id", &id))
                                    .build(),
                            )
                            .await?;
                        }
                        return Ok((id, accepted_filename));
                    }
                    let stats = tx
                        .read_one_with(admission_stats(actor), |r| {
                            Ok((r.get::<_, i64>("active")?, r.get::<_, i64>("actor_active")?))
                        })
                        .await?
                        .ok_or_else(|| RepositoryError::invalid("missing queue capacity"))?;
                    if stats.0 >= 64 || stats.1 >= 8 {
                        return Err(RepositoryError::Capacity);
                    }
                    tx.write(
                        Q::update("document_jobs")
                            .set("status", "queued")
                            .set("background", background)
                            .set("stage", "accepted")
                            .set("result_json", Option::<String>::None)
                            .increment("sequence")
                            .set_epoch_seconds_after("updated_at", 0)
                            .and_where(C::equal("job_id", &id))
                            .build(),
                    )
                    .await?;
                    return Ok((id, accepted_filename));
                }
                let (retained, active, actor_active, bytes) = tx
                    .read_one_with(admission_stats(actor), |r| {
                        Ok((
                            r.get::<_, i64>("retained")?,
                            r.get::<_, i64>("active")?,
                            r.get::<_, i64>("actor_active")?,
                            r.get::<_, i64>("bytes")?,
                        ))
                    })
                    .await?
                    .ok_or_else(|| RepositoryError::invalid("missing queue capacity"))?;
                let input_bytes = i64::try_from(file.bytes.len()).map_err(PostgresError::decode)?;
                if active >= 64
                    || actor_active >= 8
                    || retained >= 4096
                    || bytes
                        .checked_add(input_bytes)
                        .is_none_or(|n| n > 1024 * 1024 * 1024)
                {
                    return Err(RepositoryError::Capacity);
                }
                let id = uuid::Uuid::new_v4().to_string();
                // The deal-scoped content address is the logical identity; the unique key records it
                // before the first file row exists, and identical bytes always converge on it.
                tx.write(
                    Q::insert_into("document_input_identities")
                        .value("deal_id", &deal)
                        .value("input_hash", &hash)
                        .value("file_id", document_id_from_content(&deal, &hash))
                        .on_conflict_do_nothing(["deal_id", "input_hash"])
                        .build(),
                )
                .await?;
                let file_id = tx
                    .read_one_with(
                        Q::select("document_input_identities")
                            .columns(["file_id"])
                            .and_where(C::equal("deal_id", &deal))
                            .and_where(C::equal("input_hash", &hash))
                            .build(),
                        |r| r.get::<_, String>(0),
                    )
                    .await?
                    .ok_or_else(|| RepositoryError::invalid("document claim missing"))?;
                tx.write(
                    Q::insert_into("document_jobs")
                        .value("job_id", &id)
                        .value("deal_id", deal)
                        .value("actor_id", actor)
                        .value("input_hash", hash)
                        .value("filename", &file.filename)
                        .value("input_bytes", file.bytes)
                        .value("file_id", file_id)
                        .value("profile_id", profile)
                        .value("status", "queued")
                        .value("background", background)
                        .build(),
                )
                .await?;
                Ok((id, file.filename))
            })
            .await
    }
    pub async fn claim(&self, id: Option<&str>) -> Result<Option<Claimed>, RepositoryError> {
        self.db
            .transaction_result_async(|tx| async move {
                let mut jobs = tx
                    .lock_with(due_job(id), |r| {
                        Ok((
                            r.get::<_, String>("job_id")?,
                            r.get::<_, String>("deal_id")?,
                            r.get::<_, i64>("actor_id")?,
                            r.get::<_, String>("filename")?,
                            r.get::<_, String>("file_id")?,
                            r.get::<_, String>("input_hash")?,
                            r.get::<_, String>("profile_id")?,
                            r.get::<_, Vec<u8>>("input_bytes")?,
                            r.get::<_, Option<String>>("image_assembly_json")?,
                        ))
                    })
                    .await?;
                let Some((id, deal, actor, filename, file_id, hash, profile_id, bytes, image)) =
                    jobs.pop()
                else {
                    return Ok(None);
                };
                let token = uuid::Uuid::new_v4().to_string();
                tx.write(
                    Q::update("document_jobs")
                        .set("status", "processing")
                        .set("stage", "preparing")
                        .set("attempt_token", &token)
                        .set_epoch_seconds_after("lease_until", 600)
                        .set_epoch_seconds_after("updated_at", 0)
                        .increment("sequence")
                        .and_where(C::equal("job_id", &id))
                        .build(),
                )
                .await?;
                Ok(Some(Claimed {
                    attempt: Attempt {
                        id,
                        token,
                        deal,
                        actor,
                        filename: filename.clone(),
                        file_id,
                        hash,
                        profile_id,
                    },
                    file: UploadedDocument { filename, bytes },
                    image: image
                        .map(|s| {
                            serde_json::from_str(&s)
                                .map_err(|_| RepositoryError::invalid("invalid image checkpoint"))
                        })
                        .transpose()?,
                }))
            })
            .await
    }
    pub async fn job(&self, id: &str) -> Result<Job, RepositoryError> {
        self.db
            .read_one_with_async(
                Q::select("document_jobs")
                    .columns(["deal_id", "filename", "status", "sequence", "result_json"])
                    .and_where(C::equal("job_id", id))
                    .build(),
                |r| {
                    Ok((
                        r.get::<_, String>("deal_id")?,
                        r.get::<_, String>("filename")?,
                        r.get::<_, String>("status")?,
                        r.get::<_, i64>("sequence")?,
                        r.get::<_, Option<String>>("result_json")?,
                    ))
                },
            )
            .await?
            .ok_or_else(|| RepositoryError::NotFound("document job not found".into()))
            .and_then(|(deal, filename, status, sequence, result)| {
                Ok(Job {
                    deal,
                    filename,
                    status,
                    sequence,
                    result: result
                        .map(|s| {
                            serde_json::from_str(&s).map_err(|_| {
                                RepositoryError::invalid("invalid document job result")
                            })
                        })
                        .transpose()?,
                })
            })
    }
    pub async fn checkpoint_image(
        &self,
        attempt: &Attempt,
        json: String,
    ) -> Result<(), RepositoryError> {
        let attempt = attempt.clone();
        self.db
            .transaction_result_async(move |tx| async move {
                crate::domains::deals::coordination::lock_authority(
                    &tx,
                    &attempt.deal,
                    RowLock::Share,
                )
                .await?;
                crate::domains::memberships::require_deal(
                    &tx,
                    &attempt.deal,
                    attempt.actor,
                    Capability::Upload,
                )
                .await?;
                attempt.lock_and_check(&tx).await?;
                let existing = tx
                    .read_one_with(
                        Q::select("document_jobs")
                            .columns(["image_assembly_json"])
                            .and_where(C::equal("job_id", &attempt.id))
                            .build(),
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .await?
                    .flatten();
                if existing.as_ref().is_some_and(|saved| saved != &json) {
                    return Err(RepositoryError::Conflict(
                        "image checkpoint is immutable".into(),
                    ));
                }
                attempt.update("image_assembly_json", json).apply(&tx).await
            })
            .await
    }
    pub async fn fail(&self, attempt: &Attempt) -> Result<(), RepositoryError> {
        let result = failed_result(attempt.filename.clone());
        let query = terminal(attempt, "failed", &result)?;
        self.db.write_async(Ok(query)).await?;
        Ok(())
    }
}
fn failed_result(filename: String) -> ProcessedDocument {
    ProcessedDocument {
        filename,
        document_id: None,
        chunk_count: 0,
        success: false,
        sqlite_committed: false,
        skipped: false,
        error: Some(
            "Document processing failed. Refresh documents and retry the same file.".into(),
        ),
    }
}
fn abandoned_sync() -> C {
    C::all([
        C::equal("background", 0),
        C::any([
            C::all([C::equal("status", "processing"), C::expired("lease_until")]),
            C::all([
                C::equal("status", "queued"),
                C::older_than("updated_at", 600),
            ]),
        ]),
    ])
}
impl Attempt {
    pub async fn lock_and_check(&self, tx: &PostgresTransaction) -> Result<(), RepositoryError> {
        let rows = tx
            .lock_with(
                Q::select("document_jobs")
                    .columns(["job_id"])
                    .and_where(C::equal("job_id", &self.id))
                    .and_where(C::equal("attempt_token", &self.token))
                    .and_where(C::equal("status", "processing"))
                    .and_where(C::equal("input_hash", &self.hash))
                    .and_where(C::equal("file_id", &self.file_id))
                    .and_where(C::equal("profile_id", &self.profile_id))
                    .and_where(C::unexpired("lease_until"))
                    .lock(RowLock::Update, LockWait::Wait)
                    .build(),
                |_| Ok(()),
            )
            .await?;
        if rows.len() != 1 {
            return Err(RepositoryError::Conflict(
                "document attempt expired or superseded".into(),
            ));
        }
        Ok(())
    }
    pub async fn complete(
        &self,
        tx: &PostgresTransaction,
        document: &str,
        count: usize,
        skipped: bool,
    ) -> Result<(), RepositoryError> {
        let result = ProcessedDocument {
            filename: self.filename.clone(),
            document_id: Some(document.into()),
            chunk_count: count,
            success: true,
            sqlite_committed: true,
            skipped,
            error: None,
        };
        if tx
            .write(terminal(
                self,
                if skipped { "skipped" } else { "completed" },
                &result,
            ))
            .await?
            .rows_affected
            != 1
        {
            return Err(RepositoryError::Conflict(
                "document attempt expired before commit".into(),
            ));
        }
        Ok(())
    }
    fn update(&self, column: &'static str, json: String) -> Checkpoint {
        Checkpoint {
            attempt: self.clone(),
            column,
            json,
        }
    }
}
struct Checkpoint {
    attempt: Attempt,
    column: &'static str,
    json: String,
}
impl Checkpoint {
    async fn apply(self, tx: &PostgresTransaction) -> Result<(), RepositoryError> {
        let changed = tx
            .write(
                Q::update("document_jobs")
                    .set(self.column, self.json)
                    .increment("sequence")
                    .set_epoch_seconds_after("updated_at", 0)
                    .and_where(C::equal("job_id", self.attempt.id))
                    .and_where(C::equal("attempt_token", self.attempt.token))
                    .and_where(C::unexpired("lease_until"))
                    .build(),
            )
            .await?
            .rows_affected;
        if changed != 1 {
            return Err(RepositoryError::Conflict("document attempt expired".into()));
        }
        Ok(())
    }
}
fn terminal(
    attempt: &Attempt,
    status: &str,
    result: &ProcessedDocument,
) -> Result<crate::adapters::postgres::query::SqlQuery, PostgresError> {
    Q::update("document_jobs")
        .set("status", status)
        .set("stage", "terminal")
        .set(
            "result_json",
            serde_json::to_string(result).map_err(PostgresError::decode)?,
        )
        .set("attempt_token", Option::<String>::None)
        .set("lease_until", Option::<i64>::None)
        .set_epoch_seconds_after("updated_at", 0)
        .increment("sequence")
        .and_where(C::equal("job_id", &attempt.id))
        .and_where(C::equal("attempt_token", &attempt.token))
        .and_where(C::unexpired("lease_until"))
        .build()
}

pub(crate) fn due_job(id: Option<&str>) -> Result<SqlQuery, PostgresError> {
    let scope = match id {
        Some(id) => C::equal("job_id", id),
        None => C::equal("background", 1),
    };
    Q::select("document_jobs")
        .and_where(scope)
        .and_where(C::any([
            C::equal("status", "queued"),
            C::all([C::equal("status", "processing"), C::expired("lease_until")]),
        ]))
        .order_by("created_at", D::Ascending)
        .order_by("job_id", D::Ascending)
        .limit(1)
        .lock(RowLock::Update, LockWait::SkipLocked)
        .build()
}
pub(crate) fn admission_stats(actor: i64) -> Result<SqlQuery, PostgresError> {
    Q::select("document_jobs")
        .expression(E::Count, "retained")
        .expression(
            E::CountWhere(C::in_values("status", ["queued", "processing"])),
            "active",
        )
        .expression(
            E::CountWhere(C::all([
                C::in_values("status", ["queued", "processing"]),
                C::equal("actor_id", actor),
            ])),
            "actor_active",
        )
        .expression(E::SumByteLength("input_bytes".into()), "bytes")
        .build()
}
fn prune_jobs(retention: u32) -> Result<SqlQuery, PostgresError> {
    Q::delete_from("document_jobs")
        .and_where(C::any([
            C::all([
                C::in_values("status", ["completed", "skipped"]),
                C::older_than("updated_at", retention),
            ]),
            C::all([
                C::equal("status", "failed"),
                C::older_than("updated_at", 86400),
            ]),
        ]))
        .build()
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/ingestion/repository_tests.rs"]
mod tests;
