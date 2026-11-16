//! Durable admission, leases and checkpoints. Typed errors roll back every mutation.
use super::search_repository::{source_hash, PreparedNote};
use super::{
    model::*,
    repository::{conflict, folder, missing, note, owned, NotebookRepository, Result},
};
use crate::adapters::postgres::client::Row;
use crate::adapters::postgres::client::RowResult;
use crate::adapters::postgres::query::{LockWait, RowLock};
use crate::{
    adapters::postgres::{
        client::PostgresTransaction,
        query::{ComparisonOperator as Op, Condition as C, SortDirection as Sort, SqlBuilder as Q},
    },
    shared::error::RepositoryError as Error,
};
use sha2::{Digest, Sha256};

const ACTIVE: [&str; 2] = ["queued", "processing"];
fn job_row(r: &Row<'_>) -> RowResult<SaveJob> {
    let notebook_id: String = r.get("notebook_id")?;
    let job_id: String = r.get("job_id")?;
    let status_url = format!("/api/v1/notebooks/{notebook_id}/save-jobs/{job_id}");
    let status: String = r.get("status")?;
    let attempt: i64 = r.get("attempt")?;
    let error_code: Option<String> = r.get("error_code")?;
    let retryable = status == "failed"
        && attempt < 5
        && matches!(
            error_code.as_deref(),
            Some(
                "indexing_failed"
                    | "embedding_failed"
                    | "embedding_unavailable"
                    | "pipeline_changed"
                    | "worker_interrupted"
            )
        );
    let committed_revision: Option<i64> = r.get("committed_revision")?;
    Ok(SaveJob {
        job_id,
        notebook_id,
        note_id: r.get("note_id")?,
        status,
        stage: r.get("stage")?,
        attempt,
        sequence: r.get("sequence")?,
        sqlite_committed: committed_revision.is_some(),
        committed_revision,
        error_code,
        retryable,
        events_url: format!("{status_url}/events"),
        status_url,
    })
}
async fn load(tx: &PostgresTransaction, id: &str) -> Result<SaveJob> {
    tx.read_one_with(
        Q::select("notebook_save_jobs")
            .and_where(C::equal("job_id", id))
            .build(),
        job_row,
    )
    .await?
    .ok_or_else(missing)
}
async fn fence(tx: &PostgresTransaction, claim: &ClaimedJob) -> Result<SaveJob> {
    let job = tx
        .read_one_with(
            Q::select("notebook_save_jobs")
                .and_where(C::equal("job_id", &claim.job.job_id))
                .and_where(C::equal("lease_token", &claim.token))
                .and_where(C::equal("status", "processing"))
                .and_where(C::unexpired("lease_until"))
                .build(),
            job_row,
        )
        .await?
        .ok_or_else(conflict)?;
    owned(tx, claim.owner, &job.notebook_id).await?;
    Ok(job)
}
impl NotebookRepository {
    pub(super) async fn admit_with_pipeline(
        &self,
        owner: i64,
        notebook: String,
        note_id: Option<String>,
        input: SaveInput,
        pipeline_version: String,
    ) -> Result<SaveJob> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                admission_user(tx, owner).await?;
                super::repository::lock_parent(
                    tx,
                    owner,
                    &notebook,
                    crate::adapters::postgres::query::RowLock::Share,
                )
                .await?;
                let bytes = serde_json::to_vec(&(&notebook, &note_id, &input))
                    .map_err(|_| Error::storage("invalid save input"))?;
                let hash = format!("{:x}", Sha256::digest(bytes));
                if let Some((existing, prior_hash, prior_parent)) = tx
                    .read_one_with(
                        Q::select("notebook_save_jobs")
                            .and_where(C::equal("user_id", owner))
                            .and_where(C::equal("operation_id", &input.operation_id))
                            .build(),
                        |r| {
                            Ok((
                                job_row(r)?,
                                r.get::<_, String>("request_hash")?,
                                r.get::<_, String>("notebook_id")?,
                            ))
                        },
                    )
                    .await?
                {
                    if prior_hash != hash || prior_parent != notebook {
                        return Err(Error::Conflict(
                            "operationId already used with different input".into(),
                        ));
                    }
                    return Ok(existing);
                }
                let time = now();
                let mut snapshot = if let Some(id) = &note_id {
                    note(tx, &notebook, id).await?
                } else {
                    Note {
                        summary: NoteSummary {
                            note_id: uuid::Uuid::new_v4().to_string(),
                            notebook_id: notebook.clone(),
                            folder_id: None,
                            title: String::new(),
                            content_schema_version: 1,
                            revision: 1,
                            created_at: time.clone(),
                            updated_at: time.clone(),
                        },
                        blocks: serde_json::json!([]),
                    }
                };
                let target = input.folder_id.apply(snapshot.summary.folder_id);
                if let Some(id) = &target {
                    folder(tx, &notebook, id).await?;
                }
                let expected = if let PatchField::Value(value) = input.expected_revision {
                    Some(value)
                } else {
                    None
                };
                if let Some(expected) = expected {
                    if expected == MAX_REVISION {
                        return Err(conflict());
                    }
                    snapshot.summary.revision = expected.checked_add(1).ok_or_else(conflict)?;
                }
                snapshot.summary.folder_id = target;
                if let PatchField::Value(title) = input.title {
                    snapshot.summary.title = title;
                }
                if let PatchField::Value(blocks) = input.blocks {
                    snapshot.blocks = blocks;
                }
                snapshot.summary.updated_at = time.clone();
                let active_note = tx
                    .read_one(
                        Q::select("notebook_save_jobs")
                            .columns(["job_id"])
                            .and_where(C::equal("note_id", &snapshot.summary.note_id))
                            .and_where(C::in_values("status", ACTIVE))
                            .build(),
                    )
                    .await?;
                if active_note.is_some() {
                    return Err(Error::Conflict("note save already active".into()));
                }
                let slot = free_slot(tx, owner).await?;
                let id = uuid::Uuid::new_v4().to_string();
                let note_id = snapshot.summary.note_id.clone();
                let accepted = serde_json::to_string(&AcceptedSave {
                    pipeline_version: pipeline_version.clone(),
                    note: snapshot,
                    expected_revision: expected,
                })
                .map_err(|_| Error::storage("invalid save input"))?;
                tx.write(
                    Q::insert_into("notebook_save_jobs")
                        .value("job_id", &id)
                        .value("user_id", owner)
                        .value("notebook_id", notebook)
                        .value("note_id", note_id)
                        .value("operation_id", input.operation_id)
                        .value("request_hash", hash)
                        .value("pipeline_version", pipeline_version)
                        .value("input_json", accepted)
                        .value("status", "queued")
                        .value("stage", "exporting")
                        .value("created_at", &time)
                        .value("updated_at", time)
                        .build(),
                )
                .await?;
                allocate_slot(tx, slot, &id).await?;
                load(tx, &id).await
            })
            .await
    }
    pub(super) async fn job(&self, owner: i64, notebook: String, id: String) -> Result<SaveJob> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                owned(tx, owner, &notebook).await?;
                let job = load(tx, &id).await?;
                if job.notebook_id != notebook {
                    return Err(missing());
                }
                Ok(job)
            })
            .await
    }
    pub(super) async fn retry(&self, owner: i64, notebook: String, id: String) -> Result<SaveJob> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                admission_user(tx, owner).await?;
                super::repository::lock_parent(
                    tx,
                    owner,
                    &notebook,
                    crate::adapters::postgres::query::RowLock::Share,
                )
                .await?;
                let job = load(tx, &id).await?;
                if job.notebook_id != notebook {
                    return Err(missing());
                }
                if !job.retryable {
                    return Err(Error::Conflict("save job is not retryable".into()));
                }
                let previous = tx
                    .read_one_with(
                        Q::select("notebook_save_jobs")
                            .columns(["updated_at"])
                            .and_where(C::equal("job_id", &id))
                            .build(),
                        |r| r.get::<_, String>(0),
                    )
                    .await?
                    .ok_or_else(missing)?;
                let previous = chrono::DateTime::parse_from_rfc3339(&previous)
                    .map_err(|_| Error::storage("invalid job timestamp"))?;
                if chrono::Utc::now()
                    .signed_duration_since(previous)
                    .num_seconds()
                    < (1_i64 << (job.attempt - 1))
                {
                    return Err(Error::Conflict("retry backoff; retry later".into()));
                }
                if let Some(revision) = job.committed_revision {
                    if note(tx, &notebook, &job.note_id).await?.summary.revision != revision {
                        return Err(conflict());
                    }
                }
                if tx
                    .read_one(
                        Q::select("notebook_save_jobs")
                            .columns(["job_id"])
                            .and_where(C::equal("note_id", &job.note_id))
                            .and_where(C::in_values("status", ACTIVE))
                            .build(),
                    )
                    .await?
                    .is_some()
                {
                    return Err(Error::Conflict("note save already active".into()));
                }
                let slot = free_slot(tx, owner).await?;
                tx.write(
                    Q::update("notebook_save_jobs")
                        .set("status", "queued")
                        .set("error_code", None::<String>)
                        .set("lease_token", None::<String>)
                        .set("lease_until", None::<i64>)
                        .set("updated_at", now())
                        .increment("attempt")
                        .increment("sequence")
                        .and_where(C::equal("job_id", &id))
                        .build(),
                )
                .await?;
                allocate_slot(tx, slot, &id).await?;
                load(tx, &id).await
            })
            .await
    }
    pub(super) async fn claim(&self) -> Result<Option<ClaimedJob>> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                let base = || {
                    Q::select("notebook_save_jobs")
                        .order_by("created_at", Sort::Ascending)
                        .order_by("job_id", Sort::Ascending)
                        .limit(1)
                        .lock(RowLock::Update, LockWait::SkipLocked)
                };
                let mut candidate = tx
                    .lock_with(
                        base().and_where(C::equal("status", "queued")).build(),
                        |r| {
                            Ok((
                                job_row(r)?,
                                r.get::<_, i64>("user_id")?,
                                r.get::<_, String>("input_json")?,
                            ))
                        },
                    )
                    .await?
                    .pop();
                if candidate.is_none() {
                    candidate = tx
                        .lock_with(
                            base()
                                .and_where(C::equal("status", "processing"))
                                .and_where(C::expired("lease_until"))
                                .build(),
                            |r| {
                                Ok((
                                    job_row(r)?,
                                    r.get::<_, i64>("user_id")?,
                                    r.get::<_, String>("input_json")?,
                                ))
                            },
                        )
                        .await?
                        .pop();
                }
                let Some((job, owner, input)) = candidate else {
                    return Ok(None);
                };
                let parsed = serde_json::from_str::<AcceptedSave>(&input)
                    .ok()
                    .filter(|input| {
                        input.note.summary.note_id == job.note_id
                            && input.note.summary.notebook_id == job.notebook_id
                            && super::content::validate(&input.note.blocks).is_ok()
                    });
                let Some(input) = parsed else {
                    tx.write(
                        Q::update("notebook_save_jobs")
                            .set("status", "failed")
                            .set("error_code", "invalid_checkpoint")
                            .set("updated_at", now())
                            .increment("sequence")
                            .and_where(C::equal("job_id", &job.job_id))
                            .build(),
                    )
                    .await?;
                    release_slot(tx, &job.job_id).await?;
                    return Ok(None);
                };
                let token = uuid::Uuid::new_v4().to_string();
                // One attempt has a 120-second deadline. Lease is longer so a live attempt cannot expire.
                tx.write(
                    Q::update("notebook_save_jobs")
                        .set("status", "processing")
                        .set("lease_token", &token)
                        .set_epoch_seconds_after("lease_until", 180)
                        .set("stage", "exporting")
                        .set("updated_at", now())
                        .increment("sequence")
                        .and_where(C::equal("job_id", &job.job_id))
                        .build(),
                )
                .await?;
                Ok(Some(ClaimedJob {
                    job: load(tx, &job.job_id).await?,
                    owner,
                    token,
                    input,
                }))
            })
            .await
    }
    pub(super) async fn persist(
        &self,
        claim: &ClaimedJob,
        prepared: Option<PreparedNote>,
        reuse: Option<String>,
    ) -> Result<()> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                let n = &claim.input.note;
                let s = &n.summary;
                super::repository::lock_parent(tx, claim.owner, &s.notebook_id, RowLock::Share)
                    .await?;
                if let Some(id) = &s.folder_id {
                    super::repository::lock_folder(tx, &s.notebook_id, id, RowLock::Share).await?;
                }
                let job = fence(tx, claim).await?;
                let hash = source_hash(n, &claim.input.pipeline_version)?;
                let set = if let Some(set) = reuse {
                    let valid = tx
                        .read_one(
                            Q::select("notebook_search_sets")
                                .inner_join(
                                    "notebook_notes",
                                    "notebook_notes.active_set_id",
                                    "notebook_search_sets.set_id",
                                )
                                .and_where(C::equal("notebook_search_sets.set_id", &set))
                                .and_where(C::equal("notebook_notes.note_id", &s.note_id))
                                .and_where(C::equal("source_hash", &hash))
                                .build(),
                        )
                        .await?
                        .is_some();
                    if !valid || prepared.is_some() {
                        return Err(conflict());
                    }
                    set
                } else {
                    let batch = prepared
                        .as_ref()
                        .ok_or_else(|| Error::invalid("missing notebook artifact"))?;
                    if batch.source_hash != hash || batch.pipeline != claim.input.pipeline_version {
                        return Err(conflict());
                    }
                    uuid::Uuid::new_v4().to_string()
                };
                let blocks = serde_json::to_string(&n.blocks)
                    .map_err(|_| Error::storage("invalid durable blocks"))?;
                if let Some(expected) = claim.input.expected_revision {
                    if tx
                        .write(
                            Q::update("notebook_notes")
                                .set("title", &s.title)
                                .set("folder_id", s.folder_id.as_deref())
                                .set("blocks_json", blocks)
                                .set("revision", s.revision)
                                .set("updated_at", &s.updated_at)
                                .set("active_set_id", &set)
                                .and_where(C::equal("note_id", &s.note_id))
                                .and_where(C::equal("notebook_id", &s.notebook_id))
                                .and_where(C::equal("revision", expected))
                                .build(),
                        )
                        .await?
                        .rows_affected
                        != 1
                    {
                        return Err(conflict());
                    }
                } else {
                    tx.write(
                        Q::insert_into("notebook_notes")
                            .value("note_id", &s.note_id)
                            .value("notebook_id", &s.notebook_id)
                            .value("folder_id", s.folder_id.as_deref())
                            .value("title", &s.title)
                            .value("blocks_json", blocks)
                            .value("revision", 1_i64)
                            .value("active_set_id", &set)
                            .value("created_at", &s.created_at)
                            .value("updated_at", &s.updated_at)
                            .build(),
                    )
                    .await?;
                }
                if let Some(batch) = prepared {
                    batch.insert(tx, n, &set).await?;
                }
                tx.write(
                    Q::delete_from("notebook_search_sets")
                        .and_where(C::equal("note_id", &s.note_id))
                        .and_where(C::compare("set_id", Op::NotEqual, &set))
                        .build(),
                )
                .await?;
                // Repeat token/status/DB-clock predicates after row waits and every chunk batch.
                // A lease lost while preparing or waiting must roll back native and derived state.
                if tx
                    .write(
                        Q::update("notebook_save_jobs")
                            .set("committed_revision", s.revision)
                            .set("committed_set_id", set)
                            .set("content_hash", hash)
                            .set("status", "completed")
                            .set("stage", "indexing")
                            .set("updated_at", now())
                            .increment("sequence")
                            .and_where(C::equal("job_id", job.job_id))
                            .and_where(C::equal("lease_token", &claim.token))
                            .and_where(C::equal("status", "processing"))
                            .and_where(C::equal("stage", &job.stage))
                            .and_where(C::unexpired("lease_until"))
                            .build(),
                    )
                    .await?
                    .rows_affected
                    != 1
                {
                    return Err(conflict());
                }
                release_slot(tx, &claim.job.job_id).await?;
                Ok(())
            })
            .await
    }
    pub(super) async fn checkpoint(&self, claim: &ClaimedJob, stage: &'static str) -> Result<()> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                fence(&tx, claim).await?;
                if tx
                    .write(
                        Q::update("notebook_save_jobs")
                            .set("stage", stage)
                            .set("updated_at", now())
                            .increment("sequence")
                            .and_where(C::equal("job_id", &claim.job.job_id))
                            .and_where(C::equal("lease_token", &claim.token))
                            .and_where(C::equal("status", "processing"))
                            .and_where(C::unexpired("lease_until"))
                            .build(),
                    )
                    .await?
                    .rows_affected
                    != 1
                {
                    return Err(conflict());
                }
                Ok(())
            })
            .await
    }
    pub(super) async fn fail(&self, claim: &ClaimedJob, code: &'static str) -> Result<()> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let changed = tx
                    .write(
                        Q::update("notebook_save_jobs")
                            .set("status", "failed")
                            .set("error_code", code)
                            .set("updated_at", now())
                            .increment("sequence")
                            .and_where(C::equal("job_id", &claim.job.job_id))
                            .and_where(C::equal("lease_token", &claim.token))
                            .and_where(C::equal("status", "processing"))
                            .and_where(C::unexpired("lease_until"))
                            .build(),
                    )
                    .await?;
                if changed.rows_affected == 1 {
                    release_slot(&tx, &claim.job.job_id).await?;
                }
                Ok(())
            })
            .await
    }
    pub(super) async fn prune(&self) -> Result<()> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.postgres
            .write_async(
                Q::delete_from("notebook_save_jobs")
                    .and_where(C::in_values("status", ["completed", "failed"]))
                    .and_where(C::compare("updated_at", Op::LessThan, cutoff))
                    .build(),
            )
            .await?;
        Ok(())
    }
}
// The user row serializes only admissions/retries for that user. Terminal transitions only
// free slots, and never acquire this row while holding a parent or slot.
async fn admission_user(tx: &PostgresTransaction, owner: i64) -> Result<()> {
    tx.write(
        Q::insert_into("notebook_admission_users")
            .value("user_id", owner)
            .on_conflict_do_nothing(["user_id"])
            .build(),
    )
    .await?;
    tx.lock_with(
        Q::select("notebook_admission_users")
            .and_where(C::equal("user_id", owner))
            .lock(RowLock::Update, LockWait::Wait)
            .build(),
        |r| r.get::<_, i64>(0),
    )
    .await?;
    Ok(())
}
async fn free_slot(tx: &PostgresTransaction, owner: i64) -> Result<i64> {
    if tx
        .read(
            Q::select("notebook_save_jobs")
                .column("job_id")
                .and_where(C::equal("user_id", owner))
                .and_where(C::in_values("status", ACTIVE))
                .limit(32)
                .build(),
        )
        .await?
        .len()
        >= 32
    {
        return Err(Error::Capacity);
    }
    tx.lock_with(
        Q::select("notebook_save_slots")
            .column("slot")
            .and_where(C::is_null("job_id"))
            .order_by("slot", Sort::Ascending)
            .limit(1)
            .lock(RowLock::Update, LockWait::SkipLocked)
            .build(),
        |r| r.get(0),
    )
    .await?
    .pop()
    .ok_or(Error::Capacity)
}
async fn allocate_slot(tx: &PostgresTransaction, slot: i64, job: &str) -> Result<()> {
    if tx
        .write(
            Q::update("notebook_save_slots")
                .set("job_id", job)
                .and_where(C::equal("slot", slot))
                .and_where(C::is_null("job_id"))
                .build(),
        )
        .await?
        .rows_affected
        != 1
    {
        return Err(Error::Capacity);
    }
    Ok(())
}
pub(super) async fn release_slot(tx: &PostgresTransaction, job: &str) -> Result<()> {
    tx.write(
        Q::update("notebook_save_slots")
            .set("job_id", None::<String>)
            .and_where(C::equal("job_id", job))
            .build(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/integration/domains/notebooks/job_repository_support.rs"]
mod test_support;
