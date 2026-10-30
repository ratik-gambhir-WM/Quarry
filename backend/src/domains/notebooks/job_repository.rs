//! Durable admission, leases and checkpoints. Typed errors roll back every mutation.
use super::{
    model::*,
    repository::{conflict, folder, missing, note, owned, NotebookRepository, Result},
};
use crate::{
    adapters::sqlite::{
        client::SqliteTransaction,
        query::{ComparisonOperator as Op, Condition as C, SortDirection as Sort, SqlBuilder as Q},
    },
    shared::error::RepositoryError as Error,
};
use rusqlite::Row;
use sha2::{Digest, Sha256};

const ACTIVE: [&str; 2] = ["queued", "processing"];
fn job_row(r: &Row<'_>) -> rusqlite::Result<SaveJob> {
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
fn load(tx: &SqliteTransaction<'_>, id: &str) -> Result<SaveJob> {
    tx.read_one_with(
        Q::select("notebook_save_jobs")
            .and_where(C::equal("job_id", id))
            .build(),
        job_row,
    )?
    .ok_or_else(missing)
}
fn fence(tx: &SqliteTransaction<'_>, claim: &ClaimedJob) -> Result<SaveJob> {
    let job = tx
        .read_one_with(
            Q::select("notebook_save_jobs")
                .and_where(C::equal("job_id", &claim.job.job_id))
                .and_where(C::equal("lease_token", &claim.token))
                .and_where(C::equal("status", "processing"))
                .and_where(C::compare(
                    "lease_until",
                    Op::GreaterThan,
                    chrono::Utc::now().timestamp(),
                ))
                .build(),
            job_row,
        )?
        .ok_or_else(conflict)?;
    owned(tx, claim.owner, &job.notebook_id)?;
    if let Some(revision) = job.committed_revision {
        if note(tx, &job.notebook_id, &job.note_id)?.summary.revision != revision {
            return Err(conflict());
        }
    }
    Ok(job)
}
impl NotebookRepository {
    #[cfg(test)]
    pub(super) async fn admit(
        &self,
        owner: i64,
        notebook: String,
        note_id: Option<String>,
        input: SaveInput,
    ) -> Result<SaveJob> {
        self.admit_with_pipeline(owner, notebook, note_id, input, PIPELINE.into())
            .await
    }
    pub(super) async fn admit_with_pipeline(
        &self,
        owner: i64,
        notebook: String,
        note_id: Option<String>,
        input: SaveInput,
        pipeline_version: String,
    ) -> Result<SaveJob> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                let bytes = serde_json::to_vec(&(&notebook, &note_id, &input))
                    .map_err(|_| Error::storage("invalid save input"))?;
                let hash = format!("{:x}", Sha256::digest(bytes));
                if let Some((existing, prior_hash, prior_parent)) = tx.read_one_with(
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
                )? {
                    if prior_hash != hash || prior_parent != notebook {
                        return Err(Error::Conflict(
                            "operationId already used with different input".into(),
                        ));
                    }
                    return Ok(existing);
                }
                let time = now();
                let mut snapshot = if let Some(id) = &note_id {
                    note(tx, &notebook, id)?
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
                    folder(tx, &notebook, id)?;
                }
                let expected = if let PatchField::Value(value) = input.expected_revision {
                    Some(value)
                } else {
                    None
                };
                if let Some(expected) = expected {
                    if snapshot.summary.revision != expected || expected == MAX_REVISION {
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
                let active_note = tx.read_one(
                    Q::select("notebook_save_jobs")
                        .columns(["job_id"])
                        .and_where(C::equal("note_id", &snapshot.summary.note_id))
                        .and_where(C::in_values("status", ACTIVE))
                        .build(),
                )?;
                if active_note.is_some() {
                    return Err(Error::Conflict("note save already active".into()));
                }
                let global = tx.read(
                    Q::select("notebook_save_jobs")
                        .columns(["job_id"])
                        .and_where(C::in_values("status", ACTIVE))
                        .limit(256)
                        .build(),
                )?;
                let actor = tx.read(
                    Q::select("notebook_save_jobs")
                        .columns(["job_id"])
                        .and_where(C::equal("user_id", owner))
                        .and_where(C::in_values("status", ACTIVE))
                        .limit(32)
                        .build(),
                )?;
                if global.len() >= 256 || actor.len() >= 32 {
                    return Err(Error::Capacity);
                }
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
                        .value("stage", "persisting")
                        .value("created_at", &time)
                        .value("updated_at", time)
                        .build(),
                )?;
                load(tx, &id)
            })
            .await
    }
    pub(super) async fn job(&self, owner: i64, notebook: String, id: String) -> Result<SaveJob> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                let job = load(tx, &id)?;
                if job.notebook_id != notebook {
                    return Err(missing());
                }
                Ok(job)
            })
            .await
    }
    pub(super) async fn retry(&self, owner: i64, notebook: String, id: String) -> Result<SaveJob> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                let job = load(tx, &id)?;
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
                    )?
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
                    if note(tx, &notebook, &job.note_id)?.summary.revision != revision {
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
                    )?
                    .is_some()
                {
                    return Err(Error::Conflict("note save already active".into()));
                }
                // Retries share the same bounded queue as new admissions.
                if tx
                    .read(
                        Q::select("notebook_save_jobs")
                            .columns(["job_id"])
                            .and_where(C::in_values("status", ACTIVE))
                            .limit(256)
                            .build(),
                    )?
                    .len()
                    >= 256
                    || tx
                        .read(
                            Q::select("notebook_save_jobs")
                                .columns(["job_id"])
                                .and_where(C::equal("user_id", owner))
                                .and_where(C::in_values("status", ACTIVE))
                                .limit(32)
                                .build(),
                        )?
                        .len()
                        >= 32
                {
                    return Err(Error::Capacity);
                }
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
                )?;
                load(tx, &id)
            })
            .await
    }
    pub(super) async fn claim(&self) -> Result<Option<ClaimedJob>> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let base = || {
                    Q::select("notebook_save_jobs")
                        .order_by("created_at", Sort::Ascending)
                        .order_by("job_id", Sort::Ascending)
                        .limit(1)
                };
                let mut candidate = tx.read_one_with(
                    base().and_where(C::equal("status", "queued")).build(),
                    |r| {
                        Ok((
                            job_row(r)?,
                            r.get::<_, i64>("user_id")?,
                            r.get::<_, String>("input_json")?,
                        ))
                    },
                )?;
                if candidate.is_none() {
                    candidate = tx.read_one_with(
                        base()
                            .and_where(C::equal("status", "processing"))
                            .and_where(C::compare(
                                "lease_until",
                                Op::LessThanOrEqual,
                                chrono::Utc::now().timestamp(),
                            ))
                            .build(),
                        |r| {
                            Ok((
                                job_row(r)?,
                                r.get::<_, i64>("user_id")?,
                                r.get::<_, String>("input_json")?,
                            ))
                        },
                    )?;
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
                    )?;
                    return Ok(None);
                };
                let token = uuid::Uuid::new_v4().to_string();
                // One attempt has a 120-second deadline. Lease is longer so a live attempt cannot expire.
                tx.write(
                    Q::update("notebook_save_jobs")
                        .set("status", "processing")
                        .set("lease_token", &token)
                        .set("lease_until", chrono::Utc::now().timestamp() + 180)
                        .set("updated_at", now())
                        .increment("sequence")
                        .and_where(C::equal("job_id", &job.job_id))
                        .build(),
                )?;
                Ok(Some(ClaimedJob {
                    job: load(tx, &job.job_id)?,
                    owner,
                    token,
                    input,
                }))
            })
            .await
    }
    pub(super) async fn persist(&self, claim: &ClaimedJob) -> Result<()> {
        let claim = clone_claim(claim);
        self.sqlite
            .transaction_result_async(move |tx| {
                let job = fence(tx, &claim)?;
                if job.committed_revision.is_some() {
                    return Ok(());
                }
                let n = &claim.input.note;
                let s = &n.summary;
                if let Some(id) = &s.folder_id {
                    folder(tx, &s.notebook_id, id)?;
                }
                let blocks = serde_json::to_string(&n.blocks)
                    .map_err(|_| Error::storage("invalid durable blocks"))?;
                if let Some(expected) = claim.input.expected_revision {
                    if note(tx, &s.notebook_id, &s.note_id)?.summary.revision != expected {
                        return Err(conflict());
                    }
                    tx.write(
                        Q::update("notebook_notes")
                            .set("title", &s.title)
                            .set("folder_id", s.folder_id.as_deref())
                            .set("blocks_json", blocks)
                            .set("content_schema_version", 1_i64)
                            .set("revision", s.revision)
                            .set("updated_at", &s.updated_at)
                            .and_where(C::equal("note_id", &s.note_id))
                            .build(),
                    )?;
                } else {
                    // A deletion tombstone fences a worker even before an admitted create commits.
                    if tx
                        .read_one(
                            Q::select("notebook_cleanup")
                                .and_where(C::equal("note_id", &s.note_id))
                                .build(),
                        )?
                        .is_some()
                    {
                        return Err(missing());
                    }
                    tx.write(
                        Q::insert_into("notebook_notes")
                            .value("note_id", &s.note_id)
                            .value("notebook_id", &s.notebook_id)
                            .value("folder_id", s.folder_id.as_deref())
                            .value("title", &s.title)
                            .value("blocks_json", blocks)
                            .value("content_schema_version", 1_i64)
                            .value("revision", 1_i64)
                            .value("created_at", &s.created_at)
                            .value("updated_at", &s.updated_at)
                            .build(),
                    )?;
                }
                tx.write(
                    Q::update("notebook_save_jobs")
                        .set("committed_revision", s.revision)
                        .set("stage", "exporting")
                        .set("updated_at", now())
                        .increment("sequence")
                        .and_where(C::equal("job_id", job.job_id))
                        .build(),
                )?;
                Ok(())
            })
            .await
    }
    pub(super) async fn checkpoint(
        &self,
        claim: &ClaimedJob,
        stage: &'static str,
        hash: Option<String>,
        complete: bool,
    ) -> Result<()> {
        let claim = clone_claim(claim);
        self.sqlite
            .transaction_result_async(move |tx| {
                let job = fence(tx, &claim)?;
                tx.write(
                    Q::update("notebook_save_jobs")
                        .set("stage", stage)
                        .set("content_hash", hash)
                        .set("status", if complete { "completed" } else { "processing" })
                        .set("updated_at", now())
                        .increment("sequence")
                        .and_where(C::equal("job_id", job.job_id))
                        .build(),
                )?;
                Ok(())
            })
            .await
    }
    pub(super) async fn fail(&self, claim: &ClaimedJob, code: &'static str) -> Result<()> {
        let id = claim.job.job_id.clone();
        let token = claim.token.clone();
        self.sqlite
            .write_async(
                Q::update("notebook_save_jobs")
                    .set("status", "failed")
                    .set("error_code", code)
                    .set("updated_at", now())
                    .increment("sequence")
                    .and_where(C::equal("job_id", id))
                    .and_where(C::equal("lease_token", token))
                    .and_where(C::equal("status", "processing"))
                    .build(),
            )
            .await?;
        Ok(())
    }
    pub(super) async fn cleanup_batch(&self) -> Result<Vec<(String, i64)>> {
        Ok(self
            .sqlite
            .read_with_async(
                Q::select("notebook_cleanup")
                    .and_where(C::compare(
                        "next_attempt_at",
                        Op::LessThanOrEqual,
                        chrono::Utc::now().timestamp(),
                    ))
                    .order_by("created_at", Sort::Ascending)
                    .limit(1)
                    .build(),
                |r| Ok((r.get("note_id")?, r.get("user_id")?)),
            )
            .await?)
    }
    pub(super) async fn cleaned(&self, note: String) -> Result<()> {
        self.sqlite
            .write_async(
                Q::update("notebook_cleanup")
                    .set("next_attempt_at", chrono::Utc::now().timestamp() + 3600)
                    .set("confirmed", 1_i64)
                    .and_where(C::equal("note_id", note))
                    .build(),
            )
            .await?;
        Ok(())
    }
    pub(super) async fn cleanup_failed(&self, note: String) -> Result<()> {
        self.sqlite
            .write_async(
                Q::update("notebook_cleanup")
                    .set("next_attempt_at", chrono::Utc::now().timestamp() + 60)
                    .and_where(C::equal("note_id", note))
                    .build(),
            )
            .await?;
        Ok(())
    }
    pub(super) async fn prune(&self) -> Result<()> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.sqlite
            .write_async(
                Q::delete_from("notebook_save_jobs")
                    .and_where(C::in_values("status", ["completed", "failed"]))
                    .and_where(C::compare("updated_at", Op::LessThan, cutoff))
                    .build(),
            )
            .await?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.sqlite
            .write_async(
                Q::delete_from("notebook_cleanup")
                    .and_where(C::equal("confirmed", 1_i64))
                    .and_where(C::compare("created_at", Op::LessThan, cutoff))
                    .build(),
            )
            .await?;
        Ok(())
    }
}
fn clone_claim(c: &ClaimedJob) -> ClaimedJob {
    ClaimedJob {
        job: c.job.clone(),
        owner: c.owner,
        token: c.token.clone(),
        input: c.input.clone(),
    }
}
